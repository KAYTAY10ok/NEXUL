//! Cross-platform pseudo-terminal and VT screen model for NEXUL Studio.
//!
//! The PTY backend is provided by `portable-pty` (ConPTY on Windows and the
//! platform's native PTY elsewhere); `vte` supplies the escape-sequence parser.

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use unicode_width::UnicodeWidthChar;

const DEFAULT_SCROLLBACK: usize = 2_000;
const MAX_DIMENSION: u16 = 1_000;

/// A shell executable and its arguments. Use the constructors for common shells,
/// or provide a custom executable for user-configured shells.
#[derive(Clone, Debug)]
pub struct ShellSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: BTreeMap<OsString, OsString>,
}

impl ShellSpec {
    /// Returns PowerShell on Windows and `$SHELL` (or `sh`) on Unix.
    pub fn default_shell() -> io::Result<Self> {
        #[cfg(windows)]
        {
            let executable = find_executable(&["pwsh.exe", "powershell.exe", "pwsh", "powershell"])
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "PowerShell was not found in PATH")
                })?;
            let mut shell = Self::custom(executable);
            shell.args.push(OsString::from("-NoLogo"));
            return Ok(shell);
        }
        #[cfg(not(windows))]
        {
            let shell = std::env::var_os("SHELL")
                .filter(|p| Path::new(p).is_file())
                .unwrap_or_else(|| OsString::from("sh"));
            Ok(Self::custom(PathBuf::from(shell)))
        }
    }

    pub fn powershell() -> io::Result<Self> {
        let executable = find_executable(&["pwsh.exe", "powershell.exe", "pwsh", "powershell"])
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "PowerShell was not found in PATH")
            })?;
        let mut shell = Self::custom(executable);
        shell.args.push(OsString::from("-NoLogo"));
        Ok(shell)
    }

    pub fn cmd() -> io::Result<Self> {
        let executable = find_executable(&["cmd.exe", "cmd"]).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "cmd.exe was not found in PATH")
        })?;
        let mut shell = Self::custom(executable);
        shell.args.extend([OsString::from("/Q")]);
        Ok(shell)
    }

    pub fn git_bash() -> io::Result<Self> {
        let mut candidates = Vec::new();
        if let Some(root) = std::env::var_os("ProgramFiles") {
            candidates.push(PathBuf::from(&root).join("Git\\bin\\bash.exe"));
            candidates.push(PathBuf::from(root).join("Git\\usr\\bin\\bash.exe"));
        }
        if let Some(program) = find_executable(&["bash.exe", "bash"]) {
            candidates.push(program);
        }
        let program = candidates
            .into_iter()
            .find(|p| p.is_file())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Git Bash was not found"))?;
        let mut shell = Self::custom(program);
        shell
            .args
            .extend([OsString::from("--login"), OsString::from("-i")]);
        Ok(shell)
    }

    pub fn wsl() -> io::Result<Self> {
        let program = find_executable(&["wsl.exe", "wsl"]).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "wsl.exe was not found in PATH")
        })?;
        Ok(Self::custom(program))
    }

    pub fn custom(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            env: BTreeMap::new(),
        }
    }

    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
}

fn find_executable(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let extensions: Vec<OsString> = if cfg!(windows) {
        std::env::var_os("PATHEXT")
            .unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"))
            .to_string_lossy()
            .split(';')
            .filter(|s| !s.is_empty())
            .map(OsString::from)
            .collect()
    } else {
        vec![OsString::new()]
    };
    for dir in std::env::split_paths(&path) {
        for name in names {
            let named = PathBuf::from(name);
            let has_extension = named.extension().is_some();
            let base = dir.join(name);
            if base.is_file() {
                return Some(base);
            }
            if !has_extension {
                for extension in &extensions {
                    let candidate = dir.join(format!("{}{}", name, extension.to_string_lossy()));
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }
    None
}

/// Terminal colors retain ANSI indexed and RGB forms for the UI renderer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Color {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CellStyle {
    pub foreground: Color,
    pub background: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// One terminal cell. A wide Unicode glyph uses a leading cell with `width = 2`
/// and a continuation cell with `width = 0`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub width: u8,
    pub style: CellStyle,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            text: " ".into(),
            width: 1,
            style: CellStyle::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellPosition {
    /// Row in the combined scrollback and visible screen.
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: CellPosition,
    pub focus: CellPosition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenSnapshot {
    pub columns: usize,
    pub rows: usize,
    pub scrollback: Vec<Vec<Cell>>,
    pub screen: Vec<Vec<Cell>>,
    pub cursor: Cursor,
    pub title: Option<String>,
    pub selection: Option<Selection>,
}

impl ScreenSnapshot {
    pub fn lines(&self) -> impl Iterator<Item = &[Cell]> {
        self.scrollback
            .iter()
            .chain(self.screen.iter())
            .map(Vec::as_slice)
    }

    pub fn plain_text(&self) -> String {
        self.lines()
            .map(|line| {
                line.iter()
                    .filter(|cell| cell.width != 0)
                    .map(|cell| cell.text.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalEvent {
    /// Raw bytes from the PTY; use the parsed screen snapshot for rendering.
    Output(Vec<u8>),
    TitleChanged(String),
    Exited(Option<i32>),
    Error(String),
}

struct Screen {
    columns: usize,
    rows: usize,
    scrollback_limit: usize,
    scrollback: VecDeque<Vec<Cell>>,
    cells: Vec<Vec<Cell>>,
    cursor_row: usize,
    cursor_col: usize,
    wrap_pending: bool,
    cursor_visible: bool,
    current_style: CellStyle,
    saved_cursor: (usize, usize, CellStyle),
    scroll_top: usize,
    scroll_bottom: usize,
    title: Option<String>,
    selection: Option<Selection>,
}

impl Screen {
    fn new(columns: u16, rows: u16, scrollback_limit: usize) -> Self {
        let columns = dimension(columns);
        let rows = dimension(rows);
        Self {
            columns,
            rows,
            scrollback_limit,
            scrollback: VecDeque::new(),
            cells: vec![vec![Cell::default(); columns]; rows],
            cursor_row: 0,
            cursor_col: 0,
            wrap_pending: false,
            cursor_visible: true,
            current_style: CellStyle::default(),
            saved_cursor: (0, 0, CellStyle::default()),
            scroll_top: 0,
            scroll_bottom: rows - 1,
            title: None,
            selection: None,
        }
    }

    fn snapshot(&self) -> ScreenSnapshot {
        ScreenSnapshot {
            columns: self.columns,
            rows: self.rows,
            scrollback: self.scrollback.iter().cloned().collect(),
            screen: self.cells.clone(),
            cursor: Cursor {
                row: self.cursor_row,
                col: self.cursor_col,
                visible: self.cursor_visible,
            },
            title: self.title.clone(),
            selection: self.selection,
        }
    }

    fn resize(&mut self, columns: u16, rows: u16) {
        let columns = dimension(columns);
        let rows = dimension(rows);
        for line in &mut self.cells {
            line.resize(columns, Cell::default());
            if columns > 0 {
                line.truncate(columns);
            }
        }
        if rows < self.rows {
            let remove = self.rows - rows;
            for _ in 0..remove {
                let line = self.cells.remove(0);
                self.push_scrollback(line);
            }
            self.cursor_row = self.cursor_row.saturating_sub(remove);
        } else if rows > self.rows {
            self.cells
                .extend((0..rows - self.rows).map(|_| vec![Cell::default(); columns]));
        }
        self.columns = columns;
        self.rows = rows;
        self.cursor_row = self.cursor_row.min(rows - 1);
        self.cursor_col = self.cursor_col.min(columns - 1);
        self.scroll_top = 0;
        self.scroll_bottom = rows - 1;
        self.wrap_pending = false;
    }

    fn push_scrollback(&mut self, line: Vec<Cell>) {
        if self.scrollback_limit == 0 {
            return;
        }
        if self.scrollback.len() == self.scrollback_limit {
            self.scrollback.pop_front();
        }
        self.scrollback.push_back(line);
    }

    fn index(&mut self) {
        if self.cursor_row == self.scroll_bottom {
            self.scroll_region_up(1);
        } else if self.cursor_row + 1 < self.rows {
            self.cursor_row += 1;
        }
    }

    fn reverse_index(&mut self) {
        if self.cursor_row == self.scroll_top {
            self.scroll_region_down(1);
        } else {
            self.cursor_row = self.cursor_row.saturating_sub(1);
        }
    }

    fn scroll_region_up(&mut self, count: usize) {
        let count = count.min(self.scroll_bottom - self.scroll_top + 1);
        for _ in 0..count {
            let line = self.cells.remove(self.scroll_top);
            if self.scroll_top == 0 && self.scroll_bottom + 1 == self.rows {
                self.push_scrollback(line);
            }
            self.cells
                .insert(self.scroll_bottom, vec![Cell::default(); self.columns]);
        }
    }

    fn scroll_region_down(&mut self, count: usize) {
        let count = count.min(self.scroll_bottom - self.scroll_top + 1);
        for _ in 0..count {
            self.cells.remove(self.scroll_bottom);
            self.cells
                .insert(self.scroll_top, vec![Cell::default(); self.columns]);
        }
    }

    fn line_feed(&mut self) {
        self.index();
    }

    fn print(&mut self, ch: char) {
        let width = UnicodeWidthChar::width(ch).unwrap_or(0).min(2);
        if width == 0 {
            if self.cursor_col > 0 {
                let col = if self.wrap_pending {
                    self.cursor_col
                } else {
                    self.cursor_col - 1
                };
                if let Some(cell) = self.cells[self.cursor_row].get_mut(col) {
                    cell.text.push(ch);
                }
            }
            return;
        }
        if self.wrap_pending {
            self.cursor_col = 0;
            self.line_feed();
            self.wrap_pending = false;
        }
        if width == 2 && self.cursor_col + 1 >= self.columns {
            self.cursor_col = 0;
            self.line_feed();
        }
        self.cells[self.cursor_row][self.cursor_col] = Cell {
            text: ch.to_string(),
            width: width as u8,
            style: self.current_style.clone(),
        };
        if width == 2 {
            self.cells[self.cursor_row][self.cursor_col + 1] = Cell {
                text: String::new(),
                width: 0,
                style: self.current_style.clone(),
            };
        }
        let next = self.cursor_col + width;
        if next >= self.columns {
            self.cursor_col = self.columns - 1;
            self.wrap_pending = true;
        } else {
            self.cursor_col = next;
        }
    }

    fn erase_line(&mut self, mode: u16) {
        let (start, end) = match mode {
            1 => (0, self.cursor_col + 1),
            2 => (0, self.columns),
            _ => (self.cursor_col, self.columns),
        };
        for cell in &mut self.cells[self.cursor_row][start.min(self.columns)..end.min(self.columns)]
        {
            *cell = Cell::default();
        }
    }

    fn erase_display(&mut self, mode: u16) {
        match mode {
            1 => {
                for row in 0..self.cursor_row {
                    self.cells[row].fill(Cell::default());
                }
                let end = self.cursor_col.saturating_add(1).min(self.columns);
                self.cells[self.cursor_row][..end].fill(Cell::default());
            }
            2 | 3 => {
                for row in &mut self.cells {
                    row.fill(Cell::default());
                }
                if mode == 3 {
                    self.scrollback.clear();
                }
            }
            _ => {
                self.cells[self.cursor_row][self.cursor_col..].fill(Cell::default());
                for row in self.cursor_row + 1..self.rows {
                    self.cells[row].fill(Cell::default());
                }
            }
        }
    }

    fn set_cursor(&mut self, row: usize, col: usize) {
        self.cursor_row = row.min(self.rows - 1);
        self.cursor_col = col.min(self.columns - 1);
        self.wrap_pending = false;
    }

    fn set_selection(&mut self, selection: Option<Selection>) {
        self.selection = selection;
    }

    fn selected_text(&self) -> Option<String> {
        let selection = self.selection?;
        let total = self.scrollback.len() + self.rows;
        let (start, end) = if (selection.anchor.row, selection.anchor.col)
            <= (selection.focus.row, selection.focus.col)
        {
            (selection.anchor, selection.focus)
        } else {
            (selection.focus, selection.anchor)
        };
        if start.row >= total || end.row >= total {
            return None;
        }
        let lines: Vec<&Vec<Cell>> = self.scrollback.iter().chain(self.cells.iter()).collect();
        let mut selected = Vec::new();
        for row in start.row..=end.row {
            let from = if row == start.row { start.col } else { 0 };
            let to = if row == end.row {
                end.col.saturating_add(1)
            } else {
                self.columns
            };
            let line = lines[row];
            let text = line
                .iter()
                .enumerate()
                .filter(|(col, _)| *col >= from && *col < to)
                .filter(|(_, cell)| cell.width != 0)
                .map(|(_, cell)| cell.text.as_str())
                .collect::<String>();
            selected.push(text.trim_end().to_owned());
        }
        Some(selected.join("\n"))
    }
}

fn dimension(value: u16) -> usize {
    value.clamp(1, MAX_DIMENSION) as usize
}

impl vte::Perform for Screen {
    fn print(&mut self, ch: char) {
        Screen::print(self, ch);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0b | 0x0c => self.line_feed(),
            b'\r' => {
                self.cursor_col = 0;
                self.wrap_pending = false;
            }
            b'\x08' => {
                self.cursor_col = self.cursor_col.saturating_sub(1);
                self.wrap_pending = false;
            }
            b'\t' => {
                let next = ((self.cursor_col / 8) + 1) * 8;
                self.cursor_col = next.min(self.columns - 1);
                self.wrap_pending = false;
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let values: Vec<Vec<u16>> = params.iter().map(|p| p.to_vec()).collect();
        let first = |default| {
            values
                .first()
                .and_then(|p| p.first())
                .copied()
                .filter(|v| *v != 0)
                .unwrap_or(default)
        };
        let n = first(1) as usize;
        let private = intermediates == b"?";
        match action {
            'A' => self.set_cursor(self.cursor_row.saturating_sub(n), self.cursor_col),
            'B' => self.set_cursor((self.cursor_row + n).min(self.rows - 1), self.cursor_col),
            'C' => self.set_cursor(self.cursor_row, (self.cursor_col + n).min(self.columns - 1)),
            'D' => self.set_cursor(self.cursor_row, self.cursor_col.saturating_sub(n)),
            'E' => self.set_cursor((self.cursor_row + n).min(self.rows - 1), 0),
            'F' => self.set_cursor(self.cursor_row.saturating_sub(n), 0),
            'G' | '`' => self.set_cursor(self.cursor_row, first(1).saturating_sub(1) as usize),
            'd' => self.set_cursor(first(1).saturating_sub(1) as usize, self.cursor_col),
            'H' | 'f' => {
                let row = values
                    .first()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize
                    - 1;
                let col = values
                    .get(1)
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize
                    - 1;
                self.set_cursor(row, col);
            }
            'J' => self.erase_display(first(0)),
            'K' => self.erase_line(first(0)),
            'm' => self.sgr(&values),
            'r' => {
                let top = values
                    .first()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(1)
                    .max(1) as usize
                    - 1;
                let bottom = values
                    .get(1)
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(self.rows as u16) as usize
                    - 1;
                if top < bottom && bottom < self.rows {
                    self.scroll_top = top;
                    self.scroll_bottom = bottom;
                    self.set_cursor(top, 0);
                }
            }
            's' => {
                self.saved_cursor = (self.cursor_row, self.cursor_col, self.current_style.clone())
            }
            'u' => {
                let (row, col, style) = self.saved_cursor.clone();
                self.set_cursor(row, col);
                self.current_style = style;
            }
            'S' => self.scroll_region_up(n),
            'T' => self.scroll_region_down(n),
            'L' => {
                let (old_top, old_bottom) = (self.scroll_top, self.scroll_bottom);
                self.scroll_top = self.cursor_row;
                self.scroll_bottom = self.rows - 1;
                self.scroll_region_down(n);
                self.scroll_top = old_top;
                self.scroll_bottom = old_bottom;
            }
            'M' => {
                let (old_top, old_bottom) = (self.scroll_top, self.scroll_bottom);
                self.scroll_top = self.cursor_row;
                self.scroll_bottom = self.rows - 1;
                self.scroll_region_up(n);
                self.scroll_top = old_top;
                self.scroll_bottom = old_bottom;
            }
            '@' => {
                let row = &mut self.cells[self.cursor_row];
                for _ in 0..n {
                    row.insert(self.cursor_col, Cell::default());
                    row.pop();
                }
            }
            'P' => {
                let row = &mut self.cells[self.cursor_row];
                for _ in 0..n {
                    row.remove(self.cursor_col);
                    row.push(Cell::default());
                }
            }
            'X' => {
                let end = (self.cursor_col + n).min(self.columns);
                for cell in &mut self.cells[self.cursor_row][self.cursor_col..end] {
                    *cell = Cell::default();
                }
            }
            'h' | 'l' if private => {
                if values.iter().any(|p| p.first() == Some(&25)) {
                    self.cursor_visible = action == 'h';
                }
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if let Some(command) = params
            .first()
            .and_then(|p| std::str::from_utf8(p).ok())
            .and_then(|s| s.parse::<u16>().ok())
        {
            if command == 0 || command == 2 {
                let title = params
                    .get(1)
                    .map(|p| String::from_utf8_lossy(p).into_owned())
                    .unwrap_or_default();
                self.title = Some(title);
            }
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], ignore: bool, byte: u8) {
        if ignore {
            return;
        }
        match byte {
            b'7' => {
                self.saved_cursor = (self.cursor_row, self.cursor_col, self.current_style.clone())
            }
            b'8' => {
                let (row, col, style) = self.saved_cursor.clone();
                self.set_cursor(row, col);
                self.current_style = style;
            }
            b'D' => self.line_feed(),
            b'E' => {
                self.cursor_col = 0;
                self.line_feed();
            }
            b'M' => self.reverse_index(),
            b'c' => {
                self.current_style = CellStyle::default();
                self.scrollback.clear();
                self.cells
                    .iter_mut()
                    .for_each(|line| line.fill(Cell::default()));
                self.set_cursor(0, 0);
            }
            _ => {}
        }
    }
}

impl Screen {
    fn sgr(&mut self, params: &[Vec<u16>]) {
        let all: Vec<u16> = if params.is_empty() {
            vec![0]
        } else {
            params
                .iter()
                .map(|p| p.first().copied().unwrap_or(0))
                .collect()
        };
        let mut i = 0;
        while i < all.len() {
            match all[i] {
                0 => self.current_style = CellStyle::default(),
                1 => self.current_style.bold = true,
                3 => self.current_style.italic = true,
                4 => self.current_style.underline = true,
                22 => self.current_style.bold = false,
                23 => self.current_style.italic = false,
                24 => self.current_style.underline = false,
                30..=37 => self.current_style.foreground = Color::Indexed((all[i] - 30) as u8),
                90..=97 => self.current_style.foreground = Color::Indexed((all[i] - 90 + 8) as u8),
                39 => self.current_style.foreground = Color::Default,
                40..=47 => self.current_style.background = Color::Indexed((all[i] - 40) as u8),
                100..=107 => {
                    self.current_style.background = Color::Indexed((all[i] - 100 + 8) as u8)
                }
                49 => self.current_style.background = Color::Default,
                38 | 48 => {
                    let is_foreground = all[i] == 38;
                    if i + 2 < all.len() && all[i + 1] == 5 {
                        let color = Color::Indexed(all[i + 2].min(255) as u8);
                        if is_foreground {
                            self.current_style.foreground = color;
                        } else {
                            self.current_style.background = color;
                        }
                        i += 2;
                    } else if i + 4 < all.len() && all[i + 1] == 2 {
                        let color = Color::Rgb(
                            all[i + 2].min(255) as u8,
                            all[i + 3].min(255) as u8,
                            all[i + 4].min(255) as u8,
                        );
                        if is_foreground {
                            self.current_style.foreground = color;
                        } else {
                            self.current_style.background = color;
                        }
                        i += 4;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

struct ParserState {
    parser: vte::Parser,
    screen: Screen,
}

/// A single live PTY terminal. Each instance has independent reader, writer and
/// child-process state, so multiple terminals can run concurrently.
pub struct Terminal {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
    state: Arc<Mutex<ParserState>>,
    events: Receiver<TerminalEvent>,
    stopped: Arc<AtomicBool>,
    reader_thread: Option<JoinHandle<()>>,
}

impl Terminal {
    /// Start a shell in a PTY using the requested grid size.
    pub fn spawn(
        shell: ShellSpec,
        cwd: impl AsRef<Path>,
        cols: u16,
        rows: u16,
    ) -> io::Result<Self> {
        Self::spawn_with_scrollback(shell, cwd, cols, rows, DEFAULT_SCROLLBACK)
    }

    pub fn spawn_default(cwd: impl AsRef<Path>, cols: u16, rows: u16) -> io::Result<Self> {
        Self::spawn(ShellSpec::default_shell()?, cwd, cols, rows)
    }

    pub fn spawn_with_scrollback(
        shell: ShellSpec,
        cwd: impl AsRef<Path>,
        cols: u16,
        rows: u16,
        scrollback_limit: usize,
    ) -> io::Result<Self> {
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize {
                rows: dimension(rows) as u16,
                cols: dimension(cols) as u16,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(other_io)?;
        let mut command = CommandBuilder::new(&shell.program);
        command.args(&shell.args);
        command.cwd(cwd.as_ref());
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "NEXUL Studio");
        for (key, value) in shell.env {
            command.env(key, value);
        }
        let child = pair.slave.spawn_command(command).map_err(other_io)?;
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().map_err(other_io)?;
        let writer = pair.master.take_writer().map_err(other_io)?;
        let master = Arc::new(Mutex::new(pair.master));
        let writer = Arc::new(Mutex::new(writer));
        let child = Arc::new(Mutex::new(child));
        let state = Arc::new(Mutex::new(ParserState {
            parser: vte::Parser::new(),
            screen: Screen::new(cols, rows, scrollback_limit),
        }));
        let (tx, events) = mpsc::channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let reader_thread = Some(spawn_reader(
            reader,
            state.clone(),
            tx,
            stopped.clone(),
            child.clone(),
        ));
        Ok(Self {
            writer,
            master,
            child,
            state,
            events,
            stopped,
            reader_thread,
        })
    }

    /// Write raw key/input bytes to the child process.
    pub fn write(&self, input: &[u8]) -> io::Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| io::Error::other("terminal input lock poisoned"))?;
        writer.write_all(input)?;
        writer.flush()
    }

    pub fn write_text(&self, input: &str) -> io::Result<()> {
        self.write(input.as_bytes())
    }

    /// Resize both the PTY and its screen model. Parameters are clamped to 1..=1000.
    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        let cols = dimension(cols) as u16;
        let rows = dimension(rows) as u16;
        self.master
            .lock()
            .map_err(|_| io::Error::other("terminal PTY lock poisoned"))?
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(other_io)?;
        self.state
            .lock()
            .map_err(|_| io::Error::other("terminal screen lock poisoned"))?
            .screen
            .resize(cols, rows);
        Ok(())
    }

    pub fn snapshot(&self) -> ScreenSnapshot {
        self.state
            .lock()
            .expect("terminal screen lock poisoned")
            .screen
            .snapshot()
    }

    pub fn set_selection(&self, selection: Option<Selection>) {
        self.state
            .lock()
            .expect("terminal screen lock poisoned")
            .screen
            .set_selection(selection);
    }

    pub fn selected_text(&self) -> Option<String> {
        self.state
            .lock()
            .expect("terminal screen lock poisoned")
            .screen
            .selected_text()
    }

    pub fn try_recv_event(&self) -> Result<TerminalEvent, TryRecvError> {
        self.events.try_recv()
    }

    pub fn recv_event_timeout(&self, timeout: Duration) -> Result<TerminalEvent, RecvTimeoutError> {
        self.events.recv_timeout(timeout)
    }

    pub fn process_id(&self) -> Option<u32> {
        self.child.lock().ok().and_then(|child| child.process_id())
    }

    /// Terminate the shell, wait for it, and join the PTY reader.
    pub fn stop(&mut self) -> io::Result<()> {
        if !self.stopped.load(Ordering::Acquire) {
            let mut child = self
                .child
                .lock()
                .map_err(|_| io::Error::other("terminal child lock poisoned"))?;
            let _ = child.kill();
            let _ = child.wait()?;
        }
        if let Some(handle) = self.reader_thread.take() {
            let _ = handle.join();
        }
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn spawn_reader(
    mut reader: Box<dyn Read + Send>,
    state: Arc<Mutex<ParserState>>,
    events: Sender<TerminalEvent>,
    stopped: Arc<AtomicBool>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("nexul-terminal-reader".into())
        .spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(length) => {
                        let output = buffer[..length].to_vec();
                        let mut state = match state.lock() {
                            Ok(state) => state,
                            Err(_) => break,
                        };
                        let old_title = state.screen.title.clone();
                        let ParserState { parser, screen } = &mut *state;
                        parser.advance(screen, &buffer[..length]);
                        let new_title = state.screen.title.clone();
                        drop(state);
                        let _ = events.send(TerminalEvent::Output(output));
                        if let Some(title) =
                            new_title.filter(|title| old_title.as_ref() != Some(title))
                        {
                            let _ = events.send(TerminalEvent::TitleChanged(title));
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        if !stopped.load(Ordering::Acquire) {
                            let _ = events.send(TerminalEvent::Error(error.to_string()));
                        }
                        break;
                    }
                }
            }
            let exit_code = child
                .lock()
                .ok()
                .and_then(|mut child| child.wait().ok())
                .map(|status| status.exit_code() as i32);
            if !stopped.swap(true, Ordering::AcqRel) {
                let _ = events.send(TerminalEvent::Exited(exit_code));
            }
        })
        .expect("failed to start terminal reader thread")
}

fn other_io(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error.to_string())
}

/// Manages independent live terminals. Removing a terminal stops its process.
#[derive(Default)]
pub struct TerminalManager {
    next_id: u64,
    terminals: BTreeMap<u64, Terminal>,
}

impl TerminalManager {
    pub fn spawn(
        &mut self,
        shell: ShellSpec,
        cwd: impl AsRef<Path>,
        cols: u16,
        rows: u16,
    ) -> io::Result<u64> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.terminals
            .insert(id, Terminal::spawn(shell, cwd, cols, rows)?);
        Ok(id)
    }

    pub fn get(&self, id: u64) -> Option<&Terminal> {
        self.terminals.get(&id)
    }

    pub fn close(&mut self, id: u64) -> io::Result<bool> {
        if let Some(mut terminal) = self.terminals.remove(&id) {
            terminal.stop()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.terminals.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn feed(screen: &mut Screen, text: &str) {
        let mut parser = vte::Parser::new();
        parser.advance(screen, text.as_bytes());
    }

    fn wait_for_exit(terminal: &Terminal, timeout: Duration) -> Option<i32> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            match terminal.recv_event_timeout(Duration::from_millis(100)) {
                Ok(TerminalEvent::Exited(code)) => return code,
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return None,
            }
        }
        None
    }

    #[test]
    fn ansi_colors_cursor_title_and_scrollback_are_parsed() {
        let mut screen = Screen::new(8, 2, 4);
        feed(
            &mut screen,
            "\x1b]0;NEXUL\x07\x1b[31;1mred\x1b[0m\r\nnext\r\n",
        );
        assert_eq!(screen.title.as_deref(), Some("NEXUL"));
        assert_eq!(screen.scrollback.len(), 1);
        assert_eq!(screen.scrollback[0][0].style.foreground, Color::Indexed(1));
        assert!(screen.scrollback[0][0].style.bold);
    }

    #[test]
    fn unicode_accent_cjk_and_emoji_keep_cell_widths() {
        let mut screen = Screen::new(10, 2, 4);
        feed(&mut screen, "e\u{301} 中 🧪");
        assert_eq!(screen.cells[0][0].text, "e\u{301}");
        assert_eq!(screen.cells[0][2].text, "中");
        assert_eq!(screen.cells[0][2].width, 2);
        assert_eq!(screen.cells[0][3].width, 0);
        assert_eq!(screen.cells[0][5].text, "🧪");
        assert_eq!(screen.cells[0][5].width, 2);
    }

    #[test]
    fn resize_changes_grid_and_selection_can_be_copied() {
        let mut screen = Screen::new(8, 2, 4);
        feed(&mut screen, "hello\nworld");
        screen.selection = Some(Selection {
            anchor: CellPosition { row: 0, col: 0 },
            focus: CellPosition { row: 0, col: 4 },
        });
        assert_eq!(screen.selected_text().as_deref(), Some("hello"));
        screen.resize(4, 3);
        assert_eq!((screen.columns, screen.rows), (4, 3));
        assert_eq!(screen.cells.len(), 3);
    }

    #[cfg(windows)]
    fn interactive_shell() -> ShellSpec {
        ShellSpec::powershell()
            .expect("PowerShell is required for the Windows PTY integration tests")
    }

    #[cfg(unix)]
    fn interactive_shell() -> ShellSpec {
        let shell = std::env::var_os("SHELL").unwrap_or_else(|| OsString::from("/bin/sh"));
        ShellSpec::custom(PathBuf::from(shell))
    }

    #[test]
    fn pty_runs_interactive_program_and_exits() {
        let cwd = std::env::current_dir().unwrap();
        let terminal = Terminal::spawn(interactive_shell(), cwd, 80, 24).unwrap();
        #[cfg(windows)]
        terminal
            .write_text("Write-Output 'NEXUL PTY READY'\r\nexit\r\n")
            .unwrap();
        #[cfg(unix)]
        terminal
            .write_text("printf 'NEXUL PTY READY\\n'\nexit\n")
            .unwrap();
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(5)
            && !terminal.snapshot().plain_text().contains("NEXUL PTY READY")
        {
            let _ = terminal.recv_event_timeout(Duration::from_millis(50));
        }
        assert!(terminal.snapshot().plain_text().contains("NEXUL PTY READY"));
        assert_eq!(wait_for_exit(&terminal, Duration::from_secs(5)), Some(0));
    }

    #[test]
    fn pty_resize_and_stop_are_reported() {
        let cwd = std::env::current_dir().unwrap();
        let mut terminal = Terminal::spawn(interactive_shell(), cwd, 80, 24).unwrap();
        terminal.resize(100, 30).unwrap();
        assert_eq!(
            (terminal.snapshot().columns, terminal.snapshot().rows),
            (100, 30)
        );
        terminal.stop().unwrap();
        assert!(terminal.stopped.load(Ordering::Acquire));
        assert!(matches!(
            terminal.recv_event_timeout(Duration::from_secs(1)),
            Ok(TerminalEvent::Exited(_))
        ));
    }
}

//! The window of the installer: a wizard with the controls of Windows, in the layout of
//! the HTML interface it replaces (side bar with the steps, pages, Précédent / Suivant).
//!
//! The model (`model.rs`) holds what is shown and chosen; this file creates the controls
//! of each page, forwards the actions of the user to the model, runs the installation on a
//! thread and follows its progress. The window procedure takes the application out of its
//! cell while it handles a message: a message sent back during that time (a control that
//! notifies its parent, a dialog box that pumps messages) finds no application and gets the
//! default handling, so no state is borrowed twice.

use super::model::{format_size, old_texts, Model, Page};
use super::sys::*;
use crate::win::ffi::{CoCreateInstance, CLSCTX_INPROC_SERVER, GUID, HANDLE, HRESULT, HWND};
use crate::win::{shell, wide, Com};
use crate::{old, Options, Progress, State};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;

// The layout, in pixels at 96 dpi (scaled by the DPI of the screen).
const W: i32 = 860;
const H: i32 = 600;
const SIDE: i32 = 230;
const LEFT: i32 = 266;
const WIDTH: i32 = 558;
const FOOT: i32 = 534;
const OLD_ITEM: i32 = 72;

const ID_BACK: usize = 3;
const ID_ACCEPT: usize = 10;
const ID_BROWSE: usize = 11;
const ID_DIR: usize = 12;
const ID_OPT: usize = 20;
const ID_ALL_REMOVE: usize = 30;
const ID_ALL_BACKUP: usize = 31;
const ID_LAUNCH: usize = 40;
const ID_OPEN_DIR: usize = 41;
const ID_OPEN_BACKUPS: usize = 42;
const ID_COMPONENT: usize = 100;
const ID_OLD: usize = 1000;
const TIMER_PROGRESS: usize = 1;
const TIMER_SELFTEST: usize = 2;
const WM_SCAN_DONE: u32 = WM_APP + 1;

const LICENSE: &str = "Licences\r\n\r\nNEXUL (langage, compilateur, bibliothèque standard, NEXUL Studio, outils) est distribué \
sous licence MIT ou Apache 2.0, au choix.\r\n\r\nComposants tiers inclus : LLVM et LLD (génération de code, édition de liens \
et débogueur) — licence Apache 2.0 avec exceptions LLVM, voir llvm.org/LICENSE.txt ; Python (moteur utilisé par le \
débogueur LLDB) — licence de la Python Software Foundation, fournie avec le composant.\r\n\r\nLE LOGICIEL EST FOURNI « TEL \
QUEL », SANS GARANTIE D'AUCUNE SORTE, EXPLICITE OU IMPLICITE.";

/// The colors of a text and of its background.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Tint {
    Text,
    Muted,
    Faint,
    /// Information (light blue).
    Note,
    /// Warnings (light amber).
    Warn,
    /// Error (light red).
    Error,
    /// A neutral box (the color of the side bar).
    Panel,
}

impl Tint {
    const ALL: [Tint; 7] = [Tint::Text, Tint::Muted, Tint::Faint, Tint::Note, Tint::Warn, Tint::Error, Tint::Panel];

    fn fg(self) -> COLORREF {
        match self {
            Tint::Text | Tint::Panel => TEXT,
            Tint::Muted => rgb(94, 90, 114),
            Tint::Faint => rgb(128, 124, 145),
            Tint::Note => rgb(11, 87, 112),
            Tint::Warn => rgb(122, 75, 0),
            Tint::Error => rgb(155, 28, 49),
        }
    }

    fn bg(self) -> COLORREF {
        match self {
            Tint::Text | Tint::Muted | Tint::Faint => WHITE,
            Tint::Note => rgb(232, 244, 252),
            Tint::Warn => rgb(255, 246, 229),
            Tint::Error => rgb(253, 236, 239),
            Tint::Panel => SIDE_BG,
        }
    }
}

const TEXT: COLORREF = rgb(31, 29, 43);
const WHITE: COLORREF = rgb(255, 255, 255);
const SIDE_BG: COLORREF = rgb(246, 245, 251);
const LINE: COLORREF = rgb(226, 224, 236);
const ACCENT: COLORREF = rgb(124, 77, 255);
const ACTIVE_BG: COLORREF = rgb(237, 231, 255);
const OK: COLORREF = rgb(46, 158, 91);
const OK_BG: COLORREF = rgb(230, 244, 234);
const ERR: COLORREF = rgb(196, 43, 72);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Font {
    Normal,
    Bold,
    Small,
    Title,
    Mono,
    Logo,
    Hero,
    StepNumber,
}

impl Font {
    const ALL: [Font; 8] =
        [Font::Normal, Font::Bold, Font::Small, Font::Title, Font::Mono, Font::Logo, Font::Hero, Font::StepNumber];

    /// Size in points, weight, face.
    fn spec(self) -> (f32, i32, &'static str) {
        match self {
            Font::Normal => (9.0, FW_NORMAL, "Segoe UI"),
            Font::Bold => (9.0, FW_SEMIBOLD, "Segoe UI"),
            Font::Small => (8.25, FW_NORMAL, "Segoe UI"),
            Font::Title => (17.0, FW_SEMIBOLD, "Segoe UI"),
            Font::Mono => (8.25, FW_NORMAL, "Consolas"),
            Font::Logo => (16.0, FW_BOLD, "Segoe UI"),
            Font::Hero => (21.0, FW_BOLD, "Segoe UI"),
            Font::StepNumber => (8.0, FW_BOLD, "Segoe UI"),
        }
    }
}

/// Fonts and brushes for one scale.
struct Style {
    scale: f32,
    fonts: Vec<HANDLE>,
    brushes: Vec<HANDLE>,
    white: HANDLE,
    side: HANDLE,
    line: HANDLE,
    active: HANDLE,
    accent: HANDLE,
}

impl Style {
    fn new(scale: f32) -> Style {
        let fonts = Font::ALL
            .iter()
            .map(|f| {
                let (pt, weight, face) = f.spec();
                let height = -((pt * 96.0 * scale / 72.0).round() as i32);
                unsafe {
                    CreateFontW(
                        height,
                        0,
                        0,
                        0,
                        weight,
                        0,
                        0,
                        0,
                        DEFAULT_CHARSET,
                        0,
                        0,
                        CLEARTYPE_QUALITY,
                        0,
                        wide(face).as_ptr(),
                    )
                }
            })
            .collect();
        let brush = |c| unsafe { CreateSolidBrush(c) };
        Style {
            scale,
            fonts,
            brushes: Tint::ALL.iter().map(|t| brush(t.bg())).collect(),
            white: brush(WHITE),
            side: brush(SIDE_BG),
            line: brush(LINE),
            active: brush(ACTIVE_BG),
            accent: brush(ACCENT),
        }
    }

    fn font(&self, f: Font) -> HANDLE {
        self.fonts[Font::ALL.iter().position(|x| *x == f).unwrap_or(0)]
    }

    fn brush(&self, t: Tint) -> HANDLE {
        self.brushes[Tint::ALL.iter().position(|x| *x == t).unwrap_or(0)]
    }

    fn px(&self, v: i32) -> i32 {
        (v as f32 * self.scale).round() as i32
    }
}

impl Drop for Style {
    fn drop(&mut self) {
        for h in self.fonts.iter().chain(&self.brushes).chain([&self.white, &self.side, &self.line, &self.active, &self.accent]) {
            unsafe { DeleteObject(*h) };
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum StepState {
    Done,
    Active,
    Todo,
}

/// What the window procedure needs without the application: the style, the colors of the
/// controls, and what is painted (side bar, boxes behind texts, the sign of the last page).
struct View {
    style: Option<Rc<Style>>,
    tints: BTreeMap<isize, Tint>,
    version: String,
    steps: Vec<(String, Option<StepState>)>,
    /// Rectangles in pixels of the window.
    boxes: Vec<(RECT, Tint)>,
    /// On the last page: `Some(true)` success, `Some(false)` failure.
    hero: Option<bool>,
}

impl View {
    const fn new() -> View {
        View { style: None, tints: BTreeMap::new(), version: String::new(), steps: Vec::new(), boxes: Vec::new(), hero: None }
    }
}

// The initializers are `const`: Clippy 1.99 still reports them (`missing_const_for_thread_local`).
#[allow(clippy::missing_const_for_thread_local)]
mod cells {
    use super::{App, View};
    use std::cell::{Cell, RefCell};

    thread_local! {
        pub(super) static APP: RefCell<Option<App>> = const { RefCell::new(None) };
        pub(super) static VIEW: RefCell<View> = const { RefCell::new(View::new()) };
        pub(super) static EXIT_CODE: Cell<i32> = const { Cell::new(0) };
    }
}
use cells::{APP, EXIT_CODE, VIEW};

/// Runs `f` on the application, if it is not already in use (see the module comment).
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    let mut app = APP.with(|a| a.borrow_mut().take())?;
    let r = f(&mut app);
    APP.with(|a| *a.borrow_mut() = Some(app));
    Some(r)
}

/// The result of the work, shown on the last page.
#[derive(Clone)]
struct Outcome {
    error: Option<String>,
    warnings: Vec<String>,
    backups: bool,
}

/// `ITaskbarList3` (ShObjIdl_core.h): the progress on the button of the taskbar.
#[repr(C)]
#[allow(non_snake_case)]
struct ITaskbarList3Vtbl {
    base: crate::win::ffi::IUnknownVtbl,
    HrInit: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    _add_tab: usize,
    _delete_tab: usize,
    _activate_tab: usize,
    _set_active_alt: usize,
    _mark_fullscreen: usize,
    SetProgressValue: unsafe extern "system" fn(*mut c_void, HWND, u64, u64) -> HRESULT,
    SetProgressState: unsafe extern "system" fn(*mut c_void, HWND, u32) -> HRESULT,
}

const CLSID_TASKBAR_LIST: GUID =
    GUID { data1: 0x56FD_F344, data2: 0xFD6D, data3: 0x11D0, data4: [0x95, 0x8A, 0x00, 0x60, 0x97, 0xC9, 0xA0, 0x90] };
const IID_ITASKBAR_LIST3: GUID =
    GUID { data1: 0xEA1A_FB91, data2: 0x9E28, data3: 0x4B86, data4: [0x90, 0xE9, 0x9E, 0x9F, 0x8A, 0x5E, 0xEF, 0xAF] };
const TBPF_NOPROGRESS: u32 = 0;
const TBPF_NORMAL: u32 = 2;
const TBPF_ERROR: u32 = 4;

struct Taskbar(shell::ComPtr<ITaskbarList3Vtbl>);

impl Taskbar {
    fn new() -> Option<Taskbar> {
        let mut p = std::ptr::null_mut();
        let hr = unsafe { CoCreateInstance(&CLSID_TASKBAR_LIST, std::ptr::null_mut(), CLSCTX_INPROC_SERVER, &IID_ITASKBAR_LIST3, &mut p) };
        if hr < 0 {
            return None;
        }
        let t = unsafe { shell::ComPtr::<ITaskbarList3Vtbl>::from_raw(p) }?;
        (unsafe { (t.vtbl().HrInit)(t.p) } >= 0).then_some(Taskbar(t))
    }

    fn progress(&self, hwnd: HWND, value: u64, total: u64) {
        unsafe { (self.0.vtbl().SetProgressValue)(self.0.p, hwnd, value, total) };
    }

    fn state(&self, hwnd: HWND, state: u32) {
        unsafe { (self.0.vtbl().SetProgressState)(self.0.p, hwnd, state) };
    }
}

/// The controls of the current page the application uses again.
#[derive(Default)]
struct Controls {
    next: Option<HWND>,
    back: Option<HWND>,
    foot: Option<HWND>,
    accept: Option<HWND>,
    panel: Option<HWND>,
    choices_left: Option<HWND>,
    old: Vec<Vec<(&'static str, HWND)>>,
    components: Vec<HWND>,
    dir: Option<HWND>,
    options: Vec<HWND>,
    summary: Option<HWND>,
    bar: Option<HWND>,
    step: Option<HWND>,
    file: Option<HWND>,
    log: Option<HWND>,
}

pub(super) struct App {
    hwnd: HWND,
    st: Arc<State>,
    model: Model,
    style: Rc<Style>,
    /// The scale imposed by the self-test (`NEXUL_SETUP_UI_SCALE`), if any.
    forced_scale: Option<f32>,
    children: Vec<HWND>,
    c: Controls,
    /// Where the uninstaller found the installation (`Uninstall` page): what, and the
    /// folder.
    found: (String, Option<String>),
    running: bool,
    log_len: usize,
    last_step: String,
    last_file: String,
    outcome: Option<Outcome>,
    taskbar: Option<Taskbar>,
    #[cfg(feature = "test-sandbox")]
    selftest: Option<super::selftest::SelfTest>,
}

fn instance() -> HANDLE {
    unsafe { crate::win::ffi::GetModuleHandleW(std::ptr::null()) }
}

fn set_text(hwnd: HWND, text: &str) {
    unsafe { SetWindowTextW(hwnd, wide(text).as_ptr()) };
}

fn get_text(hwnd: HWND) -> String {
    let n = unsafe { GetWindowTextLengthW(hwnd) }.max(0) as usize;
    let mut buf = vec![0u16; n + 1];
    let got = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..got.min(n)])
}

fn checked(hwnd: HWND) -> bool {
    unsafe { SendMessageW(hwnd, BM_GETCHECK, 0, 0) as usize == BST_CHECKED }
}

fn set_checked(hwnd: HWND, on: bool) {
    unsafe { SendMessageW(hwnd, BM_SETCHECK, on as usize, 0) };
}

fn enable(hwnd: HWND, on: bool) {
    unsafe { EnableWindow(hwnd, on as i32) };
}

impl App {
    fn px(&self, v: i32) -> i32 {
        self.style.px(v)
    }

    /// A child control at `(x, y, w, h)` (logical pixels) of `parent` (the window if
    /// `None`).
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        parent: Option<HWND>,
        class: &str,
        text: &str,
        style: u32,
        ex: u32,
        (x, y, w, h): (i32, i32, i32, i32),
        id: usize,
        font: Font,
        tint: Tint,
    ) -> HWND {
        let hwnd = unsafe {
            crate::win::ffi::CreateWindowExW(
                ex | WS_EX_NOPARENTNOTIFY,
                wide(class).as_ptr(),
                wide(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | style,
                self.px(x),
                self.px(y),
                self.px(w),
                self.px(h),
                parent.unwrap_or(self.hwnd),
                id as HANDLE,
                instance(),
                std::ptr::null_mut(),
            )
        };
        unsafe { SendMessageW(hwnd, WM_SETFONT, self.style.font(font) as usize, 0) };
        VIEW.with(|v| v.borrow_mut().tints.insert(hwnd as isize, tint));
        if parent.is_none() {
            self.children.push(hwnd);
        }
        hwnd
    }

    fn label(&mut self, text: &str, r: (i32, i32, i32, i32), font: Font, tint: Tint) -> HWND {
        self.add(None, "STATIC", text, SS_LEFT | SS_NOPREFIX, 0, r, 0, font, tint)
    }

    fn button(&mut self, text: &str, r: (i32, i32, i32, i32), id: usize, default: bool) -> HWND {
        let kind = if default { BS_DEFPUSHBUTTON } else { BS_PUSHBUTTON };
        self.add(None, "BUTTON", text, kind | WS_TABSTOP, 0, r, id, Font::Normal, Tint::Text)
    }

    fn check(&mut self, text: &str, r: (i32, i32, i32, i32), id: usize, on: bool, enabled: bool, font: Font) -> HWND {
        let disabled = if enabled { 0 } else { WS_DISABLED };
        let h = self.add(None, "BUTTON", text, BS_AUTOCHECKBOX | WS_TABSTOP | disabled, 0, r, id, font, Tint::Text);
        set_checked(h, on);
        h
    }

    fn title(&mut self, text: &str) {
        self.label(text, (LEFT, 26, WIDTH, 36), Font::Title, Tint::Text);
    }

    /// The height (logical pixels) of `text` written in `font` on `w` logical pixels, as a
    /// static control breaks its lines. The fonts do not grow exactly with the scale, so
    /// the texts are measured instead of given a fixed height.
    fn measure(&self, text: &str, font: Font, w: i32) -> i32 {
        let t = wide(text);
        let mut r = RECT { left: 0, top: 0, right: self.px(w), bottom: 0 };
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            let old = SelectObject(dc, self.style.font(font));
            DrawTextW(dc, t.as_ptr(), (t.len() - 1) as i32, &mut r, DT_CALCRECT | DT_WORDBREAK | DT_EXPANDTABS | DT_NOPREFIX);
            SelectObject(dc, old);
            DeleteDC(dc);
        }
        (r.bottom as f32 / self.style.scale).ceil() as i32 + 1
    }

    /// A label as high as its text; returns its bottom (logical pixels).
    fn text(&mut self, text: &str, (x, y, w): (i32, i32, i32), font: Font, tint: Tint) -> i32 {
        let h = self.measure(text, font, w);
        self.label(text, (x, y, w, h), font, tint);
        y + h
    }

    /// The lead paragraph under the title; returns its bottom.
    fn lead(&mut self, text: &str) -> i32 {
        self.text(text, (LEFT, 70, WIDTH), Font::Normal, Tint::Muted)
    }

    /// A text in a colored box (12 pixels of margin, 8 above and below); returns the
    /// bottom of the box.
    fn boxed(&mut self, text: &str, y: i32, font: Font, tint: Tint) -> i32 {
        let h = self.measure(text, font, WIDTH - 24);
        self.paint_box((LEFT, y, WIDTH, h + 16), tint);
        self.label(text, (LEFT + 12, y + 8, WIDTH - 24, h), font, tint);
        y + h + 16
    }

    /// A colored box behind texts, painted by the window.
    fn paint_box(&self, (x, y, w, h): (i32, i32, i32, i32), tint: Tint) {
        let r = RECT { left: self.px(x), top: self.px(y), right: self.px(x + w), bottom: self.px(y + h) };
        VIEW.with(|v| v.borrow_mut().boxes.push((r, tint)));
    }

    /// Destroys the controls of the page and creates those of the current page.
    fn build(&mut self) {
        for h in self.children.drain(..) {
            unsafe { DestroyWindow(h) };
        }
        self.c = Controls::default();
        let pages = self.model.pages();
        let current = pages.iter().position(|p| *p == self.model.page).unwrap_or(0);
        VIEW.with(|v| {
            let mut v = v.borrow_mut();
            v.tints.clear();
            v.boxes.clear();
            v.hero = None;
            v.steps = pages
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let state = match i.cmp(&current) {
                        std::cmp::Ordering::Less => StepState::Done,
                        std::cmp::Ordering::Equal => StepState::Active,
                        std::cmp::Ordering::Greater => StepState::Todo,
                    };
                    (self.model.step_name(*p).to_string(), Some(state))
                })
                .collect();
        });
        match self.model.page {
            Page::Welcome => self.page_welcome(),
            Page::OldVersions => self.page_old(),
            Page::Components => self.page_components(),
            Page::Location => self.page_location(),
            Page::Progress => self.page_progress(),
            Page::Done => self.page_done(),
            Page::Uninstall => self.page_uninstall(),
        }
        // The footer.
        self.c.foot = Some(self.label("", (LEFT, H - 46, WIDTH - 250, 20), Font::Small, Tint::Faint));
        if self.model.can_go_back() {
            self.c.back = Some(self.button("Précédent", (LEFT + WIDTH - 232, H - 50, 112, 30), ID_BACK, false));
        }
        if self.model.page != Page::Progress {
            let label = self.model.next_label();
            self.c.next = Some(self.button(label, (LEFT + WIDTH - 112, H - 50, 112, 30), IDOK, true));
        }
        self.refresh();
        let focus = match self.model.page {
            Page::Location => self.c.dir,
            _ => self.c.next.or(self.c.accept),
        };
        if let Some(f) = focus {
            unsafe { SetFocus(f) };
        }
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 1) };
    }

    fn page_welcome(&mut self) {
        self.title("Bienvenue dans NEXUL");
        let mut y = self.lead(
            "Cet assistant installe le langage NEXUL sur votre ordinateur : le compilateur natif, ses outils et NEXUL \
             Studio, l'environnement de développement officiel.",
        ) + 14;
        // Known once the search of the earlier versions is over: the page is built again then.
        let found = self.model.found_text();
        if !found.is_empty() {
            y = self.boxed(&found, y, Font::Small, Tint::Note) + 16;
        }
        let features = [
            ("Compilateur natif", "nxc compile en code machine optimisé (LLVM) pour Windows, Linux, macOS, WebAssembly et les microcontrôleurs."),
            ("NEXUL Studio", "Éditeur avec complétion, documentation au survol, refactorisation, tests et aperçu web."),
            ("Débogueur", "Points d'arrêt, pas à pas, variables et pile d'appels directement dans Studio."),
            ("Paquets nxpm", "Dépendances locales ou Git, versions figées et vérification d'intégrité SHA-256."),
        ];
        for row in features.chunks(2) {
            let mut bottom = y;
            for (k, (t, d)) in row.iter().enumerate() {
                let x = LEFT + k as i32 * 285;
                self.label(t, (x, y, 273, 18), Font::Bold, Tint::Text);
                bottom = bottom.max(self.text(d, (x, y + 20, 273), Font::Small, Tint::Muted));
            }
            y = bottom + 10;
        }
        let style = ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | WS_VSCROLL | WS_TABSTOP;
        let h = (FOOT - 48 - y).clamp(60, 110);
        self.add(None, "EDIT", LICENSE, style, WS_EX_CLIENTEDGE, (LEFT, y + 4, WIDTH, h), 0, Font::Small, Tint::Muted);
        self.c.accept = Some(self.check(
            "J'accepte les conditions des licences",
            (LEFT, y + h + 14, WIDTH, 22),
            ID_ACCEPT,
            self.model.accepted,
            true,
            Font::Normal,
        ));
    }

    fn page_old(&mut self) {
        self.title("Anciennes versions");
        let y = self.lead(
            "Des versions précédentes de NEXUL ont été trouvées sur cet ordinateur. Choisissez pour chacune de la \
             supprimer ou de la sauvegarder.",
        ) + 10;
        let note = format!(
            "Une sauvegarde est déplacée dans {}. Elle est retirée du PATH et n'a plus de raccourci, d'association de \
             fichiers ni d'inscription dans les applications installées : elle ne gêne pas la nouvelle version. Pour \
             vous en débarrasser plus tard, supprimez simplement son dossier.",
            old::backup_root().display()
        );
        let y = self.boxed(&note, y, Font::Small, Tint::Note) + 12;
        self.c.choices_left = Some(self.label("", (LEFT, y + 6, WIDTH - 280, 18), Font::Small, Tint::Faint));
        self.button("Tout supprimer", (LEFT + WIDTH - 266, y, 124, 28), ID_ALL_REMOVE, false);
        self.button("Tout sauvegarder", (LEFT + WIDTH - 136, y, 136, 28), ID_ALL_BACKUP, false);
        // The list, in a panel that scrolls.
        let (py, ph) = (y + 36, FOOT - y - 36 - 8);
        let panel = self.add(
            None,
            PANEL_CLASS,
            "",
            WS_VSCROLL | WS_CLIPCHILDREN,
            WS_EX_CONTROLPARENT,
            (LEFT, py, WIDTH, ph),
            0,
            Font::Normal,
            Tint::Text,
        );
        self.c.panel = Some(panel);
        let items = self.model.old.clone().unwrap_or_default();
        let inner = WIDTH - 22;
        for (i, c) in items.iter().enumerate() {
            let y = i as i32 * OLD_ITEM;
            let (title, location, details) = old_texts(c);
            let add = |app: &mut App, text: &str, r, font, tint, extra| {
                app.add(Some(panel), "STATIC", text, SS_LEFT | SS_NOPREFIX | extra, 0, r, 0, font, tint)
            };
            add(self, &title, (8, y + 6, inner - 276, 18), Font::Bold, Tint::Text, SS_ENDELLIPSIS);
            add(self, &location, (8, y + 26, inner - 276, 16), Font::Mono, Tint::Faint, SS_PATHELLIPSIS);
            add(self, &details, (8, y + 44, inner - 276, 18), Font::Small, Tint::Muted, SS_ENDELLIPSIS);
            let mut radios = Vec::new();
            for (k, a) in c.choices.iter().enumerate() {
                let group = if k == 0 { WS_GROUP } else { 0 };
                let r = self.add(
                    Some(panel),
                    "BUTTON",
                    super::model::action_name(a),
                    BS_AUTORADIOBUTTON | WS_TABSTOP | group,
                    0,
                    (inner - 240 + k as i32 * 120, y + 22, 116, 22),
                    ID_OLD + 4 * i + k,
                    Font::Normal,
                    Tint::Text,
                );
                radios.push((*a, r));
            }
            self.c.old.push(radios);
            add(self, "", (0, y + OLD_ITEM - 2, inner, 2), Font::Normal, Tint::Text, SS_ETCHEDHORZ);
        }
        let content = self.px(items.len() as i32 * OLD_ITEM);
        let info = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
            nMin: 0,
            nMax: content.max(1) - 1,
            nPage: self.px(ph) as u32,
            nPos: 0,
            nTrackPos: 0,
        };
        unsafe { SetScrollInfo(panel, SB_VERT, &info, 1) };
    }

    fn page_components(&mut self) {
        self.title("Composants");
        let mut y = self.lead(
            "Choisissez ce qui doit être installé. Les éléments indispensables au compilateur sont toujours installés.",
        ) + 12;
        let comps = self.model.components.clone();
        for (i, (c, on)) in comps.iter().enumerate() {
            let name = if c.required { format!("{} (requis)", c.name) } else { c.name.clone() };
            let h = self.check(&name, (LEFT, y, WIDTH - 110, 22), ID_COMPONENT + i, *on, !c.required, Font::Bold);
            self.c.components.push(h);
            let size = format_size(c.size);
            self.add(None, "STATIC", &size, SS_RIGHT | SS_NOPREFIX, 0, (LEFT + WIDTH - 100, y + 3, 100, 18), 0, Font::Small, Tint::Muted);
            y = self.text(&c.description, (LEFT + 20, y + 23, WIDTH - 130), Font::Small, Tint::Muted).max(y + 40) + 14;
        }
    }

    fn page_location(&mut self) {
        self.title("Emplacement et options");
        self.label(
            "NEXUL s'installe pour votre compte utilisateur, sans droits d'administrateur.",
            (LEFT, 70, WIDTH, 20),
            Font::Normal,
            Tint::Muted,
        );
        self.label("Dossier d'installation", (LEFT, 104, WIDTH, 18), Font::Small, Tint::Muted);
        let dir = self.model.dir.clone();
        self.c.dir = Some(self.add(
            None,
            "EDIT",
            &dir,
            ES_AUTOHSCROLL | WS_TABSTOP,
            WS_EX_CLIENTEDGE,
            (LEFT, 124, WIDTH - 124, 28),
            ID_DIR,
            Font::Normal,
            Tint::Text,
        ));
        self.button("Parcourir…", (LEFT + WIDTH - 112, 123, 112, 30), ID_BROWSE, false);
        self.label(
            "Les programmes sont placés dans bin ; LLVM dans llvm ; les exemples dans examples.",
            (LEFT, 160, WIDTH, 18),
            Font::Small,
            Tint::Faint,
        );
        let studio = self.model.studio_options_enabled();
        let options = [
            ("Ajouter au PATH", "Utiliser nxc et nxpm depuis un terminal.", self.model.path, true),
            ("Raccourci sur le Bureau", "Lancer NEXUL Studio d'un double clic.", self.model.desktop && studio, studio),
            ("Menu Démarrer", "NEXUL Studio, le guide et la désinstallation.", self.model.start_menu, true),
            ("Ouvrir les fichiers .nx", "Avec NEXUL Studio, depuis l'Explorateur.", self.model.associate && studio, studio),
        ];
        for (k, (t, d, on, enabled)) in options.iter().enumerate() {
            let (x, y) = (LEFT + (k as i32 % 2) * 285, 198 + (k as i32 / 2) * 62);
            let h = self.check(t, (x, y, 273, 22), ID_OPT + k, *on, *enabled, Font::Bold);
            self.c.options.push(h);
            self.label(d, (x + 20, y + 23, 253, 34), Font::Small, Tint::Muted);
        }
        self.c.summary = Some(self.label("", (LEFT, 330, WIDTH, 38), Font::Normal, Tint::Muted));
    }

    fn page_progress(&mut self) {
        let (title, lead) = if self.model.uninstall {
            ("Désinstallation en cours", "Les fichiers de NEXUL sont supprimés.")
        } else {
            ("Installation en cours", "Les fichiers sont décompressés et vérifiés un par un.")
        };
        self.title(title);
        self.label(lead, (LEFT, 70, WIDTH, 20), Font::Normal, Tint::Muted);
        let bar = self.add(None, "msctls_progress32", "", 0, 0, (LEFT, 112, WIDTH, 14), 0, Font::Normal, Tint::Text);
        unsafe { SendMessageW(bar, PBM_SETRANGE32, 0, 1000) };
        self.c.bar = Some(bar);
        self.c.step = Some(self.label("Préparation…", (LEFT, 140, WIDTH, 20), Font::Bold, Tint::Text));
        let file =
            self.add(None, "STATIC", "", SS_LEFT | SS_NOPREFIX | SS_PATHELLIPSIS, 0, (LEFT, 162, WIDTH, 16), 0, Font::Mono, Tint::Faint);
        self.c.file = Some(file);
        let style = LBS_NOSEL | LBS_NOINTEGRALHEIGHT | WS_VSCROLL;
        self.c.log = Some(self.add(None, "LISTBOX", "", style, WS_EX_CLIENTEDGE, (LEFT, 190, WIDTH, 330), 0, Font::Small, Tint::Muted));
        self.log_len = 0;
        self.last_step.clear();
        self.last_file.clear();
    }

    fn page_done(&mut self) {
        // Kept: the page is built again if the DPI changes.
        let outcome = self.outcome.clone().unwrap_or(Outcome { error: None, warnings: Vec::new(), backups: false });
        let ok = outcome.error.is_none();
        VIEW.with(|v| v.borrow_mut().hero = Some(ok));
        let un = self.model.uninstall;
        let title = match (ok, un) {
            (true, false) => "NEXUL est installé",
            (true, true) => "NEXUL a été désinstallé",
            (false, false) => "L'installation a échoué",
            (false, true) => "La désinstallation a échoué",
        };
        self.label(title, (LEFT + 76, 30, WIDTH - 76, 36), Font::Title, Tint::Text);
        let dir = self.st.installed_dir.lock().unwrap().clone();
        let sub = match (ok, un) {
            (true, false) => {
                format!("Version {} — {}", self.model.version, dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default())
            }
            (true, true) => "Merci d'avoir essayé NEXUL.".into(),
            _ => String::new(),
        };
        let style = SS_LEFT | SS_NOPREFIX | SS_PATHELLIPSIS;
        self.add(None, "STATIC", &sub, style, 0, (LEFT + 76, 66, WIDTH - 76, 20), 0, Font::Normal, Tint::Muted);
        let mut y = 112;
        if let Some(e) = &outcome.error {
            self.paint_box((LEFT, y, WIDTH, 170), Tint::Error);
            let style = ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | WS_VSCROLL;
            self.add(None, "EDIT", &e.replace('\n', "\r\n"), style, 0, (LEFT + 10, y + 8, WIDTH - 20, 154), 0, Font::Normal, Tint::Error);
            y += 182;
        } else if !un {
            let studio = self.model.is_chosen("studio");
            let samples = self.model.is_chosen("samples");
            let mut tips = String::new();
            match (studio, samples) {
                (true, true) => tips.push_str(
                    "Lancez NEXUL Studio depuis le Bureau ou le menu Démarrer : les exemples s'ouvrent pour commencer.\r\n",
                ),
                (true, false) => tips.push_str("Lancez NEXUL Studio depuis le Bureau ou le menu Démarrer.\r\n"),
                _ => {}
            }
            if samples {
                tips.push_str("Dans un nouveau terminal : nxc run bonjour.nx · nxc test calcul.nx · nxpm new mon_projet\r\n");
            } else {
                tips.push_str("Dans un nouveau terminal : nxpm new mon_projet\r\n");
            }
            tips.push_str("Le guide du langage est dans docs\\guide.md ; nxc explain E0302 explique une erreur.");
            y = self.boxed(&tips, y, Font::Small, Tint::Panel) + 16;
            let mut x = LEFT;
            if studio {
                self.button("Lancer NEXUL Studio", (x, y, 168, 30), ID_LAUNCH, false);
                x += 178;
            }
            self.button("Ouvrir le dossier d'installation", (x, y, 214, 30), ID_OPEN_DIR, false);
            x += 224;
            if outcome.backups {
                self.button("Ouvrir les sauvegardes", (x, y, 156, 30), ID_OPEN_BACKUPS, false);
            }
            y += 46;
        }
        if !outcome.warnings.is_empty() {
            let text: Vec<String> = outcome.warnings.iter().map(|w| format!("! {w}")).collect();
            let text = text.join("\r\n");
            let room = FOOT - 12 - y;
            if self.measure(&text, Font::Small, WIDTH - 24) + 16 <= room {
                self.boxed(&text, y, Font::Small, Tint::Warn);
            } else {
                // Too many to be shown: a box that scrolls.
                self.paint_box((LEFT, y, WIDTH, room), Tint::Warn);
                let style = ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | WS_VSCROLL;
                self.add(None, "EDIT", &text, style, 0, (LEFT + 10, y + 8, WIDTH - 20, room - 16), 0, Font::Small, Tint::Warn);
            }
        }
    }

    fn page_uninstall(&mut self) {
        self.title("Désinstaller NEXUL");
        let y = self.lead(
            "Le compilateur, NEXUL Studio, la chaîne LLVM, les raccourcis, l'association des fichiers .nx et l'entrée \
             du PATH seront supprimés.\r\nVos projets et vos réglages ne sont pas touchés.",
        ) + 14;
        let (what, dir) = self.found.clone();
        let h = if dir.is_some() { 44 } else { 22 };
        self.paint_box((LEFT, y, WIDTH, h + 20), Tint::Panel);
        self.label(&what, (LEFT + 12, y + 10, WIDTH - 24, 20), Font::Normal, Tint::Panel);
        if let Some(d) = dir {
            let style = SS_LEFT | SS_NOPREFIX | SS_PATHELLIPSIS;
            self.add(None, "STATIC", &d, style, 0, (LEFT + 12, y + 32, WIDTH - 24, 18), 0, Font::Mono, Tint::Panel);
        }
    }

    /// Updates the controls of the page from the model, without recreating them.
    fn refresh(&mut self) {
        let m = &self.model;
        if let Some(next) = self.c.next {
            enable(next, m.can_go_next());
        }
        let foot = match m.page {
            Page::Welcome if m.old.is_none() => "Recherche des versions précédentes…".to_string(),
            Page::Components | Page::Location => m.space_text(),
            _ => String::new(),
        };
        if let Some(f) = self.c.foot {
            set_text(f, &foot);
        }
        if let Some(l) = self.c.choices_left {
            set_text(l, &m.choices_left_text());
        }
        for (i, radios) in self.c.old.iter().enumerate() {
            let choice = m.old.as_ref().and_then(|o| o.get(i)).and_then(|c| c.choice);
            for (a, h) in radios {
                set_checked(*h, choice == Some(*a));
            }
        }
        for (i, h) in self.c.components.iter().enumerate() {
            if let Some((_, on)) = m.components.get(i) {
                set_checked(*h, *on);
            }
        }
        if self.c.options.len() == 4 {
            let studio = m.studio_options_enabled();
            let values = [m.path, m.desktop && studio, m.start_menu, m.associate && studio];
            for (k, h) in self.c.options.iter().enumerate() {
                set_checked(*h, values[k]);
                if k == 1 || k == 3 {
                    enable(*h, studio);
                }
            }
        }
        if let Some(s) = self.c.summary {
            set_text(s, &m.summary_text());
        }
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 1) };
    }

    fn message(&self, text: &str, icon: u32) {
        let caption = if self.model.uninstall { "Désinstallation de NEXUL" } else { "Installation de NEXUL" };
        unsafe { MessageBoxW(self.hwnd, wide(text).as_ptr(), wide(caption).as_ptr(), MB_OK | icon) };
    }

    fn start(&mut self, work: impl FnOnce(&State) -> Result<(), String> + Send + 'static) {
        *self.st.progress.lock().unwrap() = Progress { running: true, step: "Préparation".into(), ..Default::default() };
        let st = self.st.clone();
        std::thread::spawn(move || {
            let r = work(&st);
            crate::set_progress(&st, |p| {
                p.running = false;
                p.done = true;
                if let Err(e) = r {
                    p.error = Some(e);
                }
            });
        });
        self.running = true;
        self.model.page = Page::Progress;
        self.build();
        unsafe { SetTimer(self.hwnd, TIMER_PROGRESS, 100, std::ptr::null()) };
        if let Some(t) = &self.taskbar {
            t.state(self.hwnd, TBPF_NORMAL);
        }
    }

    fn start_install(&mut self) {
        let c = match self.model.choices() {
            Ok(c) => c,
            Err(e) => return self.message(&e, MB_ICONWARNING),
        };
        let o = Options {
            dir: PathBuf::from(&c.dir),
            components: c.components,
            path: c.path,
            desktop: c.desktop,
            start_menu: c.start_menu,
            associate: c.associate,
            old: c.old,
        };
        self.start(move |st| crate::install(st, &o));
    }

    fn start_uninstall(&mut self) {
        let dir = self.st.exe.parent().map(Path::to_path_buf).filter(|p| p.join("install.txt").is_file()).or_else(crate::existing_install);
        match dir {
            Some(d) => self.start(move |st| crate::uninstall(st, &d)),
            None => self.message("Aucune installation de NEXUL n'a été trouvée.", MB_ICONERROR),
        }
    }

    fn on_timer(&mut self) {
        let p = self.st.progress.lock().unwrap().clone();
        if let Some(bar) = self.c.bar {
            unsafe { SendMessageW(bar, PBM_SETPOS, (p.percent * 10.0) as usize, 0) };
        }
        if let Some(t) = &self.taskbar {
            t.progress(self.hwnd, (p.percent * 10.0) as u64, 1000);
        }
        if let (Some(h), true) = (self.c.step, p.step != self.last_step) {
            set_text(h, &p.step);
            self.last_step = p.step.clone();
        }
        if let (Some(h), true) = (self.c.file, p.file != self.last_file) {
            set_text(h, &p.file);
            self.last_file = p.file.clone();
        }
        if let Some(log) = self.c.log {
            for line in p.log.iter().skip(self.log_len) {
                unsafe { SendMessageW(log, LB_ADDSTRING, 0, wide(format!("✓ {line}")).as_ptr() as isize) };
            }
            if p.log.len() > self.log_len {
                self.log_len = p.log.len();
                let n = unsafe { SendMessageW(log, LB_GETCOUNT, 0, 0) };
                unsafe { SendMessageW(log, LB_SETTOPINDEX, (n.max(1) - 1) as usize, 0) };
            }
        }
        if p.done {
            unsafe { KillTimer(self.hwnd, TIMER_PROGRESS) };
            self.running = false;
            if let Some(t) = &self.taskbar {
                t.state(self.hwnd, if p.error.is_some() { TBPF_ERROR } else { TBPF_NOPROGRESS });
            }
            self.outcome = Some(Outcome { error: p.error, warnings: p.warnings, backups: p.backups });
            self.model.page = Page::Done;
            self.build();
        }
    }

    fn close(&mut self) {
        if self.running {
            self.message("Le travail est en cours : attendez qu'il se termine.", MB_ICONWARNING);
            return;
        }
        unsafe { DestroyWindow(self.hwnd) };
    }

    fn on_command(&mut self, id: usize, code: u32, ctl: HWND) {
        match id {
            IDOK => match self.model.page {
                Page::Welcome | Page::OldVersions | Page::Components => {
                    if self.model.can_go_next() {
                        self.model.next();
                        self.build();
                    }
                }
                Page::Location => self.start_install(),
                Page::Uninstall => self.start_uninstall(),
                Page::Done => self.close(),
                Page::Progress => {}
            },
            IDCANCEL => self.close(),
            ID_BACK => {
                self.model.back();
                self.build();
            }
            ID_ACCEPT => {
                self.model.accepted = checked(ctl);
                self.refresh();
            }
            ID_DIR if code == EN_CHANGE => self.model.dir = get_text(ctl),
            ID_BROWSE => {
                let start = PathBuf::from(&self.model.dir);
                if let Some(p) = shell::pick_folder_for(self.hwnd, "Dossier d'installation de NEXUL", &start) {
                    let named = p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("NEXUL"));
                    let p = if named { p } else { p.join("NEXUL") };
                    self.model.dir = p.display().to_string();
                    if let Some(d) = self.c.dir {
                        set_text(d, &self.model.dir);
                    }
                }
            }
            x if (ID_OPT..ID_OPT + 4).contains(&x) => {
                let on = checked(ctl);
                match x - ID_OPT {
                    0 => self.model.path = on,
                    1 => self.model.desktop = on,
                    2 => self.model.start_menu = on,
                    _ => self.model.associate = on,
                }
                self.refresh();
            }
            ID_ALL_REMOVE | ID_ALL_BACKUP => {
                self.model.choose_all(if id == ID_ALL_REMOVE { "remove" } else { "backup" });
                self.refresh();
            }
            ID_LAUNCH => {
                if let Some(dir) = self.st.installed_dir.lock().unwrap().clone() {
                    let mut cmd = Command::new(dir.join("bin").join("NEXUL Studio.exe"));
                    if dir.join("examples").is_dir() {
                        cmd.arg(dir.join("examples"));
                    }
                    let _ = cmd.spawn();
                }
                self.close();
            }
            ID_OPEN_DIR => {
                if let Some(dir) = self.st.installed_dir.lock().unwrap().clone() {
                    let _ = Command::new("explorer").arg(dir).spawn();
                }
            }
            ID_OPEN_BACKUPS => {
                let _ = Command::new("explorer").arg(old::backup_root()).spawn();
            }
            x if (ID_COMPONENT..ID_COMPONENT + 100).contains(&x) => {
                if let Some((c, _)) = self.model.components.get(x - ID_COMPONENT) {
                    let cid = c.id.clone();
                    self.model.toggle(&cid);
                }
                self.refresh();
            }
            x if x >= ID_OLD => {
                let (i, k) = ((x - ID_OLD) / 4, (x - ID_OLD) % 4);
                let action = self.model.old.as_ref().and_then(|o| o.get(i)).and_then(|c| c.choices.get(k).copied());
                if let Some(a) = action {
                    self.model.choose(i, a);
                }
                self.refresh();
            }
            _ => {}
        }
    }

    fn on_scan_done(&mut self) {
        let items = self.st.scan.lock().unwrap().as_ref().map(|s| s.items.clone()).unwrap_or_default();
        self.model.set_old(&items);
        if self.model.page == Page::Welcome {
            self.build();
        }
    }

    fn on_dpi(&mut self, dpi: u32, suggested: &RECT) {
        let scale = self.forced_scale.unwrap_or(dpi as f32 / 96.0);
        unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                suggested.left,
                suggested.top,
                suggested.right - suggested.left,
                suggested.bottom - suggested.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        self.style = Rc::new(Style::new(scale));
        VIEW.with(|v| v.borrow_mut().style = Some(self.style.clone()));
        self.build();
    }
}

// ---------------------------------------------------------------------------------------
// Painting.

fn text_out(hdc: HANDLE, text: &str, r: RECT, font: HANDLE, color: COLORREF, format: u32) {
    let w = wide(text);
    let mut r = r;
    unsafe {
        let old = SelectObject(hdc, font);
        SetTextColor(hdc, color);
        SetBkMode(hdc, TRANSPARENT);
        DrawTextW(hdc, w.as_ptr(), (w.len() - 1) as i32, &mut r, format | DT_NOPREFIX);
        SelectObject(hdc, old);
    }
}

fn circle(hdc: HANDLE, r: RECT, fill: Option<COLORREF>, outline: Option<COLORREF>, width: i32) {
    unsafe {
        let brush = fill.map(|c| CreateSolidBrush(c));
        let pen = outline.map(|c| CreatePen(PS_SOLID, width, c));
        let ob = SelectObject(hdc, brush.unwrap_or_else(|| GetStockObject(NULL_BRUSH)));
        let op = SelectObject(hdc, pen.unwrap_or_else(|| GetStockObject(NULL_PEN)));
        Ellipse(hdc, r.left, r.top, r.right, r.bottom);
        SelectObject(hdc, ob);
        SelectObject(hdc, op);
        for h in brush.into_iter().chain(pen) {
            DeleteObject(h);
        }
    }
}

fn paint(hwnd: HWND, hdc: HANDLE) {
    VIEW.with(|v| {
        let v = v.borrow();
        let Some(s) = v.style.clone() else { return };
        let p = |x: i32| s.px(x);
        let rect = |l, t, r, b| RECT { left: p(l), top: p(t), right: p(r), bottom: p(b) };
        let mut client = RECT::default();
        unsafe {
            GetClientRect(hwnd, &mut client);
            FillRect(hdc, &client, s.white);
            // The side bar.
            FillRect(hdc, &RECT { left: 0, top: 0, right: p(SIDE), bottom: client.bottom }, s.side);
            FillRect(hdc, &RECT { left: p(SIDE), top: 0, right: p(SIDE) + 1, bottom: client.bottom }, s.line);
            // The mark and the name.
            let ob = SelectObject(hdc, s.accent);
            let op = SelectObject(hdc, GetStockObject(NULL_PEN));
            RoundRect(hdc, p(24), p(26), p(66), p(68), p(12), p(12));
            SelectObject(hdc, ob);
            SelectObject(hdc, op);
        }
        text_out(hdc, "N", rect(24, 26, 66, 68), s.font(Font::Logo), WHITE, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        text_out(hdc, "NEXUL", rect(78, 28, SIDE - 12, 48), s.font(Font::Bold), TEXT, DT_SINGLELINE);
        text_out(hdc, &v.version, rect(78, 48, SIDE - 12, 66), s.font(Font::Small), Tint::Muted.fg(), DT_SINGLELINE);
        // The steps.
        for (i, (name, state)) in v.steps.iter().enumerate() {
            let y = 100 + i as i32 * 40;
            let state = state.unwrap_or(StepState::Todo);
            if state == StepState::Active {
                unsafe {
                    let ob = SelectObject(hdc, s.active);
                    let op = SelectObject(hdc, GetStockObject(NULL_PEN));
                    RoundRect(hdc, p(12), p(y), p(SIDE - 12), p(y + 36), p(10), p(10));
                    SelectObject(hdc, ob);
                    SelectObject(hdc, op);
                }
            }
            let c = rect(26, y + 7, 48, y + 29);
            let (number, color) = match state {
                StepState::Active => {
                    circle(hdc, c, Some(ACCENT), None, 0);
                    ((i + 1).to_string(), WHITE)
                }
                StepState::Done => {
                    circle(hdc, c, Some(SIDE_BG), Some(OK), p(2).max(1));
                    ("✓".to_string(), OK)
                }
                StepState::Todo => {
                    circle(hdc, c, Some(SIDE_BG), Some(rgb(201, 198, 214)), p(2).max(1));
                    ((i + 1).to_string(), Tint::Faint.fg())
                }
            };
            text_out(hdc, &number, c, s.font(Font::StepNumber), color, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            let (font, color) = match state {
                StepState::Active => (Font::Bold, TEXT),
                StepState::Done => (Font::Normal, TEXT),
                StepState::Todo => (Font::Normal, Tint::Muted.fg()),
            };
            text_out(hdc, name, rect(58, y, SIDE - 14, y + 36), s.font(font), color, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
        }
        text_out(
            hdc,
            "Langage NEXUL, compilateur, bibliothèque standard, outils et NEXUL Studio.\nLicence MIT ou Apache 2.0.",
            rect(24, H - 100, SIDE - 20, H - 20),
            s.font(Font::Small),
            Tint::Faint.fg(),
            DT_WORDBREAK,
        );
        // Boxes behind the texts, the line above the buttons, the sign of the last page.
        for (r, t) in &v.boxes {
            unsafe { FillRect(hdc, r, s.brush(*t)) };
        }
        unsafe { FillRect(hdc, &rect(SIDE + 1, FOOT, W, FOOT + 1), s.line) };
        if let Some(ok) = v.hero {
            let c = rect(LEFT, 26, LEFT + 60, 86);
            circle(hdc, c, Some(if ok { OK_BG } else { Tint::Error.bg() }), None, 0);
            text_out(hdc, if ok { "✓" } else { "!" }, c, s.font(Font::Hero), if ok { OK } else { ERR }, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        }
    });
}

/// The colors of a control, from its tint.
fn control_color(hdc: HANDLE, control: HWND) -> isize {
    VIEW.with(|v| {
        let v = v.borrow();
        let tint = v.tints.get(&(control as isize)).copied().unwrap_or(Tint::Text);
        let Some(s) = &v.style else { return 0 };
        unsafe {
            SetTextColor(hdc, tint.fg());
            SetBkColor(hdc, tint.bg());
        }
        s.brush(tint) as isize
    })
}

// ---------------------------------------------------------------------------------------
// Window procedures.

const MAIN_CLASS: &str = "NexulSetupWindow";
const PANEL_CLASS: &str = "NexulSetupPanel";
const WM_PRINTCLIENT: u32 = 0x0318;

unsafe extern "system" fn main_proc(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            paint(hwnd, hdc);
            EndPaint(hwnd, &ps);
            0
        }
        WM_PRINTCLIENT => {
            paint(hwnd, wp as HANDLE);
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => control_color(wp as HANDLE, lp as HWND),
        DM_GETDEFID => with_app(|a| match a.c.next {
            Some(n) if IsWindowEnabled(n) != 0 => ((DC_HASDEFID as isize) << 16) | IDOK as isize,
            _ => 0,
        })
        .unwrap_or(0),
        WM_COMMAND => {
            let (id, code) = (wp & 0xFFFF, (wp >> 16) as u32);
            with_app(|a| a.on_command(id, code, lp as HWND));
            0
        }
        WM_TIMER => {
            match wp {
                TIMER_PROGRESS => {
                    with_app(|a| a.on_timer());
                }
                // The self-test decides with the application, then acts without it: the
                // clicks it sends come back here as commands.
                #[cfg(feature = "test-sandbox")]
                TIMER_SELFTEST => {
                    let acts = with_app(super::selftest::plan).unwrap_or_default();
                    super::selftest::perform(hwnd, acts);
                }
                _ => {}
            }
            0
        }
        WM_SCAN_DONE => {
            with_app(|a| a.on_scan_done());
            0
        }
        WM_DPICHANGED => {
            let rect = *(lp as *const RECT);
            with_app(|a| a.on_dpi((wp >> 16) as u32 & 0xFFFF, &rect));
            0
        }
        WM_CLOSE => {
            with_app(|a| a.close());
            0
        }
        WM_DESTROY => {
            PostQuitMessage(EXIT_CODE.with(|c| c.get()));
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// The panel that scrolls the list of the earlier versions.
unsafe extern "system" fn panel_proc(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    match msg {
        WM_COMMAND | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => SendMessageW(GetParent(hwnd), msg, wp, lp),
        WM_VSCROLL | WM_MOUSEWHEEL => {
            let mut si = SCROLLINFO { cbSize: std::mem::size_of::<SCROLLINFO>() as u32, fMask: SIF_RANGE | SIF_PAGE | SIF_POS | SIF_TRACKPOS, ..Default::default() };
            GetScrollInfo(hwnd, SB_VERT, &mut si);
            let line = 24;
            let max = (si.nMax - si.nPage as i32 + 1).max(0);
            let pos = if msg == WM_MOUSEWHEEL {
                let delta = ((wp >> 16) & 0xFFFF) as u16 as i16 as i32;
                si.nPos - delta * 3 * line / 120
            } else {
                match (wp & 0xFFFF) as u32 {
                    SB_LINEUP => si.nPos - line,
                    SB_LINEDOWN => si.nPos + line,
                    SB_PAGEUP => si.nPos - si.nPage as i32,
                    SB_PAGEDOWN => si.nPos + si.nPage as i32,
                    SB_THUMBTRACK => si.nTrackPos,
                    SB_TOP => 0,
                    SB_BOTTOM => max,
                    _ => si.nPos,
                }
            }
            .clamp(0, max);
            if pos != si.nPos {
                ScrollWindowEx(
                    hwnd,
                    0,
                    si.nPos - pos,
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    SW_SCROLLCHILDREN | SW_INVALIDATE | SW_ERASE,
                );
                si.fMask = SIF_POS;
                si.nPos = pos;
                SetScrollInfo(hwnd, SB_VERT, &si, 1);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

// ---------------------------------------------------------------------------------------
// Start.

/// DPI per monitor, and the controls of Windows in their current look (`comctl32` version 6,
/// through an activation context: the program does not need a manifest for it).
fn prepare() {
    unsafe {
        if let Some(f) = lookup("user32.dll", "SetProcessDpiAwarenessContext") {
            let f: unsafe extern "system" fn(isize) -> i32 = std::mem::transmute(f);
            f(PER_MONITOR_AWARE_V2);
        } else if let Some(f) = lookup("user32.dll", "SetProcessDPIAware") {
            let f: unsafe extern "system" fn() -> i32 = std::mem::transmute(f);
            f();
        }
        let manifest = std::env::temp_dir().join(format!("nexul-setup-{}.manifest", std::process::id()));
        let text = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/></dependentAssembly></dependency>
</assembly>"#;
        if std::fs::write(&manifest, text).is_ok() {
            let source = wide(&manifest);
            let ctx = ACTCTXW {
                cbSize: std::mem::size_of::<ACTCTXW>() as u32,
                dwFlags: 0,
                lpSource: source.as_ptr(),
                wProcessorArchitecture: 0,
                wLangId: 0,
                lpAssemblyDirectory: std::ptr::null(),
                lpResourceName: std::ptr::null(),
                lpApplicationName: std::ptr::null(),
                hModule: std::ptr::null_mut(),
            };
            let handle = CreateActCtxW(&ctx);
            if !handle.is_null() && handle != crate::win::ffi::INVALID_HANDLE_VALUE {
                let mut cookie = 0usize;
                ActivateActCtx(handle, &mut cookie); // for the whole life of the program
            }
            let _ = std::fs::remove_file(&manifest);
        }
        if let Some(f) = lookup("comctl32.dll", "InitCommonControlsEx") {
            let f: unsafe extern "system" fn(*const INITCOMMONCONTROLSEX) -> i32 = std::mem::transmute(f);
            let icc = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_STANDARD_CLASSES | ICC_PROGRESS_CLASS,
            };
            f(&icc);
        }
    }
}

fn dpi_of(hwnd: HWND) -> u32 {
    unsafe {
        if !hwnd.is_null() {
            if let Some(f) = lookup("user32.dll", "GetDpiForWindow") {
                let f: unsafe extern "system" fn(HWND) -> u32 = std::mem::transmute(f);
                let d = f(hwnd);
                if d > 0 {
                    return d;
                }
            }
        }
        if let Some(f) = lookup("user32.dll", "GetDpiForSystem") {
            let f: unsafe extern "system" fn() -> u32 = std::mem::transmute(f);
            return f().max(96);
        }
    }
    96
}

/// An icon of the `.ico` file embedded in the program, near `size` pixels.
fn icon(size: i32) -> HANDLE {
    let ico = crate::ICON;
    let count = ico.get(4..6).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let mut best: Option<(i32, usize, usize)> = None;
    for i in 0..count {
        let Some(e) = ico.get(6 + 16 * i..6 + 16 * i + 16) else { break };
        let w = if e[0] == 0 { 256 } else { e[0] as i32 };
        let len = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
        let off = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
        let better = match best {
            None => true,
            Some((bw, ..)) => (w >= size && (bw < size || w < bw)) || (bw < size && w > bw),
        };
        if better {
            best = Some((w, off, len));
        }
    }
    match best {
        Some((_, off, len)) if off + len <= ico.len() => unsafe {
            CreateIconFromResourceEx(ico[off..].as_ptr(), len as u32, 1, 0x0003_0000, size, size, 0)
        },
        _ => std::ptr::null_mut(),
    }
}

/// What the uninstaller shows about the installation it removes, and its folder.
fn found_text(st: &State) -> (String, Option<String>) {
    let dir = st.exe.parent().map(Path::to_path_buf).filter(|p| p.join("install.txt").is_file()).or_else(crate::existing_install);
    match dir {
        Some(d) => {
            let version = std::fs::read_to_string(d.join("install.txt"))
                .ok()
                .and_then(|t| t.lines().next().and_then(|l| l.strip_prefix("version\t")).map(str::to_string))
                .unwrap_or_else(|| "?".into());
            (format!("Installation trouvée (version {version}) :"), Some(d.display().to_string()))
        }
        None => ("Installation trouvée dans le dossier de ce programme.".into(), None),
    }
}

/// Shows the wizard and returns the exit code of the program. With `selftest` (tests
/// only), the window is placed out of the screen and driven by `selftest.rs`.
pub fn run(st: Arc<State>, selftest: Option<PathBuf>) -> i32 {
    let _com = Com::init();
    prepare();
    #[cfg(not(feature = "test-sandbox"))]
    let _ = &selftest;
    let forced_scale: Option<f32> = if cfg!(feature = "test-sandbox") {
        std::env::var("NEXUL_SETUP_UI_SCALE").ok().and_then(|s| s.parse().ok())
    } else {
        None
    };
    let scale = forced_scale.unwrap_or(dpi_of(std::ptr::null_mut()) as f32 / 96.0);
    let style = Rc::new(Style::new(scale));
    let version = st.manifest.as_ref().map(|m| m.version.clone()).unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    VIEW.with(|v| {
        let mut v = v.borrow_mut();
        v.style = Some(style.clone());
        v.version = format!("Version {version}");
    });
    let components = st.manifest.as_ref().map(|m| m.components.clone()).unwrap_or_default();
    let dir = crate::existing_install().unwrap_or_else(crate::default_dir).display().to_string();
    let model = Model::new(st.uninstall_mode, &version, &components, &dir);
    unsafe {
        let cursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW as *const u16);
        let (big, small) = (icon(style.px(32)), icon(style.px(16)));
        for (name, proc_) in [(MAIN_CLASS, main_proc as WNDPROC), (PANEL_CLASS, panel_proc as WNDPROC)] {
            let class = wide(name);
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: proc_,
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance(),
                hIcon: if name == MAIN_CLASS { big } else { std::ptr::null_mut() },
                hCursor: cursor,
                hbrBackground: if name == PANEL_CLASS { style.white } else { std::ptr::null_mut() },
                lpszMenuName: std::ptr::null(),
                lpszClassName: class.as_ptr(),
                hIconSm: if name == MAIN_CLASS { small } else { std::ptr::null_mut() },
            };
            RegisterClassExW(&wc);
        }
        let ws = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
        let mut r = RECT { left: 0, top: 0, right: style.px(W), bottom: style.px(H) };
        AdjustWindowRectEx(&mut r, ws, 0, WS_EX_CONTROLPARENT);
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        let (screen_w, screen_h) = (crate::win::ffi::GetSystemMetrics(0), crate::win::ffi::GetSystemMetrics(1));
        let (x, y) = if selftest.is_some() {
            // Left of every screen: the self-test does not appear.
            (crate::win::ffi::GetSystemMetrics(76) - w - 100, 0)
        } else {
            ((screen_w - w).max(0) / 2, (screen_h - h).max(0) / 2)
        };
        let title = if st.uninstall_mode { "Désinstallation de NEXUL" } else { "Installation de NEXUL" };
        let hwnd = crate::win::ffi::CreateWindowExW(
            WS_EX_CONTROLPARENT,
            wide(MAIN_CLASS).as_ptr(),
            wide(title).as_ptr(),
            ws,
            x,
            y,
            w,
            h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance(),
            std::ptr::null_mut(),
        );
        if hwnd.is_null() {
            return 1;
        }
        let found = if st.uninstall_mode { found_text(&st) } else { (String::new(), None) };
        let mut app = App {
            hwnd,
            st: st.clone(),
            model,
            style,
            forced_scale,
            children: Vec::new(),
            c: Controls::default(),
            found,
            running: false,
            log_len: 0,
            last_step: String::new(),
            last_file: String::new(),
            outcome: None,
            taskbar: None,
            #[cfg(feature = "test-sandbox")]
            selftest: selftest.map(super::selftest::SelfTest::new),
        };
        // The DPI of the screen of the window, if it differs from the one of the system.
        let dpi = dpi_of(hwnd);
        if app.forced_scale.is_none() && (dpi as f32 / 96.0 - app.style.scale).abs() > 0.01 {
            app.style = Rc::new(Style::new(dpi as f32 / 96.0));
            VIEW.with(|v| v.borrow_mut().style = Some(app.style.clone()));
        }
        app.build();
        ShowWindow(hwnd, if app_is_selftest(&app) { SW_SHOWNOACTIVATE } else { SW_SHOW });
        UpdateWindow(hwnd);
        app.taskbar = Taskbar::new();
        if app_is_selftest(&app) {
            SetTimer(hwnd, TIMER_SELFTEST, 150, std::ptr::null());
        }
        APP.with(|a| *a.borrow_mut() = Some(app));
        // The earlier versions are looked for while the welcome page is read.
        if !st.uninstall_mode {
            let (st, target) = (st.clone(), hwnd as isize);
            std::thread::spawn(move || {
                let scan = old::scan();
                *st.scan.lock().unwrap() = Some(scan);
                PostMessageW(target as HWND, WM_SCAN_DONE, 0, 0);
            });
        }
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if r == 0 || r == -1 {
                break;
            }
            if IsDialogMessageW(hwnd, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        APP.with(|a| a.borrow_mut().take());
        VIEW.with(|v| v.borrow_mut().style = None);
        msg.wParam as i32
    }
}

fn app_is_selftest(_app: &App) -> bool {
    #[cfg(feature = "test-sandbox")]
    {
        _app.selftest.is_some()
    }
    #[cfg(not(feature = "test-sandbox"))]
    {
        false
    }
}

/// Ends the program with `code` (the window is closed).
#[cfg(feature = "test-sandbox")]
pub(super) fn quit(hwnd: HWND, code: i32) {
    EXIT_CODE.with(|c| c.set(code));
    unsafe { DestroyWindow(hwnd) };
}

// What the self-test needs from the application.
#[cfg(feature = "test-sandbox")]
impl App {
    pub(super) fn hwnd(&self) -> HWND {
        self.hwnd
    }
    pub(super) fn page(&self) -> Page {
        self.model.page
    }
    pub(super) fn model(&self) -> &Model {
        &self.model
    }
    pub(super) fn next_button(&self) -> Option<HWND> {
        self.c.next
    }
    pub(super) fn accept_box(&self) -> Option<HWND> {
        self.c.accept
    }
    pub(super) fn component_box(&self, id: &str) -> Option<HWND> {
        let i = self.model.components.iter().position(|(c, _)| c.id == id)?;
        self.c.components.get(i).copied()
    }
    pub(super) fn dir_box(&self) -> Option<HWND> {
        self.c.dir
    }
    pub(super) fn error(&self) -> Option<String> {
        self.st.progress.lock().unwrap().error.clone()
    }
    pub(super) fn selftest(&mut self) -> Option<&mut super::selftest::SelfTest> {
        self.selftest.as_mut()
    }
}

/// The control of the window with the id `id`, for the self-test ("Tout sauvegarder").
#[cfg(feature = "test-sandbox")]
pub(super) fn child_by_id(app: &App, id: usize) -> Option<HWND> {
    #[link(name = "user32")]
    extern "system" {
        fn GetDlgItem(hwnd: HWND, id: i32) -> HWND;
    }
    let h = unsafe { GetDlgItem(app.hwnd, id as i32) };
    (!h.is_null()).then_some(h)
}

#[cfg(feature = "test-sandbox")]
pub(super) const ALL_BACKUP_ID: usize = ID_ALL_BACKUP;

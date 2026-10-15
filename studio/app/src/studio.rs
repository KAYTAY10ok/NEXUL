//! The main window of NEXUL Studio: title bar with the menus, activity bar, explorer,
//! editors in tabs, output panel and status bar.

mod files;
mod lang;

use crate::explorer;
use crate::ide;
use crate::settings::Settings;
use crate::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Editor, EditorState, Escape, Input, InputEvent},
    list::ListItem,
    menu::{AppMenuBar, ContextMenuExt as _, PopupMenu, PopupMenuItem},
    resizable::{h_resizable, resizable_panel, v_resizable},
    tab::{Tab, TabBar},
    tree::{tree, TreeEvent, TreeState},
    status_bar::StatusBar,
    v_flex, ActiveTheme as _, IconName, Selectable as _, Sizable as _, Theme, ThemeMode, TitleBar, WindowExt as _,
};
use gpui_kit::assets::IconName as Lucide;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use files::{Clip, Dragged, Naming};
use lang::PanelTab;
use nexul_studio::workspace::Kind;
use nexul_studio::{compiler, runner, watch, workspace};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// An open file.
struct Doc {
    rel: String,
    name: SharedString,
    lang: &'static str,
    editor: Entity<EditorState>,
    /// Its full path, shared with the services of nxc.
    file: ide::FileRef,
    encoding: &'static str,
    dirty: bool,
    /// Hash of the text on disk, to tell real changes from reloads.
    saved_hash: u64,
    /// The next change event comes from a reload, not from the user.
    reloading: bool,
    _changes: Subscription,
}

fn text_hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

pub struct Studio {
    focus: FocusHandle,
    settings: Settings,
    root: Option<PathBuf>,
    tree: Entity<TreeState>,
    dirs: HashSet<String>,
    expanded: HashSet<String>,
    docs: Vec<Doc>,
    active: Option<usize>,
    output: Entity<EditorState>,
    nxc: Option<PathBuf>,
    runs: runner::Runs,
    run: Option<u32>,
    watcher: Option<watch::Watcher>,
    changed: Arc<AtomicBool>,
    menu_bar: Entity<AppMenuBar>,
    /// The name box of the explorer (new entry, rename).
    naming: Option<Naming>,
    /// Entries copied or cut in the explorer.
    clip: Option<Clip>,
    /// Diagnostics of `nxc check`, with the file whose check found them.
    problems: Vec<(String, ide::Problem)>,
    /// Checks running.
    checking: usize,
    panel_tab: PanelTab,
    /// The name box of the symbol to rename.
    rename_box: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}

impl Studio {
    pub fn new(start: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = Settings::load();
        let tree = cx.new(|cx| TreeState::new(cx));
        let output = cx.new(|cx| EditorState::new(window, cx).language("text").line_number(false));
        let subscriptions = vec![cx.subscribe(&tree, |this: &mut Self, _, event: &TreeEvent, _| {
            match event {
                TreeEvent::Expanded(id) => this.expanded.insert(id.to_string()),
                TreeEvent::Collapsed(id) => this.expanded.remove(id.as_ref()),
            };
        })];
        let mut this = Studio {
            focus: cx.focus_handle(),
            settings,
            root: None,
            tree,
            dirs: HashSet::new(),
            expanded: HashSet::new(),
            docs: Vec::new(),
            active: None,
            output,
            nxc: compiler::find_nxc(),
            runs: runner::Runs::default(),
            run: None,
            watcher: None,
            changed: Arc::new(AtomicBool::new(false)),
            menu_bar: AppMenuBar::new(cx),
            naming: None,
            clip: None,
            problems: Vec::new(),
            checking: 0,
            panel_tab: PanelTab::Output,
            rename_box: None,
            _subscriptions: subscriptions,
        };
        this.apply_theme(window, cx);
        // The shortcuts work even before a file is open.
        this.focus.focus(window, cx);
        // The folder (or the file) given on the command line, else the last one.
        let start = start.or_else(|| this.settings.last_root.clone().map(PathBuf::from));
        match start {
            Some(p) if p.is_file() => {
                let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
                this.open_root(dir.clone(), window, cx);
                if let Ok(rel) = p.strip_prefix(&dir) {
                    this.open_file(&rel.to_string_lossy().replace('\\', "/"), window, cx);
                }
            }
            Some(p) if p.is_dir() => this.open_root(p, window, cx),
            _ => {}
        }
        this.poll(window, cx);
        this
    }

    // -------------------------------------------------------------- folder

    fn open_root(&mut self, root: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let root = std::fs::canonicalize(&root).map(|p| strip_verbatim(&p)).unwrap_or(root);
        self.save_session();
        self.docs.clear();
        self.active = None;
        let key = root.to_string_lossy().to_string();
        let session = self.settings.session(&key).clone();
        self.expanded = session.expanded.iter().cloned().collect();
        self.root = Some(root.clone());
        self.settings.last_root = Some(key);
        self.settings.save();
        // The tree follows the changes made inside or outside of Studio.
        let changed = self.changed.clone();
        self.watcher = watch::Watcher::start(&root, move |_| changed.store(true, Ordering::SeqCst)).ok();
        self.refresh_tree(cx);
        for rel in &session.open {
            if root.join(rel).is_file() {
                self.open_file(rel, window, cx);
            }
        }
        if let Some(a) = &session.active {
            if let Some(ix) = self.docs.iter().position(|d| &d.rel == a) {
                self.active = Some(ix);
            }
        }
        window.set_window_title(&format!("{} — NEXUL Studio", root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
        cx.notify();
    }

    fn refresh_tree(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let new_in = match &self.naming {
            Some(Naming { target: files::Target::New { dir, .. }, .. }) => Some(dir.as_str()),
            _ => None,
        };
        let (items, dirs) = explorer::build(&root, &self.expanded, new_in);
        self.dirs = dirs;
        // The selection stays on the same entry.
        self.tree.update(cx, |t, cx| {
            let selected = t.selected_item().map(|i| i.id.clone());
            t.set_items(items, cx);
            if let Some(ix) = selected.and_then(|id| t.index_of(&id)) {
                t.set_selected_index(Some(ix), cx);
            }
        });
    }

    fn save_session(&mut self) {
        let Some(root) = &self.root else { return };
        let key = root.to_string_lossy().to_string();
        let open: Vec<String> = self.docs.iter().map(|d| d.rel.clone()).collect();
        let active = self.active.and_then(|i| self.docs.get(i)).map(|d| d.rel.clone());
        let expanded: Vec<String> = self.expanded.iter().cloned().collect();
        let s = self.settings.session(&key);
        s.open = open;
        s.active = active;
        s.expanded = expanded;
        self.settings.save();
    }

    /// Changes on disk (watch) and the output of a running program, several times a second.
    fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(150)).await;
            let alive = this
                .update_in(cx, |this, window, cx| {
                    if this.changed.swap(false, Ordering::SeqCst) {
                        this.refresh_tree(cx);
                        this.reload_unmodified(window, cx);
                    }
                    this.update_output(window, cx);
                })
                .is_ok();
            if !alive {
                break;
            }
        })
        .detach();
    }

    /// Open files changed on disk are read again when they have no unsaved change.
    fn reload_unmodified(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        for doc in &mut self.docs {
            if doc.dirty {
                continue;
            }
            let Ok(bytes) = std::fs::read(root.join(&doc.rel)) else { continue };
            let Some((text, encoding)) = workspace::decode_text(&bytes) else { continue };
            let current = doc.editor.read(cx).value();
            if current.as_ref() != text.as_str() {
                doc.encoding = encoding;
                doc.saved_hash = text_hash(&text);
                doc.reloading = true;
                doc.editor.update(cx, |e, cx| e.set_value(text, window, cx));
                doc.dirty = false;
            }
        }
    }

    // -------------------------------------------------------------- files

    pub fn open_file(&mut self, rel: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.docs.iter().position(|d| d.rel == rel) {
            self.activate(ix, window, cx);
            return;
        }
        let Some(root) = self.root.clone() else { return };
        let bytes = match std::fs::read(root.join(rel)) {
            Ok(b) => b,
            Err(e) => return self.notify_error(&workspace::io_message(&e, rel).message, window, cx),
        };
        let Some((text, encoding)) = workspace::decode_text(&bytes) else {
            return self.notify_error(&format!("« {} » est un fichier binaire.", rel), window, cx);
        };
        let name: SharedString = rel.rsplit('/').next().unwrap_or(rel).to_string().into();
        let lang = explorer::language_of(&name);
        let saved_hash = text_hash(&text);
        let editor = cx.new(|cx| EditorState::new(window, cx).language(lang).line_number(true).searchable(true).default_value(text));
        let changes = cx.subscribe(&editor, |this: &mut Self, editor, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                if let Some(doc) = this.docs.iter_mut().find(|d| d.editor == editor) {
                    if doc.reloading {
                        // The change made by a reload is not a modification.
                        doc.reloading = false;
                        if text_hash(&editor.read(cx).value()) == doc.saved_hash {
                            return;
                        }
                    }
                    if !doc.dirty {
                        doc.dirty = true;
                        cx.notify();
                    }
                }
            }
        });
        let file: ide::FileRef = Rc::new(RefCell::new(root.join(rel)));
        if lang == "nexul" {
            editor.update(cx, |e, cx| e.set_highlighter_factory(crate::highlight::factory(), cx));
            self.attach_services(&editor, &file, cx);
        }
        self.docs.push(Doc { rel: rel.to_string(), name, lang, editor, file, encoding, dirty: false, saved_hash, reloading: false, _changes: changes });
        self.activate(self.docs.len() - 1, window, cx);
        self.save_session();
        if lang == "nexul" {
            self.check_file(rel, window, cx);
        } else {
            self.show_diagnostics(cx);
        }
    }

    fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(doc) = self.docs.get(ix) {
            self.active = Some(ix);
            doc.editor.update(cx, |e, cx| e.focus(window, cx));
            cx.notify();
        }
    }

    fn save_doc(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(root) = self.root.clone() else { return false };
        let Some(doc) = self.docs.get_mut(ix) else { return false };
        let text = doc.editor.read(cx).value().to_string();
        let (bytes, encoding) = workspace::encode_text(&text, doc.encoding);
        match workspace::save(&root, &doc.rel, &bytes) {
            Ok(()) => {
                doc.encoding = encoding;
                doc.saved_hash = text_hash(&text);
                doc.dirty = false;
                let rel = doc.rel.clone();
                self.check_file(&rel, window, cx);
                cx.notify();
                true
            }
            Err(e) => {
                self.notify_error(&e.message, window, cx);
                false
            }
        }
    }

    fn close_doc(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.docs.len() {
            return;
        }
        // The text is compared, not only the flag: the last keystroke may not have been
        // reported yet.
        let modified = self.docs[ix].dirty || text_hash(&self.docs[ix].editor.read(cx).value()) != self.docs[ix].saved_hash;
        if modified {
            // Unsaved changes: saved before closing (as with automatic saving).
            if !self.save_doc(ix, window, cx) {
                return;
            }
        }
        self.docs.remove(ix);
        self.active = if self.docs.is_empty() { None } else { Some(ix.min(self.docs.len() - 1)) };
        if let Some(a) = self.active {
            self.activate(a, window, cx);
        }
        self.save_session();
        cx.notify();
    }

    fn notify_error(&mut self, message: &str, window: &mut Window, cx: &mut Context<Self>) {
        window.push_notification(gpui_kit::component::notification::Notification::error(message.to_string()), cx);
    }

    // -------------------------------------------------------------- running

    fn run_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.active else { return };
        if !self.save_doc(ix, window, cx) {
            return;
        }
        let (Some(root), Some(doc)) = (self.root.clone(), self.docs.get(ix)) else { return };
        if doc.lang != "nexul" {
            return self.notify_error("F5 exécute un programme NEXUL (fichier .nx).", window, cx);
        }
        let Some(nxc) = self.nxc.clone() else {
            return self.notify_error("Compilateur nxc introuvable : installez NEXUL ou définissez NEXUL_NXC.", window, cx);
        };
        if let Some(id) = self.run.take() {
            self.runs.kill(id);
        }
        let file = root.join(&doc.rel).to_string_lossy().to_string();
        let step = runner::Step {
            banner: format!("▶ nxc run {}\n", doc.rel),
            cmd: runner::command(&nxc, &["run".into(), file], &root),
            interactive: false,
            report: None,
        };
        self.run = Some(self.runs.start(vec![step], Vec::new()));
        self.settings.panel = true;
        self.settings.save();
        cx.notify();
    }

    fn stop_run(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.run {
            self.runs.kill(id);
            cx.notify();
        }
    }

    /// Copies the output of the run into the output panel.
    fn update_output(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.run else { return };
        let Some(run) = self.runs.map.get(&id) else { return };
        let text = String::from_utf8_lossy(&run.output.lock().unwrap()).to_string();
        let done = *run.done.lock().unwrap();
        let mut shown = text;
        if let Some(code) = done {
            shown.push_str(&format!("\n— terminé (code {})\n", code));
            self.run = None;
        }
        if self.output.read(cx).value().as_ref() != shown.as_str() {
            self.output.update(cx, |o, cx| o.set_value(shown, window, cx));
        }
    }

    // -------------------------------------------------------------- appearance

    fn apply_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = match self.settings.theme.as_str() {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => {
                if matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark) {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                }
            }
        };
        Theme::change(mode, Some(window), cx);
    }

    // -------------------------------------------------------------- actions

    fn on_open_folder(&mut self, _: &OpenFolder, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Ouvrir un dossier".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = paths.await {
                if let Some(p) = paths.pop() {
                    let _ = this.update_in(cx, |this, window, cx| this.open_root(p, window, cx));
                }
            }
        })
        .detach();
    }

    fn on_save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.active {
            self.save_doc(ix, window, cx);
        }
    }

    fn on_save_all(&mut self, _: &SaveAll, window: &mut Window, cx: &mut Context<Self>) {
        for ix in 0..self.docs.len() {
            if self.docs[ix].dirty {
                self.save_doc(ix, window, cx);
            }
        }
    }

    fn on_close_editor(&mut self, _: &CloseEditor, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.active {
            self.close_doc(ix, window, cx);
        }
    }

    fn on_toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.settings.sidebar = !self.settings.sidebar;
        self.settings.save();
        cx.notify();
    }

    fn on_toggle_panel(&mut self, _: &TogglePanel, _: &mut Window, cx: &mut Context<Self>) {
        self.settings.panel = !self.settings.panel;
        self.settings.save();
        cx.notify();
    }

    fn on_toggle_minimal(&mut self, _: &ToggleMinimal, _: &mut Window, cx: &mut Context<Self>) {
        self.settings.minimal = !self.settings.minimal;
        self.settings.save();
        cx.notify();
    }

    fn on_toggle_theme(&mut self, _: &ToggleTheme, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.theme = if cx.theme().is_dark() { "light".into() } else { "dark".into() };
        self.settings.save();
        self.apply_theme(window, cx);
        cx.notify();
    }

    fn on_run(&mut self, _: &Run, window: &mut Window, cx: &mut Context<Self>) {
        self.run_active(window, cx);
    }

    fn on_stop(&mut self, _: &Stop, _: &mut Window, cx: &mut Context<Self>) {
        self.stop_run(cx);
    }

    fn on_next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(a) = self.active {
            if !self.docs.is_empty() {
                self.activate((a + 1) % self.docs.len(), window, cx);
            }
        }
    }

    fn on_new_file(&mut self, _: &NewFile, window: &mut Window, cx: &mut Context<Self>) {
        let dir = self.target_dir(cx);
        self.start_new(dir, Kind::File, window, cx);
    }

    fn on_new_folder(&mut self, _: &NewFolder, window: &mut Window, cx: &mut Context<Self>) {
        let dir = self.target_dir(cx);
        self.start_new(dir, Kind::Dir, window, cx);
    }

    fn on_rename_entry(&mut self, _: &RenameEntry, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(rel) = self.selected(cx) {
            self.start_rename(rel, window, cx);
        }
    }

    fn on_delete_entry(&mut self, _: &DeleteEntry, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(rel) = self.selected(cx) {
            self.confirm_delete(rel, window, cx);
        }
    }

    fn on_copy_entry(&mut self, _: &CopyEntry, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(rel) = self.selected(cx) {
            self.set_clip(vec![rel], false, cx);
        }
    }

    fn on_cut_entry(&mut self, _: &CutEntry, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(rel) = self.selected(cx) {
            self.set_clip(vec![rel], true, cx);
        }
    }

    fn on_paste_entry(&mut self, _: &PasteEntry, window: &mut Window, cx: &mut Context<Self>) {
        let dir = self.target_dir(cx);
        self.paste(dir, window, cx);
    }

    fn on_reveal_entry(&mut self, _: &RevealEntry, window: &mut Window, cx: &mut Context<Self>) {
        let rel = self.selected(cx).unwrap_or_default();
        self.reveal(&rel, window, cx);
    }

    fn on_copy_path(&mut self, _: &CopyPath, _: &mut Window, cx: &mut Context<Self>) {
        let rel = self.selected(cx).unwrap_or_default();
        self.copy_path(&rel, false, cx);
    }

    fn on_copy_relative_path(&mut self, _: &CopyRelativePath, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(rel) = self.selected(cx) {
            self.copy_path(&rel, true, cx);
        }
    }

    fn on_refresh_explorer(&mut self, _: &RefreshExplorer, _: &mut Window, cx: &mut Context<Self>) {
        self.refresh_tree(cx);
    }

    fn on_collapse_explorer(&mut self, _: &CollapseExplorer, _: &mut Window, cx: &mut Context<Self>) {
        self.expanded.clear();
        self.refresh_tree(cx);
        self.save_session();
    }

    fn on_format_document(&mut self, _: &FormatDocument, window: &mut Window, cx: &mut Context<Self>) {
        self.format_active(window, cx);
    }

    fn on_rename_symbol(&mut self, _: &RenameSymbol, window: &mut Window, cx: &mut Context<Self>) {
        self.ask_rename_symbol(window, cx);
    }

    fn on_show_problems(&mut self, _: &ShowProblems, _: &mut Window, cx: &mut Context<Self>) {
        self.show_panel(PanelTab::Problems, cx);
    }

    fn on_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        if !self.cancel_naming(window, cx) {
            cx.propagate();
        }
    }

    // -------------------------------------------------------------- views

    fn render_activity_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .w(px(44.))
            .h_full()
            .py_2()
            .gap_1()
            .items_center()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .child(
                Button::new("act-explorer")
                    .ghost()
                    .icon(IconName::Folder)
                    .selected(self.settings.sidebar)
                    .tooltip("Explorateur (Ctrl+B)")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(ToggleSidebar), cx)),
            )
            .child(
                Button::new("act-run")
                    .ghost()
                    .icon(IconName::Play)
                    .tooltip("Exécuter (F5)")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Run), cx)),
            )
            .child(
                Button::new("act-panel")
                    .ghost()
                    .icon(IconName::SquareTerminal)
                    .selected(self.settings.panel)
                    .tooltip("Sortie (Ctrl+J)")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(TogglePanel), cx)),
            )
            .child(div().flex_1())
            .child(
                Button::new("act-theme")
                    .ghost()
                    .icon(if cx.theme().is_dark() { IconName::Sun } else { IconName::Moon })
                    .tooltip("Thème clair ou sombre")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(ToggleTheme), cx)),
            )
    }

    fn render_explorer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let studio = cx.entity().downgrade();
        let title = self.root.as_ref().and_then(|r| r.file_name()).map(|n| n.to_string_lossy().to_uppercase()).unwrap_or_else(|| "AUCUN DOSSIER".into());
        let header = h_flex()
            .id("explorer-header")
            .h(px(32.))
            .pl_3()
            .pr_1()
            .gap_0p5()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().child(title));
        if self.root.is_none() {
            return v_flex().size_full().bg(theme.sidebar).child(header).child(
                v_flex().p_3().gap_2().child(div().text_sm().text_color(theme.muted_foreground).child("Aucun dossier n'est ouvert.")).child(
                    Button::new("open-folder")
                        .primary()
                        .label("Ouvrir un dossier")
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(OpenFolder), cx)),
                ),
            );
        }
        // The header: tools, drop and menu for the root of the folder.
        let header = header
            .child(tool("new-file", Lucide::FilePlus, "Nouveau fichier (Ctrl+N)", NewFile))
            .child(tool("new-folder", Lucide::FolderPlus, "Nouveau dossier", NewFolder))
            .child(tool("refresh", IconName::RefreshCw, "Actualiser", RefreshExplorer))
            .child(tool("collapse", Lucide::ListChevronsDownUp, "Tout replier", CollapseExplorer))
            .drag_over::<Dragged>(|s, _, _, cx| s.bg(cx.theme().drop_target))
            .drag_over::<ExternalPaths>(|s, _, _, cx| s.bg(cx.theme().drop_target))
            .on_drop(drop_entries_into(&studio, String::new()))
            .on_drop(drop_paths_into(&studio, String::new()))
            .context_menu({
                let studio = studio.clone();
                move |menu, _, _| root_menu(menu, &studio)
            });

        let dirs = self.dirs.clone();
        let naming = self.naming.as_ref().map(|n| (n.row(), n.input.clone(), matches!(n.target, files::Target::New { kind: Kind::Dir, .. })));
        let cut: HashSet<String> = self.clip.iter().filter(|c| c.cut).flat_map(|c| c.items.iter().cloned()).collect();
        let row_studio = studio.clone();
        let list = tree(&self.tree, move |ix, entry, selected, _window, _cx| {
            let rel = entry.item().id.to_string();
            let editing = naming.as_ref().filter(|(row, _, _)| row.as_ref() == rel);
            let is_dir = dirs.contains(&rel) || editing.is_some_and(|(_, _, dir)| *dir && rel == explorer::NEW_ENTRY);
            let icon = if is_dir {
                if entry.is_expanded() { IconName::FolderOpen } else { IconName::Folder }
            } else {
                IconName::File
            };
            let item = ListItem::new(ix).selected(selected).py_0p5().pl(px(10. + entry.depth() as f32 * 14.));
            let icon = gpui_kit::component::Icon::new(icon).xsmall();
            if let Some((_, input, _)) = editing {
                return item.child(h_flex().gap_1p5().text_sm().child(icon).child(div().flex_1().child(Input::new(input).xsmall())));
            }
            let dir = if is_dir { rel.clone() } else { files::parent_of(&rel).to_string() };
            let studio = row_studio.clone();
            let open = rel.clone();
            item.child(
                h_flex()
                    .gap_1p5()
                    .text_sm()
                    .when(cut.contains(&rel), |this| this.opacity(0.5))
                    .child(icon)
                    .child(entry.item().label.clone()),
            )
            .on_drag(Dragged::new(&rel), |dragged, _, _, cx| cx.new(|_| dragged.clone()))
            .drag_over::<Dragged>(|s, _, _, cx| s.bg(cx.theme().drop_target))
            .drag_over::<ExternalPaths>(|s, _, _, cx| s.bg(cx.theme().drop_target))
            .on_drop(drop_entries_into(&studio, dir.clone()))
            .on_drop(drop_paths_into(&studio, dir))
            .on_click(move |_, window, cx| {
                if !is_dir {
                    let _ = studio.update(cx, |this, cx| this.open_file(&open, window, cx));
                }
            })
        })
        .context_menu({
            let studio = studio.clone();
            let dirs = self.dirs.clone();
            move |_, entry, menu, _, _| {
                let rel = entry.item().id.to_string();
                if rel == explorer::NEW_ENTRY {
                    return menu;
                }
                entry_menu(menu, &studio, rel.clone(), dirs.contains(&rel))
            }
        });
        v_flex()
            .key_context("Explorer")
            .size_full()
            .bg(theme.sidebar)
            .child(header)
            .child(
                div()
                    .id("explorer-list")
                    .flex_1()
                    .min_h_0()
                    // A drop beside the entries goes to the root.
                    .drag_over::<Dragged>(|s, _, _, cx| s.bg(cx.theme().drop_target))
                    .drag_over::<ExternalPaths>(|s, _, _, cx| s.bg(cx.theme().drop_target))
                    .on_drop(drop_entries_into(&studio, String::new()))
                    .on_drop(drop_paths_into(&studio, String::new()))
                    .child(list),
            )
    }

    fn render_editors(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let Some(active) = self.active.and_then(|i| self.docs.get(i).map(|d| (i, d))) else {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .bg(theme.background)
                .text_color(theme.muted_foreground)
                .child(div().text_lg().child("NEXUL Studio"))
                .child(div().text_sm().child("Ctrl+O : ouvrir un dossier · Ctrl+B : explorateur · Ctrl+J : sortie · F5 : exécuter"));
        };
        let (ix, doc) = active;
        let studio = cx.entity().downgrade();
        let tabs = TabBar::new("tabs")
            .underline()
            .selected_index(ix)
            .children(self.docs.iter().enumerate().map(|(i, d)| {
                let studio = studio.clone();
                Tab::new()
                    .label(if d.dirty { format!("● {}", d.name) } else { d.name.to_string() })
                    .suffix(Button::new(("close", i)).ghost().xsmall().icon(IconName::Close).on_click(move |_, window, cx| {
                        let _ = studio.update(cx, |this, cx| this.close_doc(i, window, cx));
                    }))
            }))
            .on_click(cx.listener(|this, ix: &usize, window, cx| this.activate(*ix, window, cx)));
        v_flex()
            .size_full()
            .bg(theme.background)
            .child(tabs)
            .child(div().key_context("CodeEditor").flex_1().min_h_0().child(Editor::new(&doc.editor).bordered(false).h_full()))
    }

    fn render_output(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .size_full()
            .bg(theme.background)
            .border_t_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .h(px(30.))
                    .px_1()
                    .gap_2()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(
                        TabBar::new("panel-tabs")
                            .underline()
                            .xsmall()
                            .selected_index(if self.panel_tab == PanelTab::Problems { 1 } else { 0 })
                            .child(Tab::new().label("SORTIE"))
                            .child(Tab::new().label(match self.problems.len() {
                                0 => "PROBLÈMES".to_string(),
                                n => format!("PROBLÈMES ({})", n),
                            }))
                            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                                this.panel_tab = if *ix == 1 { PanelTab::Problems } else { PanelTab::Output };
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .when(self.run.is_some(), |this| {
                        this.child(
                            Button::new("stop")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Square)
                                .tooltip("Arrêter (Maj+F5)")
                                .on_click(|_, window, cx| window.dispatch_action(Box::new(Stop), cx)),
                        )
                    }),
            )
            .child(div().flex_1().min_h_0().map(|this| match self.panel_tab {
                PanelTab::Output => this.child(Editor::new(&self.output).bordered(false).readonly(true).h_full()).into_any_element(),
                PanelTab::Problems => this.child(self.render_problems(cx)).into_any_element(),
            }))
    }

    fn render_status(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let doc = self.active.and_then(|i| self.docs.get(i));
        let (pos, lang, enc) = match doc {
            Some(d) => {
                let p = d.editor.read(cx).cursor_position();
                (
                    format!("Ln {}, Col {}", p.line + 1, p.character + 1),
                    explorer::language_name(d.lang),
                    match d.encoding {
                        "utf-8-bom" => "UTF-8 avec BOM",
                        "utf-16le" => "UTF-16 LE",
                        "utf-16be" => "UTF-16 BE",
                        "windows-1252" => "Windows 1252",
                        _ => "UTF-8",
                    },
                )
            }
            None => (String::new(), "", ""),
        };
        let nxc = if self.nxc.is_some() { "nxc ✓" } else { "nxc introuvable" };
        let running = if self.run.is_some() { "Exécution…" } else if self.checking > 0 { "Vérification…" } else { "" };
        let (errors, warnings) = self.problem_counts();
        let counts = h_flex()
            .id("problem-counts")
            .gap_1()
            .cursor_pointer()
            .child(gpui_kit::component::Icon::new(IconName::CircleX).xsmall())
            .child(errors.to_string())
            .child(gpui_kit::component::Icon::new(IconName::TriangleAlert).xsmall())
            .child(warnings.to_string())
            .on_click(cx.listener(|this, _, _, cx| this.show_panel(PanelTab::Problems, cx)));
        StatusBar::new()
            .left(h_flex().gap_3().text_xs().child("NEXUL").child(counts).child(running))
            .right(h_flex().gap_4().text_xs().child(pos).child(enc).child(lang).child(nxc))
    }
}

impl Focusable for Studio {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Studio {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let minimal = self.settings.minimal;
        let sidebar = self.settings.sidebar && !minimal;
        let panel = self.settings.panel && !minimal;
        let title = self.root.as_ref().and_then(|r| r.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let center = v_resizable("center")
            .child(resizable_panel().child(self.render_editors(cx)))
            .child(resizable_panel().size(px(220.)).size_range(px(80.)..px(900.)).visible(panel).child(self.render_output(cx)));
        let main = h_resizable("main")
            .child(resizable_panel().size(px(260.)).size_range(px(160.)..px(600.)).visible(sidebar).child(self.render_explorer(cx)))
            .child(resizable_panel().child(center));
        v_flex()
            .id("studio")
            .key_context("Studio")
            .track_focus(&self.focus)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::on_open_folder))
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_save_all))
            .on_action(cx.listener(Self::on_close_editor))
            .on_action(cx.listener(Self::on_toggle_sidebar))
            .on_action(cx.listener(Self::on_toggle_panel))
            .on_action(cx.listener(Self::on_toggle_minimal))
            .on_action(cx.listener(Self::on_toggle_theme))
            .on_action(cx.listener(Self::on_run))
            .on_action(cx.listener(Self::on_stop))
            .on_action(cx.listener(Self::on_next_tab))
            .on_action(cx.listener(Self::on_new_file))
            .on_action(cx.listener(Self::on_new_folder))
            .on_action(cx.listener(Self::on_rename_entry))
            .on_action(cx.listener(Self::on_delete_entry))
            .on_action(cx.listener(Self::on_copy_entry))
            .on_action(cx.listener(Self::on_cut_entry))
            .on_action(cx.listener(Self::on_paste_entry))
            .on_action(cx.listener(Self::on_reveal_entry))
            .on_action(cx.listener(Self::on_copy_path))
            .on_action(cx.listener(Self::on_copy_relative_path))
            .on_action(cx.listener(Self::on_refresh_explorer))
            .on_action(cx.listener(Self::on_collapse_explorer))
            .on_action(cx.listener(Self::on_escape))
            .on_action(cx.listener(Self::on_format_document))
            .on_action(cx.listener(Self::on_rename_symbol))
            .on_action(cx.listener(Self::on_show_problems))
            .when(!minimal, |this| {
                this.child(TitleBar::new().child(self.menu_bar.clone()).child(div().flex_1()).child(div().text_xs().text_color(cx.theme().muted_foreground).pr_4().child(title)))
            })
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .when(!minimal, |this| this.child(self.render_activity_bar(cx)))
                    .child(div().flex_1().h_full().child(main)),
            )
            .when(!minimal, |this| this.child(self.render_status(cx)))
    }
}

/// A small button of the explorer header.
fn tool(id: &'static str, icon: impl Into<gpui_kit::component::Icon>, tip: &'static str, action: impl Action) -> Button {
    Button::new(id).ghost().xsmall().icon(icon).tooltip(tip).on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx))
}

/// Entries of the explorer dropped on `dir`.
fn drop_entries_into(studio: &WeakEntity<Studio>, dir: String) -> impl Fn(&Dragged, &mut Window, &mut App) + 'static {
    let studio = studio.clone();
    move |dragged, window, cx| {
        let _ = studio.update(cx, |this, cx| this.drop_entries(dragged, dir.clone(), window, cx));
    }
}

/// Files of the system dropped on `dir`.
fn drop_paths_into(studio: &WeakEntity<Studio>, dir: String) -> impl Fn(&ExternalPaths, &mut Window, &mut App) + 'static {
    let studio = studio.clone();
    move |paths, window, cx| {
        let _ = studio.update(cx, |this, cx| this.import_external(paths.paths(), dir.clone(), window, cx));
    }
}

/// A menu item that acts on Studio; `action` only shows its shortcut.
fn act(label: &'static str, action: Option<Box<dyn Action>>, studio: &WeakEntity<Studio>, f: impl Fn(&mut Studio, &mut Window, &mut Context<Studio>) + 'static) -> PopupMenuItem {
    let studio = studio.clone();
    let item = PopupMenuItem::new(label).on_click(move |_, window, cx| {
        let _ = studio.update(cx, |this, cx| f(this, window, cx));
    });
    match action {
        Some(a) => item.action(a),
        None => item,
    }
}

/// The menu of an entry of the explorer (right click).
fn entry_menu(menu: PopupMenu, studio: &WeakEntity<Studio>, rel: String, is_dir: bool) -> PopupMenu {
    let dir = if is_dir { rel.clone() } else { files::parent_of(&rel).to_string() };
    let mut menu = menu;
    if is_dir {
        let (d1, d2) = (dir.clone(), dir.clone());
        menu = menu
            .item(act("Nouveau fichier…", Some(Box::new(NewFile)), studio, move |s, w, c| s.start_new(d1.clone(), Kind::File, w, c)))
            .item(act("Nouveau dossier…", Some(Box::new(NewFolder)), studio, move |s, w, c| s.start_new(d2.clone(), Kind::Dir, w, c)))
            .separator();
    }
    let (r1, r2, r3, r4, r5, r6, r7) = (rel.clone(), rel.clone(), rel.clone(), rel.clone(), rel.clone(), rel.clone(), rel);
    menu.item(act("Couper", Some(Box::new(CutEntry)), studio, move |s, _, c| s.set_clip(vec![r1.clone()], true, c)))
        .item(act("Copier", Some(Box::new(CopyEntry)), studio, move |s, _, c| s.set_clip(vec![r2.clone()], false, c)))
        .item(act("Coller", Some(Box::new(PasteEntry)), studio, move |s, w, c| s.paste(dir.clone(), w, c)))
        .separator()
        .item(act("Copier le chemin", Some(Box::new(CopyPath)), studio, move |s, _, c| s.copy_path(&r3, false, c)))
        .item(act("Copier le chemin relatif", Some(Box::new(CopyRelativePath)), studio, move |s, _, c| s.copy_path(&r4, true, c)))
        .separator()
        .item(act("Afficher dans l'Explorateur", Some(Box::new(RevealEntry)), studio, move |s, w, c| s.reveal(&r5, w, c)))
        .separator()
        .item(act("Renommer…", Some(Box::new(RenameEntry)), studio, move |s, w, c| s.start_rename(r6.clone(), w, c)))
        .item(act("Supprimer", Some(Box::new(DeleteEntry)), studio, move |s, w, c| s.confirm_delete(r7.clone(), w, c)))
}

/// The menu of the root of the folder (right click on the header of the explorer).
fn root_menu(menu: PopupMenu, studio: &WeakEntity<Studio>) -> PopupMenu {
    menu.item(act("Nouveau fichier…", None, studio, |s, w, c| s.start_new(String::new(), Kind::File, w, c)))
        .item(act("Nouveau dossier…", None, studio, |s, w, c| s.start_new(String::new(), Kind::Dir, w, c)))
        .item(act("Coller", None, studio, |s, w, c| s.paste(String::new(), w, c)))
        .separator()
        .item(act("Copier le chemin", None, studio, |s, _, c| s.copy_path("", false, c)))
        .item(act("Afficher dans l'Explorateur", None, studio, |s, w, c| s.reveal("", w, c)))
        .separator()
        .item(act("Actualiser", None, studio, |s, _, c| s.refresh_tree(c)))
        .item(act("Tout replier", None, studio, |s, w, c| s.on_collapse_explorer(&CollapseExplorer, w, c)))
}

/// `\\?\C:\x` → `C:\x` (canonical paths on Windows).
fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => p.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    //! Functional tests: a real Studio window (headless), real keystrokes and actions,
    //! real files. Run with `--test-threads=1` (they share the settings file).
    use super::*;
    // The glob import of gpui_kit brings GPUI's `test` macro: Rust's own is put back.
    #[allow(unused_imports)]
    use core::prelude::v1::test;
    use gpui_kit::component::input::Position;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::TestAppContext;

    fn project(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nexul-native-test-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::write(d.join("main.nx"), "fn main() {\n    println(\"ok\")\n}\n").unwrap();
        std::fs::write(d.join("src").join("calcul.nx"), "pub fn f() {}\n").unwrap();
        d
    }

    fn open(cx: &mut TestAppContext, start: PathBuf) -> (AnyWindowHandle, Entity<Studio>) {
        std::env::set_var("NEXUL_STUDIO_CONFIG", std::env::temp_dir().join(format!("nexul-native-config-{}", std::process::id())));
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::bind_keys(cx);
        });
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
        let (window, studio) = cx.update(|cx| gpui_kit::open_window(options, cx, move |window, cx| cx.new(|cx| Studio::new(Some(start), window, cx)))).unwrap();
        cx.update_window(window, |_, window, cx| window.render_frame(cx)).unwrap();
        (window, studio)
    }

    fn update<R>(cx: &mut TestAppContext, window: AnyWindowHandle, studio: &Entity<Studio>, f: impl FnOnce(&mut Studio, &mut Window, &mut Context<Studio>) -> R) -> R {
        let r = cx.update_window(window, |_, window, cx| studio.update(cx, |s, cx| f(s, window, cx))).unwrap();
        cx.update_window(window, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
        r
    }

    fn keys(cx: &mut TestAppContext, window: AnyWindowHandle, f: impl FnOnce(&mut Window, &mut App)) {
        cx.update_window(window, |_, window, cx| {
            f(window, cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn opens_edits_and_saves_with_the_keyboard(cx: &mut TestAppContext) {
        let dir = project("save");
        let (window, studio) = open(cx, dir.clone());
        studio.read_with(cx, |s, _| {
            assert!(s.dirs.contains("src"), "the tree knows the folders");
            assert!(s.docs.is_empty());
        });
        update(cx, window, &studio, |s, window, cx| s.open_file("main.nx", window, cx));
        keys(cx, window, |window, cx| {
            window.press("ctrl-end", cx);
            window.input("// ajout été\n", cx);
        });
        studio.read_with(cx, |s, _| assert!(s.docs[0].dirty, "typing marks the file as modified"));
        keys(cx, window, |window, cx| window.press("ctrl-s", cx));
        let text = std::fs::read_to_string(dir.join("main.nx")).unwrap();
        assert!(text.ends_with("// ajout été\n"), "{:?}", text);
        studio.read_with(cx, |s, _| assert!(!s.docs[0].dirty, "saved"));
    }

    #[gpui_kit::test]
    fn keeps_the_encoding_of_the_file(cx: &mut TestAppContext) {
        let dir = project("enc");
        // "café" in Windows-1252.
        std::fs::write(dir.join("ancien.txt"), [0x63, 0x61, 0x66, 0xE9, b'\n']).unwrap();
        let (window, studio) = open(cx, dir.clone());
        update(cx, window, &studio, |s, window, cx| s.open_file("ancien.txt", window, cx));
        studio.read_with(cx, |s, cx| {
            assert_eq!(s.docs[0].encoding, "windows-1252");
            assert_eq!(s.docs[0].editor.read(cx).value().as_ref(), "café\n");
        });
        keys(cx, window, |window, cx| {
            window.press("ctrl-end", cx);
            window.input("thé\n", cx);
            window.press("ctrl-s", cx);
        });
        assert_eq!(std::fs::read(dir.join("ancien.txt")).unwrap(), b"caf\xe9\nth\xe9\n");
    }

    #[gpui_kit::test]
    fn shortcuts_toggle_the_panels(cx: &mut TestAppContext) {
        let dir = project("toggle");
        let (window, studio) = open(cx, dir);
        let before = studio.read_with(cx, |s, _| (s.settings.sidebar, s.settings.panel));
        keys(cx, window, |window, cx| window.press("ctrl-b", cx));
        keys(cx, window, |window, cx| window.press("ctrl-j", cx));
        let after = studio.read_with(cx, |s, _| (s.settings.sidebar, s.settings.panel));
        assert_eq!(after, (!before.0, !before.1));
        // Back to the previous state for the other tests.
        keys(cx, window, |window, cx| {
            window.press("ctrl-b", cx);
            window.press("ctrl-j", cx);
        });
    }

    #[gpui_kit::test]
    fn reopens_the_files_of_the_folder(cx: &mut TestAppContext) {
        let dir = project("session");
        let (window, studio) = open(cx, dir.clone());
        update(cx, window, &studio, |s, window, cx| {
            s.open_file("main.nx", window, cx);
            s.open_file("src/calcul.nx", window, cx);
        });
        let (_window2, studio2) = open(cx, dir);
        studio2.read_with(cx, |s, _| {
            let open: Vec<&str> = s.docs.iter().map(|d| d.rel.as_str()).collect();
            assert_eq!(open, ["main.nx", "src/calcul.nx"]);
            assert_eq!(s.active, Some(1));
        });
    }

    #[gpui_kit::test]
    fn follows_files_changed_outside(cx: &mut TestAppContext) {
        let dir = project("watch");
        let (window, studio) = open(cx, dir.clone());
        update(cx, window, &studio, |s, window, cx| s.open_file("main.nx", window, cx));
        std::fs::write(dir.join("nouveau.nx"), "fn g() {}\n").unwrap();
        std::fs::write(dir.join("main.nx"), "fn main() {}\n").unwrap();
        update(cx, window, &studio, |s, window, cx| {
            s.refresh_tree(cx);
            s.reload_unmodified(window, cx);
        });
        studio.read_with(cx, |s, cx| {
            assert!(s.tree.read(cx).index_of(&"nouveau.nx".into()).is_some(), "new file in the tree");
            assert_eq!(s.docs[0].editor.read(cx).value().as_ref(), "fn main() {}\n", "open file reloaded");
            assert!(!s.docs[0].dirty, "a reload is not a modification");
        });
    }

    fn focus_explorer(cx: &mut TestAppContext, window: AnyWindowHandle, studio: &Entity<Studio>, rel: &str) {
        let rel = rel.to_string();
        update(cx, window, studio, move |s, window, cx| {
            s.select(&rel, cx);
            s.tree.update(cx, |t, cx| t.focus(window, cx));
        });
    }

    #[gpui_kit::test]
    fn creates_and_renames_in_the_explorer(cx: &mut TestAppContext) {
        let dir = project("naming");
        let (window, studio) = open(cx, dir.clone());
        // Ctrl+N: the name box, in the folder of the selection.
        focus_explorer(cx, window, &studio, "src/calcul.nx");
        keys(cx, window, |window, cx| window.press("ctrl-n", cx));
        studio.read_with(cx, |s, cx| {
            assert!(s.naming.is_some(), "name box shown");
            assert!(s.tree.read(cx).index_of(&explorer::NEW_ENTRY.into()).is_some(), "row of the box in the tree");
        });
        keys(cx, window, |window, cx| {
            window.input("outils.nx", cx);
            window.press("enter", cx);
        });
        assert!(dir.join("src/outils.nx").is_file(), "file created in the folder of the selection");
        studio.read_with(cx, |s, _| {
            assert!(s.naming.is_none());
            assert_eq!(s.docs.last().map(|d| d.rel.as_str()), Some("src/outils.nx"), "new file opened");
        });
        // F2: the name without its extension is selected, typing replaces it.
        focus_explorer(cx, window, &studio, "src/outils.nx");
        keys(cx, window, |window, cx| window.press("f2", cx));
        keys(cx, window, |window, cx| {
            window.input("aide", cx);
            window.press("enter", cx);
        });
        assert!(dir.join("src/aide.nx").is_file() && !dir.join("src/outils.nx").exists(), "renamed");
        studio.read_with(cx, |s, _| {
            assert_eq!(s.docs.last().map(|d| d.rel.as_str()), Some("src/aide.nx"), "the open editor follows");
            assert_eq!(s.docs.last().map(|d| d.name.as_ref()), Some("aide.nx"));
        });
        // A folder renamed: the editors of its files follow.
        focus_explorer(cx, window, &studio, "src");
        keys(cx, window, |window, cx| window.press("f2", cx));
        keys(cx, window, |window, cx| {
            window.input("source", cx);
            window.press("enter", cx);
        });
        assert!(dir.join("source/aide.nx").is_file());
        studio.read_with(cx, |s, _| assert_eq!(s.docs.last().map(|d| d.rel.as_str()), Some("source/aide.nx")));
    }

    #[gpui_kit::test]
    fn refuses_bad_names_and_cancels(cx: &mut TestAppContext) {
        let dir = project("badname");
        let (window, studio) = open(cx, dir.clone());
        focus_explorer(cx, window, &studio, "main.nx");
        keys(cx, window, |window, cx| window.press("ctrl-n", cx));
        keys(cx, window, |window, cx| {
            window.input("CON.nx", cx);
            window.press("enter", cx);
        });
        studio.read_with(cx, |s, _| assert!(s.naming.is_some(), "a refused name keeps the box open"));
        assert!(!dir.join("CON.nx").exists());
        keys(cx, window, |window, cx| window.press("escape", cx));
        studio.read_with(cx, |s, cx| {
            assert!(s.naming.is_none(), "Escape closes the box");
            assert!(s.tree.read(cx).index_of(&explorer::NEW_ENTRY.into()).is_none());
        });
        // An existing name.
        focus_explorer(cx, window, &studio, "main.nx");
        keys(cx, window, |window, cx| window.press("ctrl-n", cx));
        keys(cx, window, |window, cx| {
            window.input("main.nx", cx);
            window.press("enter", cx);
        });
        assert_eq!(std::fs::read_to_string(dir.join("main.nx")).unwrap(), "fn main() {\n    println(\"ok\")\n}\n", "nothing overwritten");
        keys(cx, window, |window, cx| window.press("escape", cx));
    }

    #[gpui_kit::test]
    fn copies_cuts_and_pastes(cx: &mut TestAppContext) {
        let dir = project("clip");
        let (window, studio) = open(cx, dir.clone());
        update(cx, window, &studio, |s, window, cx| s.open_file("src/calcul.nx", window, cx));
        // Copy main.nx into src.
        focus_explorer(cx, window, &studio, "main.nx");
        keys(cx, window, |window, cx| window.press("ctrl-c", cx));
        focus_explorer(cx, window, &studio, "src");
        keys(cx, window, |window, cx| window.press("ctrl-v", cx));
        assert!(dir.join("src/main.nx").is_file() && dir.join("main.nx").is_file());
        // Again: a free name.
        keys(cx, window, |window, cx| window.press("ctrl-v", cx));
        assert!(dir.join("src/main (2).nx").is_file());
        // Cut src/calcul.nx, paste at the root (the folder of main.nx).
        focus_explorer(cx, window, &studio, "src/calcul.nx");
        keys(cx, window, |window, cx| window.press("ctrl-x", cx));
        focus_explorer(cx, window, &studio, "main.nx");
        keys(cx, window, |window, cx| window.press("ctrl-v", cx));
        assert!(dir.join("calcul.nx").is_file() && !dir.join("src/calcul.nx").exists());
        studio.read_with(cx, |s, cx| {
            assert_eq!(s.docs[0].rel, "calcul.nx", "the open editor follows the move");
            assert!(s.clip.is_none(), "a cut is pasted once");
            assert_eq!(s.selected(cx).as_deref(), Some("calcul.nx"), "the pasted entry is selected");
        });
    }

    #[gpui_kit::test]
    fn drags_and_drops(cx: &mut TestAppContext) {
        let dir = project("drop");
        let outside = std::env::temp_dir().join(format!("nexul-native-test-outside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&outside);
        std::fs::create_dir_all(outside.join("images")).unwrap();
        std::fs::write(outside.join("images/logo.svg"), "<svg/>").unwrap();
        let (window, studio) = open(cx, dir.clone());
        update(cx, window, &studio, |s, window, cx| {
            s.drop_entries(&Dragged::new("main.nx"), "src".into(), window, cx);
            // A folder released on itself does nothing.
            s.drop_entries(&Dragged::new("src"), "src".into(), window, cx);
            s.import_external(&[outside.join("images")], "src".into(), window, cx);
        });
        assert!(dir.join("src/main.nx").is_file() && !dir.join("main.nx").exists());
        assert!(dir.join("src/images/logo.svg").is_file(), "folder of the system copied");
        assert!(outside.join("images/logo.svg").is_file(), "the original stays");
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[gpui_kit::test]
    fn deletes_to_the_recycle_bin_after_asking(cx: &mut TestAppContext) {
        let dir = project("trash");
        std::fs::write(dir.join("brouillon.txt"), "x").unwrap();
        let (window, studio) = open(cx, dir.clone());
        update(cx, window, &studio, |s, window, cx| s.open_file("brouillon.txt", window, cx));
        focus_explorer(cx, window, &studio, "brouillon.txt");
        keys(cx, window, |window, cx| window.press("delete", cx));
        assert!(dir.join("brouillon.txt").exists(), "nothing deleted before the answer");
        let asked = cx.update_window(window, |_, window, cx| window.has_active_dialog(cx)).unwrap();
        assert!(asked, "confirmation asked");
        cx.update_window(window, |_, window, cx| window.close_dialog(cx)).unwrap();
        update(cx, window, &studio, |s, window, cx| s.trash_entries(&["brouillon.txt".to_string()], window, cx));
        assert!(!dir.join("brouillon.txt").exists());
        studio.read_with(cx, |s, _| assert!(s.docs.is_empty(), "its editor is closed"));
    }

    /// A NEXUL project with a type error; `None` when nxc is not installed here.
    fn nexul_project(tag: &str) -> Option<PathBuf> {
        compiler::find_nxc()?;
        let dir = project(tag);
        std::fs::write(dir.join("main.nx"), "fn carre(x: i32) -> i32 {\n    return x * x\n}\n\nfn main() {\n    let v: String = carre(4)\n    println(v)\n}\n").unwrap();
        Some(dir)
    }

    #[gpui_kit::test]
    fn shows_the_diagnostics_of_nxc(cx: &mut TestAppContext) {
        let Some(dir) = nexul_project("diag") else { return eprintln!("nxc introuvable : test sauté") };
        let (window, studio) = open(cx, dir.clone());
        update(cx, window, &studio, |s, window, cx| s.open_file("main.nx", window, cx));
        cx.run_until_parked();
        studio.read_with(cx, |s, cx| {
            assert_eq!(s.checking, 0);
            assert_eq!(s.problem_counts().0, 1, "{:?}", s.problems);
            let p = &s.problems[0].1;
            assert_eq!((p.range.start.line, p.range.start.character), (5, 20), "{:?}", p);
            assert!(p.message.contains("mismatched types"), "{}", p.message);
            let shown = s.docs[0].editor.read(cx).diagnostics().map(|d| d.len());
            assert_eq!(shown, Some(1), "underlined in the editor");
        });
        // Ctrl+Shift+M shows them.
        keys(cx, window, |window, cx| window.press("ctrl-shift-m", cx));
        studio.read_with(cx, |s, _| assert_eq!(s.panel_tab, PanelTab::Problems));
        // Corrected and saved: no more problems.
        update(cx, window, &studio, |s, window, cx| {
            s.docs[0].editor.update(cx, |e, cx| e.set_value("fn main() {\n    println(\"ok\")\n}\n", window, cx));
            s.save_doc(0, window, cx);
        });
        cx.run_until_parked();
        studio.read_with(cx, |s, _| assert!(s.problems.is_empty(), "{:?}", s.problems));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[gpui_kit::test]
    fn renames_a_symbol_and_formats(cx: &mut TestAppContext) {
        let Some(dir) = nexul_project("symbol") else { return eprintln!("nxc introuvable : test sauté") };
        let (window, studio) = open(cx, dir.clone());
        update(cx, window, &studio, |s, window, cx| {
            s.open_file("main.nx", window, cx);
            s.rename_symbol(Position::new(0, 4), "square".into(), window, cx);
        });
        studio.read_with(cx, |s, cx| {
            let text = s.docs[0].editor.read(cx).value().to_string();
            assert!(text.starts_with("fn square(x: i32)") && text.contains("= square(4)"), "{}", text);
            assert!(s.docs[0].dirty, "a change to save");
        });
        // An invalid name changes nothing.
        update(cx, window, &studio, |s, window, cx| s.rename_symbol(Position::new(0, 4), "2x".into(), window, cx));
        studio.read_with(cx, |s, cx| assert!(s.docs[0].editor.read(cx).value().starts_with("fn square(")));
        // Formatting (Shift+Alt+F).
        update(cx, window, &studio, |s, window, cx| s.docs[0].editor.update(cx, |e, cx| e.set_value("fn main(){\n  println(\"ok\")}\n", window, cx)));
        keys(cx, window, |window, cx| window.press("shift-alt-f", cx));
        cx.run_until_parked();
        // The text given by `nxc ide format` (its layout is the compiler's business).
        let expected = ide::format(&compiler::find_nxc().unwrap(), &dir.join("main.nx"), "fn main(){\n  println(\"ok\")}\n".into()).unwrap();
        assert!(expected.starts_with("fn main() {\n    println"), "{:?}", expected);
        studio.read_with(cx, |s, cx| assert_eq!(s.docs[0].editor.read(cx).value().as_ref(), expected));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn answers_of_nxc_ide() {
        let Some(nxc) = compiler::find_nxc() else { return eprintln!("nxc introuvable : test sauté") };
        let dir = project("ide");
        let file = dir.join("main.nx");
        let text = "fn carre(x: i32) -> i32 {\n    return x * x\n}\n\nfn main() {\n    let s = \"abc\"\n    println(carre(4))\n}\n".to_string();
        let hover = ide::ide(&nxc, "hover", &file, Some((7, 14)), None, text.clone()).expect("hover");
        assert!(hover["code"].as_str().unwrap().starts_with("fn carre"), "{}", hover);
        let def = ide::ide(&nxc, "definition", &file, Some((7, 14)), None, text.clone()).expect("definition");
        assert_eq!(ide::location_path(&def, &file).as_deref(), Some(file.as_path()));
        assert_eq!(ide::range_of(&def).start, Position::new(0, 3));
        let members = ide::ide(&nxc, "completion", &file, Some((7, 7)), None, text.replace("    println(carre(4))", "    s.")).expect("completion");
        let labels: Vec<&str> = members.as_array().unwrap().iter().filter_map(|c| c["label"].as_str()).collect();
        assert!(labels.contains(&"chars"), "{:?}", labels);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[gpui_kit::test]
    fn closing_a_modified_file_saves_it(cx: &mut TestAppContext) {
        let dir = project("close");
        let (window, studio) = open(cx, dir.clone());
        update(cx, window, &studio, |s, window, cx| s.open_file("src/calcul.nx", window, cx));
        keys(cx, window, |window, cx| {
            window.press("ctrl-end", cx);
            window.input("// fin\n", cx);
            window.press("ctrl-w", cx);
        });
        studio.read_with(cx, |s, _| assert!(s.docs.is_empty()));
        assert!(std::fs::read_to_string(dir.join("src/calcul.nx")).unwrap().ends_with("// fin\n"));
    }
}

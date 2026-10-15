//! The main window of NEXUL Studio: title bar with the menus, activity bar, explorer,
//! editors in tabs, output panel and status bar.

use crate::explorer;
use crate::settings::Settings;
use crate::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Editor, EditorState, InputEvent},
    list::ListItem,
    menu::AppMenuBar,
    resizable::{h_resizable, resizable_panel, v_resizable},
    tab::{Tab, TabBar},
    tree::{tree, TreeEvent, TreeState},
    status_bar::StatusBar,
    v_flex, ActiveTheme as _, IconName, Selectable as _, Sizable as _, Theme, ThemeMode, TitleBar, WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use nexul_studio::{compiler, runner, watch, workspace};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// An open file.
struct Doc {
    rel: String,
    name: SharedString,
    lang: &'static str,
    editor: Entity<EditorState>,
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
        let (items, dirs) = explorer::build(&root, &self.expanded);
        self.dirs = dirs;
        self.tree.update(cx, |t, cx| t.set_items(items, cx));
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
        self.docs.push(Doc { rel: rel.to_string(), name, lang, editor, encoding, dirty: false, saved_hash, reloading: false, _changes: changes });
        self.activate(self.docs.len() - 1, window, cx);
        self.save_session();
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
        let title = self.root.as_ref().and_then(|r| r.file_name()).map(|n| n.to_string_lossy().to_uppercase()).unwrap_or_else(|| "AUCUN DOSSIER".into());
        let header = h_flex()
            .h(px(32.))
            .px_3()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(title);
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
        let studio = cx.entity().downgrade();
        let dirs = self.dirs.clone();
        let list = tree(&self.tree, move |ix, entry, selected, _window, _cx| {
            let rel = entry.item().id.to_string();
            let is_dir = dirs.contains(&rel);
            let icon = if is_dir {
                if entry.is_expanded() { IconName::FolderOpen } else { IconName::Folder }
            } else {
                IconName::File
            };
            let studio = studio.clone();
            ListItem::new(ix)
                .selected(selected)
                .py_0p5()
                .pl(px(10. + entry.depth() as f32 * 14.))
                .child(h_flex().gap_1p5().text_sm().child(gpui_kit::component::Icon::new(icon).xsmall()).child(entry.item().label.clone()))
                .on_click(move |_, window, cx| {
                    if !is_dir {
                        let rel = rel.clone();
                        let _ = studio.update(cx, |this, cx| this.open_file(&rel, window, cx));
                    }
                })
        });
        v_flex().size_full().bg(theme.sidebar).child(header).child(div().flex_1().min_h_0().child(list))
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
            .child(div().flex_1().min_h_0().child(Editor::new(&doc.editor).bordered(false).h_full()))
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
                    .px_3()
                    .gap_2()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("SORTIE")
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
            .child(div().flex_1().min_h_0().child(Editor::new(&self.output).bordered(false).readonly(true).h_full()))
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
        let running = if self.run.is_some() { "Exécution…" } else { "" };
        StatusBar::new()
            .left(h_flex().gap_3().text_xs().child("NEXUL").child(running))
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

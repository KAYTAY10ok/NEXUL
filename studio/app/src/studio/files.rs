//! Operations of the explorer on files and folders: create, rename, delete (to the
//! recycle bin), cut, copy and paste, drag and drop, paths. The work itself is done by
//! `nexul_studio::workspace`; this module keeps the open editors and the tree in step.

use super::*;
use gpui_kit::component::input::InputState;
use nexul_studio::workspace::Kind;

/// What the name box of the explorer is for.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Target {
    New { dir: String, kind: Kind },
    Rename { rel: String },
}

/// The name box, shown in the tree in place of the label of a row.
pub(super) struct Naming {
    pub(super) target: Target,
    pub(super) input: Entity<InputState>,
    _events: Subscription,
}

impl Naming {
    /// The id of the row of the tree that shows the box.
    pub(super) fn row(&self) -> SharedString {
        match &self.target {
            Target::New { .. } => explorer::NEW_ENTRY.into(),
            Target::Rename { rel } => rel.clone().into(),
        }
    }
}

/// Entries copied or cut in the explorer.
#[derive(Clone, Debug)]
pub(super) struct Clip {
    pub(super) items: Vec<String>,
    pub(super) cut: bool,
}

/// Entries of the explorer being dragged (the view is the label under the pointer).
#[derive(Clone)]
pub(super) struct Dragged {
    pub(super) items: Vec<String>,
    label: SharedString,
}

impl Dragged {
    pub(super) fn new(rel: &str) -> Dragged {
        Dragged { items: vec![rel.to_string()], label: name_of(rel).to_string().into() }
    }
}

impl Render for Dragged {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .text_sm()
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .border_1()
            .border_color(cx.theme().border)
            .child(self.label.clone())
    }
}

/// The last name of a relative path.
pub(super) fn name_of(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

/// The folder of a relative path (`""` for the root).
pub(super) fn parent_of(rel: &str) -> &str {
    rel.rfind('/').map(|i| &rel[..i]).unwrap_or("")
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", dir, name)
    }
}

/// `rel` after `old` became `new` (itself or something inside it), else `None`.
fn remap(rel: &str, old: &str, new: &str) -> Option<String> {
    if rel == old {
        Some(new.to_string())
    } else {
        rel.strip_prefix(old).filter(|rest| rest.starts_with('/')).map(|rest| format!("{}{}", new, rest))
    }
}

impl Studio {
    /// The selected entry of the explorer.
    pub(super) fn selected(&self, cx: &App) -> Option<String> {
        self.tree.read(cx).selected_item().map(|i| i.id.to_string()).filter(|id| id != explorer::NEW_ENTRY)
    }

    /// The folder for a new entry or a paste: the selected folder, else the folder of the
    /// selected file, else the root.
    pub(super) fn target_dir(&self, cx: &App) -> String {
        match self.selected(cx) {
            Some(rel) if self.dirs.contains(&rel) => rel,
            Some(rel) => parent_of(&rel).to_string(),
            None => String::new(),
        }
    }

    /// Selects an entry of the tree, unfolding its folders, and scrolls to it.
    pub(super) fn select(&mut self, rel: &str, cx: &mut Context<Self>) {
        let id: SharedString = rel.to_string().into();
        self.tree.update(cx, |t, cx| {
            t.reveal_item(&id, ScrollStrategy::Center, cx);
            if let Some(ix) = t.index_of(&id) {
                t.set_selected_index(Some(ix), cx);
            }
        });
    }

    fn focus_tree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tree.update(cx, |t, cx| t.focus(window, cx));
    }

    // -------------------------------------------------------------- name box

    /// Shows the name box for a new file or folder in `dir`.
    pub(super) fn start_new(&mut self, dir: String, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        if self.root.is_none() {
            return;
        }
        if !dir.is_empty() {
            self.expanded.insert(dir.clone());
        }
        self.settings.sidebar = true;
        self.settings.minimal = false;
        self.start_naming(Target::New { dir, kind }, String::new(), 0, window, cx);
    }

    /// Shows the name box over an entry, its name selected without the extension.
    pub(super) fn start_rename(&mut self, rel: String, window: &mut Window, cx: &mut Context<Self>) {
        let name = name_of(&rel).to_string();
        let stem = if self.dirs.contains(&rel) { name.len() } else { name.rfind('.').filter(|&i| i > 0).unwrap_or(name.len()) };
        self.start_naming(Target::Rename { rel }, name, stem, window, cx);
    }

    fn start_naming(&mut self, target: Target, value: String, select: usize, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
        let events = cx.subscribe_in(&input, window, |this: &mut Self, _, event: &InputEvent, window, cx| match event {
            InputEvent::PressEnter { .. } => this.commit_naming(true, window, cx),
            // Clicking elsewhere keeps the name typed, as in Visual Studio Code.
            InputEvent::Blur => this.commit_naming(false, window, cx),
            _ => {}
        });
        input.update(cx, |i, cx| {
            i.focus(window, cx);
            i.set_selected_range(0..select, cx);
        });
        self.naming = Some(Naming { target, input, _events: events });
        self.refresh_tree(cx);
        let row = self.naming.as_ref().map(|n| n.row()).unwrap_or_default();
        self.select(&row, cx);
        cx.notify();
    }

    /// Escape: the name box goes away, nothing is changed.
    pub(super) fn cancel_naming(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.naming.take().is_none() {
            return false;
        }
        self.refresh_tree(cx);
        self.focus_tree(window, cx);
        cx.notify();
        true
    }

    /// Creates or renames with the name typed. With `keep_on_error` (Enter), a refused name
    /// leaves the box open to correct it; otherwise (focus lost) the box goes away.
    pub(super) fn commit_naming(&mut self, keep_on_error: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(naming), Some(root)) = (self.naming.take(), self.root.clone()) else { return };
        let name = naming.input.read(cx).value().trim().to_string();
        let result = match &naming.target {
            Target::New { dir, kind } if !name.is_empty() => workspace::create(&root, &join(dir, &name), *kind, b"").map(Some),
            Target::Rename { rel } if !name.is_empty() && name != name_of(rel) => {
                let to = join(parent_of(rel), &name);
                workspace::rename(&root, rel, &to).map(|new| {
                    self.moved(rel, &new, cx);
                    Some(new)
                })
            }
            _ => Ok(None),
        };
        match result {
            Err(e) if keep_on_error => {
                self.notify_error(&e.message, window, cx);
                naming.input.update(cx, |i, cx| i.focus(window, cx));
                self.naming = Some(naming);
                return;
            }
            Err(e) => {
                self.notify_error(&e.message, window, cx);
                self.refresh_tree(cx);
                self.focus_tree(window, cx);
            }
            Ok(Some(rel)) => {
                self.refresh_tree(cx);
                self.select(&rel, cx);
                if matches!(naming.target, Target::New { kind: Kind::File, .. }) {
                    self.open_file(&rel, window, cx);
                } else {
                    self.focus_tree(window, cx);
                }
            }
            Ok(None) => {
                self.refresh_tree(cx);
                self.focus_tree(window, cx);
            }
        }
        cx.notify();
    }

    /// Follows an entry renamed or moved from `old` to `new`: open editors, unfolded
    /// folders, selection.
    pub(super) fn moved(&mut self, old: &str, new: &str, cx: &mut Context<Self>) {
        for doc in &mut self.docs {
            if let Some(rel) = remap(&doc.rel, old, new) {
                if let Some(root) = &self.root {
                    *doc.file.borrow_mut() = root.join(&rel);
                }
                doc.name = name_of(&rel).to_string().into();
                let lang = explorer::language_of(&doc.name);
                if lang != doc.lang {
                    doc.lang = lang;
                    doc.editor.update(cx, |e, cx| e.set_highlighter(lang, cx));
                }
                doc.rel = rel;
            }
        }
        self.expanded = self.expanded.iter().map(|d| remap(d, old, new).unwrap_or_else(|| d.clone())).collect();
        self.save_session();
    }

    // -------------------------------------------------------------- delete

    /// Asks before sending an entry to the recycle bin.
    pub(super) fn confirm_delete(&mut self, rel: String, window: &mut Window, cx: &mut Context<Self>) {
        let studio = cx.entity().downgrade();
        let is_dir = self.dirs.contains(&rel);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let studio = studio.clone();
            let rel = rel.clone();
            alert
                .title(format!("Supprimer « {} » ?", name_of(&rel)))
                .description(if is_dir {
                    "Le dossier et tout son contenu seront envoyés à la Corbeille."
                } else {
                    "Le fichier sera envoyé à la Corbeille."
                })
                .show_cancel(true)
                .ok_text("Envoyer à la Corbeille")
                .cancel_text("Annuler")
                .on_ok(move |_, window, cx| {
                    let _ = studio.update(cx, |this, cx| this.trash_entries(&[rel.clone()], window, cx));
                    true
                })
        });
    }

    /// Sends entries to the recycle bin; their editors without unsaved changes are closed.
    pub(super) fn trash_entries(&mut self, items: &[String], window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        for rel in items {
            if let Err(e) = workspace::trash(&root, rel) {
                self.notify_error(&e.message, window, cx);
                continue;
            }
            let gone: Vec<usize> = (0..self.docs.len()).rev().filter(|&i| remap(&self.docs[i].rel, rel, rel).is_some() && !self.docs[i].dirty).collect();
            for i in gone {
                self.docs.remove(i);
            }
            self.expanded.retain(|d| remap(d, rel, rel).is_none());
        }
        self.active = if self.docs.is_empty() { None } else { Some(self.active.unwrap_or(0).min(self.docs.len() - 1)) };
        self.save_session();
        self.refresh_tree(cx);
        cx.notify();
    }

    // -------------------------------------------------------------- clipboard

    /// Copy or cut: the entries are kept for a paste, and their full paths go to the
    /// clipboard of the system as text.
    pub(super) fn set_clip(&mut self, items: Vec<String>, cut: bool, cx: &mut Context<Self>) {
        let Some(root) = &self.root else { return };
        let text = items.iter().map(|r| root.join(r).to_string_lossy().to_string()).collect::<Vec<_>>().join("\n");
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.clip = Some(Clip { items, cut });
        cx.notify();
    }

    /// Paste into `dir`: files copied in the Explorer of the system, else the entries
    /// copied or cut in Studio.
    pub(super) fn paste(&mut self, dir: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let external = cx.read_from_clipboard().and_then(|c| {
            c.entries().iter().find_map(|e| match e {
                ClipboardEntry::ExternalPaths(p) if !p.paths().is_empty() => Some(p.paths().to_vec()),
                _ => None,
            })
        });
        let result = if let Some(paths) = external {
            workspace::import_paths(&root, &dir, &paths)
        } else if let Some(clip) = self.clip.clone() {
            if clip.cut {
                self.clip = None;
                self.move_entries(clip.items, dir, window, cx);
                return;
            }
            workspace::copy_into(&root, &clip.items, &dir)
        } else {
            return;
        };
        self.after_add(result, &dir, window, cx);
    }

    /// The tree after new entries were added to `dir`: the first one is selected.
    fn after_add(&mut self, result: workspace::FsResult<Vec<String>>, dir: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !dir.is_empty() {
            self.expanded.insert(dir.to_string());
        }
        self.refresh_tree(cx);
        match result {
            Ok(added) => {
                if let Some(first) = added.first() {
                    self.select(first, cx);
                }
            }
            Err(e) => self.notify_error(&e.message, window, cx),
        }
        cx.notify();
    }

    // -------------------------------------------------------------- move, import

    /// Moves entries into `dir` (drag and drop, cut and paste).
    pub(super) fn move_entries(&mut self, items: Vec<String>, dir: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        match workspace::move_into(&root, &items, &dir) {
            Ok(pairs) => {
                for (old, new) in &pairs {
                    self.moved(old, new, cx);
                }
                let added = pairs.into_iter().map(|(_, new)| new).collect();
                self.after_add(Ok(added), &dir, window, cx);
            }
            Err(e) => self.after_add(Err(e), &dir, window, cx),
        }
    }

    /// Copies entries into `dir` (drag and drop with Ctrl).
    pub(super) fn copy_entries(&mut self, items: Vec<String>, dir: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let result = workspace::copy_into(&root, &items, &dir);
        self.after_add(result, &dir, window, cx);
    }

    /// Files and folders dropped from the Explorer of the system are copied into `dir`.
    pub(super) fn import_external(&mut self, paths: &[PathBuf], dir: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let result = workspace::import_paths(&root, &dir, paths);
        self.after_add(result, &dir, window, cx);
    }

    /// A drop of entries of the explorer: Ctrl copies, otherwise they move.
    pub(super) fn drop_entries(&mut self, dragged: &Dragged, dir: String, window: &mut Window, cx: &mut Context<Self>) {
        // A folder released on itself: nothing to do.
        if dragged.items.contains(&dir) {
            return;
        }
        if window.modifiers().control {
            self.copy_entries(dragged.items.clone(), dir, window, cx);
        } else {
            self.move_entries(dragged.items.clone(), dir, window, cx);
        }
    }

    // -------------------------------------------------------------- paths

    pub(super) fn full_path(&self, rel: &str) -> Option<PathBuf> {
        let root = self.root.as_ref()?;
        Some(if rel.is_empty() { root.clone() } else { root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR)) })
    }

    pub(super) fn copy_path(&mut self, rel: &str, relative: bool, cx: &mut Context<Self>) {
        let text = if relative { rel.to_string() } else { self.full_path(rel).map(|p| p.to_string_lossy().to_string()).unwrap_or_default() };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    /// Shows the entry in the file manager of the system.
    pub(super) fn reveal(&mut self, rel: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.full_path(rel) else { return };
        let result = reveal_in_file_manager(&p);
        if let Err(e) = result {
            self.notify_error(&format!("Impossible d'ouvrir l'Explorateur : {}", e), window, cx);
        }
    }
}

#[cfg(windows)]
fn reveal_in_file_manager(p: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt as _;
    // `/select,"chemin"` exactly: Explorer does not read the quoting of Rust.
    std::process::Command::new("explorer.exe").raw_arg(format!("/select,\"{}\"", p.display())).spawn().map(|_| ())
}

#[cfg(target_os = "macos")]
fn reveal_in_file_manager(p: &Path) -> std::io::Result<()> {
    std::process::Command::new("open").arg("-R").arg(p).spawn().map(|_| ())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn reveal_in_file_manager(p: &Path) -> std::io::Result<()> {
    let dir = if p.is_dir() { p } else { p.parent().unwrap_or(p) };
    std::process::Command::new("xdg-open").arg(dir).spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use core::prelude::v1::test;

    #[test]
    fn paths() {
        assert_eq!(name_of("a/b/c.nx"), "c.nx");
        assert_eq!(parent_of("a/b/c.nx"), "a/b");
        assert_eq!(parent_of("c.nx"), "");
        assert_eq!(join("", "x"), "x");
        assert_eq!(join("a", "x"), "a/x");
        assert_eq!(remap("a/b/c.nx", "a/b", "z").as_deref(), Some("z/c.nx"));
        assert_eq!(remap("a/b", "a/b", "z").as_deref(), Some("z"));
        assert_eq!(remap("a/bc", "a/b", "z"), None);
    }
}

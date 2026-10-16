//! No lost work. Closing an editor or the window with unsaved changes asks first
//! (Enregistrer / Ne pas enregistrer / Annuler); a file changed on disk since it was read
//! is not overwritten without asking; the unsaved texts are copied to a recovery folder
//! and come back after a crash; automatic saving is a setting, off by default.

use super::*;
use crate::settings;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::time::Duration;

/// The answer to « Enregistrer les modifications ? ».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    /// The first button (Enregistrer, Écraser…), also Enter.
    Save,
    /// The second button (Ne pas enregistrer, Recharger…).
    Discard,
    /// Annuler, also Escape: nothing happens.
    Cancel,
}

/// The copy of an unsaved text, in the recovery folder.
#[derive(Serialize, Deserialize)]
struct Backup {
    root: String,
    rel: String,
    encoding: String,
    text: String,
    /// The file on disk that the changes started from.
    disk_hash: u64,
}

/// FNV-1a: the same value in every version of Studio (names of the recovery files).
fn stable_hash(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

fn backup_path(root: &Path, rel: &str) -> PathBuf {
    let key = format!("{}\n{}", root.to_string_lossy().to_lowercase(), rel.to_lowercase());
    settings::dir().join("recovery").join(format!("{:016x}.json", stable_hash(&key)))
}

/// Where Studio goes once the unsaved changes are dealt with.
#[derive(Clone, Debug)]
pub(super) enum Leave {
    Window,
    Root(PathBuf),
}

/// A question with three answers, in a dialog: Enter gives the first, Escape cancels.
pub(super) fn ask(window: &mut Window, cx: &mut App, title: String, body: String, first: &'static str, second: &'static str, on_choice: impl Fn(Choice, &mut Window, &mut App) + 'static) {
    let on_choice = Rc::new(on_choice);
    window.open_dialog(cx, move |dialog, _, _| {
        let (c1, c2, c3, c4, c5) = (on_choice.clone(), on_choice.clone(), on_choice.clone(), on_choice.clone(), on_choice.clone());
        dialog
            .title(title.clone())
            .w(px(480.))
            .close_button(false)
            .child(div().text_sm().whitespace_normal().child(body.clone()))
            .on_ok(move |_, window, cx| {
                // Closed first: the answer may open another dialog.
                window.close_dialog(cx);
                c1(Choice::Save, window, cx);
                false
            })
            .on_cancel(move |_, window, cx| {
                c4(Choice::Cancel, window, cx);
                true
            })
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .justify_end()
                    .child(Button::new("ask-second").label(second).on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        c2(Choice::Discard, window, cx);
                    }))
                    .child(Button::new("ask-cancel").label("Annuler").on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        c5(Choice::Cancel, window, cx);
                    }))
                    .child(Button::new("ask-first").primary().label(first).on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        c3(Choice::Save, window, cx);
                    })),
            )
    });
}

impl Studio {
    /// The text differs from the file: the flag, and the text itself (the last keystroke
    /// may not have been reported yet).
    pub(super) fn is_modified(&self, ix: usize, cx: &App) -> bool {
        self.docs.get(ix).is_some_and(|d| d.dirty || text_hash(&d.editor.read(cx).value()) != d.saved_hash)
    }

    fn modified_docs(&self, cx: &App) -> Vec<usize> {
        (0..self.docs.len()).filter(|&i| self.is_modified(i, cx)).collect()
    }

    pub(super) fn notify_info(&mut self, message: &str, window: &mut Window, cx: &mut Context<Self>) {
        let note = gpui_kit::component::notification::Notification::info(message.to_string());
        cx.defer_in(window, move |_, window, cx| window.push_notification(note, cx));
    }

    // -------------------------------------------------------------- closing an editor

    /// Closes an editor; with unsaved changes, asks first.
    pub(super) fn request_close_doc(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(doc) = self.docs.get(ix) else { return };
        let rel = doc.rel.clone();
        if !self.is_modified(ix, cx) {
            self.delete_backup(&rel);
            return self.remove_doc(ix, window, cx);
        }
        let name = doc.name.to_string();
        self.activate(ix, window, cx);
        let studio = cx.entity().downgrade();
        ask(
            window,
            cx,
            format!("Enregistrer les modifications de « {} » ?", name),
            "Vos modifications seront perdues si vous ne les enregistrez pas.".into(),
            "Enregistrer",
            "Ne pas enregistrer",
            move |choice, window, cx| {
                let _ = studio.update(cx, |this, cx| this.finish_close_doc(&rel, choice, window, cx));
            },
        );
    }

    pub(super) fn finish_close_doc(&mut self, rel: &str, choice: Choice, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.docs.iter().position(|d| d.rel == rel) else { return };
        match choice {
            Choice::Save => {
                if self.save_doc(ix, window, cx) {
                    self.remove_doc(ix, window, cx);
                }
            }
            Choice::Discard => {
                self.delete_backup(rel);
                self.remove_doc(ix, window, cx);
            }
            Choice::Cancel => {}
        }
    }

    // -------------------------------------------------------------- leaving the folder

    /// Asked by the system before the window closes (title bar, Alt+F4) and by Quit.
    /// With unsaved changes: `false`, and a dialog that closes the window itself.
    pub(super) fn window_should_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.allow_close {
            return true;
        }
        let free = self.ask_unsaved(Leave::Window, window, cx);
        if free {
            self.save_session();
        }
        free
    }

    /// Opens another folder; with unsaved changes, asks first.
    pub(super) fn request_open_root(&mut self, root: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.ask_unsaved(Leave::Root(root.clone()), window, cx) {
            self.open_root(root, window, cx);
        }
    }

    /// `true` when no editor has unsaved changes; else asks what to do with them and
    /// `false`: the answer leaves (or not) by itself.
    fn ask_unsaved(&mut self, leave: Leave, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let modified = self.modified_docs(cx);
        if modified.is_empty() {
            return true;
        }
        let names: Vec<String> = modified.iter().map(|&i| self.docs[i].rel.clone()).collect();
        let when = match leave {
            Leave::Window => "avant de fermer",
            Leave::Root(_) => "avant de changer de dossier",
        };
        let (title, first) = if names.len() == 1 {
            (format!("Enregistrer les modifications de « {} » {} ?", files::name_of(&names[0]), when), "Enregistrer")
        } else {
            (format!("Enregistrer les modifications de {} fichiers {} ?", names.len(), when), "Tout enregistrer")
        };
        let mut body = String::new();
        if names.len() > 1 {
            for n in names.iter().take(8) {
                body.push_str(&format!("• {}\n", n));
            }
            if names.len() > 8 {
                body.push_str(&format!("• … et {} autres\n", names.len() - 8));
            }
        }
        body.push_str("Les modifications non enregistrées seront perdues si vous ne les enregistrez pas.");
        let studio = cx.entity().downgrade();
        let leave = Rc::new(leave);
        ask(window, cx, title, body, first, "Ne pas enregistrer", move |choice, window, cx| {
            let leave = (*leave).clone();
            let _ = studio.update(cx, |this, cx| this.finish_leave(choice, leave, window, cx));
        });
        false
    }

    pub(super) fn finish_leave(&mut self, choice: Choice, leave: Leave, window: &mut Window, cx: &mut Context<Self>) {
        let modified = self.modified_docs(cx);
        match choice {
            Choice::Save => {
                for ix in modified {
                    // A failure or a conflict stays here.
                    if !self.save_doc(ix, window, cx) {
                        return;
                    }
                }
            }
            Choice::Discard => {
                for ix in modified {
                    let rel = self.docs[ix].rel.clone();
                    self.delete_backup(&rel);
                }
            }
            Choice::Cancel => return,
        }
        match leave {
            Leave::Window => {
                self.save_session();
                self.allow_close = true;
                window.remove_window();
            }
            Leave::Root(root) => {
                // Discarded changes must not count as modified any more.
                for d in &mut self.docs {
                    d.dirty = false;
                }
                self.open_root(root, window, cx);
            }
        }
    }
    pub(super) fn on_quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        if self.window_should_close(window, cx) {
            self.allow_close = true;
            window.remove_window();
        }
    }

    // -------------------------------------------------------------- conflicts

    /// The file on disk is not the one read or saved last (changed by another program).
    pub(super) fn changed_on_disk(&self, ix: usize) -> bool {
        let (Some(root), Some(doc)) = (&self.root, self.docs.get(ix)) else { return false };
        match std::fs::read(root.join(&doc.rel)) {
            Ok(bytes) => bytes_hash(&bytes) != doc.disk_hash,
            // Deleted or moved: saving writes it again.
            Err(_) => false,
        }
    }

    pub(super) fn ask_conflict(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(doc) = self.docs.get(ix) else { return };
        let (rel, name) = (doc.rel.clone(), doc.name.to_string());
        let studio = cx.entity().downgrade();
        ask(
            window,
            cx,
            format!("« {} » a été modifié en dehors de Studio", name),
            "Le fichier sur le disque a changé depuis son ouverture. Écraser le remplace par votre version ; Recharger abandonne vos modifications et lit la version du disque.".into(),
            "Écraser",
            "Recharger",
            move |choice, window, cx| {
                let _ = studio.update(cx, |this, cx| this.finish_conflict(&rel, choice, window, cx));
            },
        );
    }

    pub(super) fn finish_conflict(&mut self, rel: &str, choice: Choice, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.docs.iter().position(|d| d.rel == rel) else { return };
        match choice {
            Choice::Save => {
                self.save_doc_as_is(ix, true, window, cx);
            }
            Choice::Discard => self.reload_from_disk(ix, window, cx),
            Choice::Cancel => {}
        }
    }

    /// Reads the file again, dropping the changes of the editor.
    fn reload_from_disk(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let Some(doc) = self.docs.get_mut(ix) else { return };
        let rel = doc.rel.clone();
        let bytes = match std::fs::read(root.join(&rel)) {
            Ok(b) => b,
            Err(e) => return self.notify_error(&workspace::io_message(&e, &rel).message, window, cx),
        };
        let Some((text, encoding)) = workspace::decode_text(&bytes) else { return };
        doc.encoding = encoding;
        doc.saved_hash = text_hash(&text);
        doc.disk_hash = bytes_hash(&bytes);
        doc.reloading = true;
        doc.dirty = false;
        doc.editor.update(cx, |e, cx| e.set_value(text, window, cx));
        self.delete_backup(&rel);
        cx.notify();
    }

    // -------------------------------------------------------------- recovery

    /// Several times a second: copies of the unsaved texts, automatic saving.
    pub(super) fn tick_safety(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.backup_due.is_some_and(|t| t.elapsed() >= Duration::from_millis(800)) {
            self.backup_due = None;
            self.write_backups(cx);
        }
        if self.settings.autosave == "delay" && self.last_change.is_some_and(|t| t.elapsed() >= Duration::from_secs(1)) {
            self.last_change = None;
            for ix in self.modified_docs(cx) {
                // A conflict is never resolved by automatic saving.
                if !self.changed_on_disk(ix) {
                    self.save_doc(ix, window, cx);
                }
            }
        }
    }

    /// Copies the unsaved texts to the recovery folder.
    pub(super) fn write_backups(&mut self, cx: &App) {
        let Some(root) = self.root.clone() else { return };
        for ix in self.modified_docs(cx) {
            let doc = &self.docs[ix];
            let backup = Backup {
                root: root.to_string_lossy().to_string(),
                rel: doc.rel.clone(),
                encoding: doc.encoding.to_string(),
                text: doc.editor.read(cx).value().to_string(),
                disk_hash: doc.disk_hash,
            };
            let path = backup_path(&root, &doc.rel);
            if let (Some(dir), Ok(json)) = (path.parent(), serde_json::to_string(&backup)) {
                let _ = std::fs::create_dir_all(dir);
                let tmp = path.with_extension("tmp");
                if std::fs::write(&tmp, json).is_ok() {
                    let _ = std::fs::rename(&tmp, &path);
                }
            }
        }
    }

    pub(super) fn delete_backup(&self, rel: &str) {
        if let Some(root) = &self.root {
            let _ = std::fs::remove_file(backup_path(root, rel));
        }
    }

    /// Puts back the unsaved text of a file found in the recovery folder.
    pub(super) fn restore_backup(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let Some(doc) = self.docs.get_mut(ix) else { return };
        let path = backup_path(&root, &doc.rel);
        let Some(backup) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Backup>(&t).ok()) else { return };
        if backup.rel != doc.rel || doc.editor.read(cx).value().as_ref() == backup.text {
            let _ = std::fs::remove_file(&path);
            return;
        }
        doc.encoding = workspace::ENCODINGS.iter().copied().find(|e| *e == backup.encoding).unwrap_or(doc.encoding);
        // Saving compares the disk with the file the changes started from.
        doc.disk_hash = backup.disk_hash;
        doc.editor.update(cx, |e, cx| e.set_value(backup.text, window, cx));
        doc.dirty = true;
        let name = doc.name.to_string();
        self.notify_info(&format!("Modifications non enregistrées de « {} » récupérées.", name), window, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use core::prelude::v1::test;

    #[test]
    fn stable_names() {
        // The name of a recovery file must not change between versions of Studio.
        assert_eq!(stable_hash(""), 0xcbf29ce484222325);
        assert_eq!(stable_hash("a"), 0xaf63dc4c8601ec8c);
    }
}

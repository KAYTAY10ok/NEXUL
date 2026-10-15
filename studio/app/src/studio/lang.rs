//! The NEXUL services in Studio: hover, completion and definition in the editors
//! (`ide::Nexul`), diagnostics of `nxc check` (underlined in the editors and listed in the
//! Problems panel), renaming a symbol, formatting.

use super::*;
use crate::ide::{self, Problem, Severity};
use gpui_kit::base::input::{Diagnostic, DiagnosticSeverity};
use gpui_kit::component::input::{InputState, Position, RopeExt as _, ShowDocumentHandler};
use std::cell::Cell;
use std::rc::Rc;

/// The tabs of the bottom panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PanelTab {
    Output,
    Problems,
}

impl Studio {
    /// Gives an editor of a NEXUL file the services of nxc.
    pub(super) fn attach_services(&self, editor: &Entity<EditorState>, file: &ide::FileRef, cx: &mut Context<Self>) {
        let Some(nxc) = self.nxc.clone() else { return };
        let services = Rc::new(ide::Nexul { nxc, file: file.clone() });
        let studio = cx.entity().downgrade();
        let shown = file.clone();
        // Go to a definition in another file: Studio opens it.
        let show: ShowDocumentHandler = Rc::new(move |params, window, cx| {
            let Some(path) = ide::uri_path(&params.uri) else { return false };
            if ide::same_file(&path, &shown.borrow()) {
                return false;
            }
            let at = params.selection.map(|r| r.start);
            studio.update(cx, |this, cx| this.open_path_at(&path, at, window, cx)).unwrap_or(false)
        });
        editor.update(cx, |e, _| {
            let lsp = e.lsp_mut();
            lsp.hover_provider = Some(services.clone());
            lsp.definition_provider = Some(services.clone());
            lsp.completion_provider = Some(services);
            lsp.show_document = Some(show);
        });
    }

    /// Opens a file of the folder (full path) with the cursor at `at`.
    pub(super) fn open_path_at(&mut self, path: &Path, at: Option<Position>, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(root) = self.root.clone() else { return false };
        let Ok(rel) = path.strip_prefix(&root) else {
            self.notify_error(&format!("« {} » est en dehors du dossier ouvert.", path.display()), window, cx);
            return false;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        self.open_file(&rel, window, cx);
        if let (Some(at), Some(doc)) = (at, self.docs.iter().find(|d| d.rel == rel)) {
            doc.editor.update(cx, |e, cx| e.set_cursor_position(at, window, cx));
        }
        true
    }

    // -------------------------------------------------------------- diagnostics

    /// Checks a NEXUL file with `nxc check` (outside of the interface thread); the
    /// diagnostics replace those of its previous check.
    pub(super) fn check_file(&mut self, rel: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(nxc), Some(root)) = (self.nxc.clone(), self.root.clone()) else { return };
        if explorer::language_of(rel) != "nexul" {
            return;
        }
        let file = root.join(rel);
        let rel = rel.to_string();
        self.checking += 1;
        let task = cx.background_spawn(async move { ide::check(&nxc, &file) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.checking -= 1;
                match result {
                    Ok(problems) => this.set_problems(&rel, problems, cx),
                    Err(message) => this.notify_error(&message, window, cx),
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn set_problems(&mut self, source: &str, problems: Vec<Problem>, cx: &mut Context<Self>) {
        self.problems.retain(|(s, _)| s != source);
        for p in problems {
            // The same diagnostic found by the check of another file is listed once.
            if !self.problems.iter().any(|(_, q)| q == &p) {
                self.problems.push((source.to_string(), p));
            }
        }
        self.problems.sort_by(|a, b| (&a.1.path, a.1.range.start.line, a.1.range.start.character).cmp(&(&b.1.path, b.1.range.start.line, b.1.range.start.character)));
        self.show_diagnostics(cx);
        cx.notify();
    }

    /// Underlines the problems in the open editors.
    pub(super) fn show_diagnostics(&mut self, cx: &mut Context<Self>) {
        for doc in &self.docs {
            let path = doc.file.borrow().clone();
            let mine: Vec<Diagnostic> = self
                .problems
                .iter()
                .filter(|(_, p)| ide::same_file(&p.path, &path))
                .map(|(_, p)| {
                    Diagnostic::new(p.range.start..p.range.end, p.message.clone())
                        .with_severity(match p.severity {
                            Severity::Error => DiagnosticSeverity::Error,
                            Severity::Warning => DiagnosticSeverity::Warning,
                            Severity::Info => DiagnosticSeverity::Info,
                        })
                        .with_code(p.code.clone())
                        .with_source("nxc")
                })
                .collect();
            doc.editor.update(cx, |e, cx| {
                let text = e.text().clone();
                if let Some(set) = e.diagnostics_mut() {
                    set.reset(&text);
                    set.extend(mine);
                }
                // The lines are laid out again with the underlines.
                e.refresh(cx);
            });
        }
    }

    /// Number of errors and of warnings.
    pub(super) fn problem_counts(&self) -> (usize, usize) {
        let errors = self.problems.iter().filter(|(_, p)| p.severity == Severity::Error).count();
        let warnings = self.problems.iter().filter(|(_, p)| p.severity == Severity::Warning).count();
        (errors, warnings)
    }

    pub(super) fn show_panel(&mut self, tab: PanelTab, cx: &mut Context<Self>) {
        self.panel_tab = tab;
        self.settings.panel = true;
        self.settings.minimal = false;
        self.settings.save();
        cx.notify();
    }

    pub(super) fn render_problems(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let root = self.root.clone().unwrap_or_default();
        if self.problems.is_empty() {
            let text = if self.checking > 0 { "Vérification…" } else { "Aucun problème détecté (les fichiers NEXUL sont vérifiés à l'ouverture et à l'enregistrement)." };
            return div().id("problems").size_full().p_3().text_sm().text_color(theme.muted_foreground).child(text);
        }
        let studio = cx.entity().downgrade();
        div().id("problems").size_full().overflow_y_scroll().py_1().children(self.problems.iter().enumerate().map(|(i, (_, p))| {
            let (icon, color) = match p.severity {
                Severity::Error => (IconName::CircleX, theme.danger),
                Severity::Warning => (IconName::TriangleAlert, theme.warning),
                Severity::Info => (IconName::Info, theme.info),
            };
            let rel = p.path.strip_prefix(&root).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_else(|_| p.path.to_string_lossy().to_string());
            let place = format!("{}:{}:{}", rel, p.range.start.line + 1, p.range.start.character + 1);
            let first_line = p.message.lines().next().unwrap_or("").to_string();
            let (path, at, studio) = (p.path.clone(), p.range.start, studio.clone());
            h_flex()
                .id(("problem", i))
                .px_3()
                .py_0p5()
                .gap_2()
                .text_sm()
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .child(gpui_kit::component::Icon::new(icon).xsmall().text_color(color))
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().child(first_line))
                .when(!p.code.is_empty(), |this| this.child(div().text_xs().text_color(theme.muted_foreground).child(p.code.clone())))
                .child(div().text_xs().text_color(theme.muted_foreground).child(place))
                .on_click(move |_, window, cx| {
                    let _ = studio.update(cx, |this, cx| this.open_path_at(&path, Some(at), window, cx));
                })
        }))
    }

    // -------------------------------------------------------------- formatting

    /// Formats the active NEXUL file with nxc (the change can be undone).
    pub(super) fn format_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(nxc) = self.nxc.clone() else { return };
        let Some(doc) = self.active.and_then(|i| self.docs.get(i)) else { return };
        if doc.lang != "nexul" {
            return;
        }
        let (editor, file) = (doc.editor.clone(), doc.file.borrow().clone());
        let text = editor.read(cx).value().to_string();
        let task = cx.background_spawn(async move { ide::format(&nxc, &file, text) });
        cx.spawn_in(window, async move |this, cx| {
            let formatted = task.await;
            let _ = this.update_in(cx, |this, window, cx| match formatted {
                Some(new) => editor.update(cx, |e, cx| {
                    if e.value().as_ref() != new.as_str() {
                        let at = e.cursor_position();
                        e.replace_all(new, window, cx);
                        e.set_cursor_position(at, window, cx);
                    }
                }),
                None => this.notify_error("Mise en forme impossible : corrigez d'abord les erreurs de syntaxe.", window, cx),
            });
        })
        .detach();
    }

    // -------------------------------------------------------------- renaming

    /// Asks the new name of the symbol under the cursor, then renames it everywhere.
    pub(super) fn ask_rename_symbol(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(doc) = self.active.and_then(|i| self.docs.get(i)) else { return };
        if doc.lang != "nexul" || self.nxc.is_none() {
            return;
        }
        let (word, at) = {
            let e = doc.editor.read(cx);
            (e.text().word_at(e.cursor()), e.cursor_position())
        };
        if word.is_empty() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).default_value(word.clone()));
        let studio = cx.entity().downgrade();
        let subscription = cx.subscribe_in(&input, window, move |this: &mut Self, input, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                let name = input.read(cx).value().trim().to_string();
                window.close_dialog(cx);
                this.rename_symbol(at, name, window, cx);
            }
        });
        self.dialog_input = Some(subscription);
        let first = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first.replace(false) {
                input.update(cx, |i, cx| {
                    i.focus(window, cx);
                    i.select_all(window, cx);
                });
            }
            let (input2, studio) = (input.clone(), studio.clone());
            dialog
                .title(format!("Renommer « {} »", word))
                .w(px(420.))
                .child(Input::new(&input))
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(Button::new("cancel-rename").label("Annuler").on_click(|_, window, cx| window.close_dialog(cx)))
                        .child(Button::new("do-rename").primary().label("Renommer").on_click(move |_, window, cx| {
                            let name = input2.read(cx).value().trim().to_string();
                            window.close_dialog(cx);
                            let _ = studio.update(cx, |this, cx| this.rename_symbol(at, name.clone(), window, cx));
                        })),
                )
        });
    }

    /// Renames the symbol at `at` of the active file: nxc gives the places; the open
    /// editors are changed (can be undone), the other files on disk.
    pub(super) fn rename_symbol(&mut self, at: Position, name: String, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog_input = None;
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') || name.starts_with(|c: char| c.is_ascii_digit()) {
            return self.notify_error(&format!("« {} » n'est pas un nom valide.", name), window, cx);
        }
        let (Some(nxc), Some(doc)) = (self.nxc.clone(), self.active.and_then(|i| self.docs.get(i))) else { return };
        let file = doc.file.borrow().clone();
        let text = doc.editor.read(cx).value().to_string();
        let answer = ide::ide(&nxc, "rename", &file, Some((at.line + 1, at.character + 1)), Some(&name), text);
        let Some(places) = answer.as_ref().and_then(|v| v.as_array()).cloned() else {
            let why = answer.as_ref().and_then(|v| v.get("error")).and_then(|e| e.as_str()).unwrap_or("rien à renommer ici");
            return self.notify_error(&format!("Renommage impossible : {}.", why), window, cx);
        };
        // The places, by file.
        let mut by_file: Vec<(PathBuf, Vec<lsp_types::Range>)> = Vec::new();
        for p in &places {
            let Some(path) = ide::location_path(p, &file) else { continue };
            let range = ide::range_of(p);
            match by_file.iter_mut().find(|(f, _)| ide::same_file(f, &path)) {
                Some((_, ranges)) => ranges.push(range),
                None => by_file.push((path, vec![range])),
            }
        }
        for (path, mut ranges) in by_file {
            // From the end, so that the positions before stay valid.
            ranges.sort_by(|a, b| (b.start.line, b.start.character).cmp(&(a.start.line, a.start.character)));
            let edits: Vec<lsp_types::TextEdit> = ranges.into_iter().map(|range| lsp_types::TextEdit { range, new_text: name.clone() }).collect();
            if let Some(doc) = self.docs.iter().find(|d| ide::same_file(&d.file.borrow(), &path)) {
                doc.editor.update(cx, |e, cx| e.apply_lsp_edits(&edits, window, cx));
            } else if let Err(message) = edit_on_disk(&path, &edits) {
                self.notify_error(&message, window, cx);
            }
        }
        cx.notify();
    }
}

/// Applies edits (sorted from the end) to a file that is not open, keeping its encoding.
fn edit_on_disk(path: &Path, edits: &[lsp_types::TextEdit]) -> Result<(), String> {
    let shown = path.display().to_string();
    let bytes = std::fs::read(path).map_err(|e| workspace::io_message(&e, &shown).message)?;
    let (text, encoding) = workspace::decode_text(&bytes).ok_or_else(|| format!("« {} » n'est pas un fichier texte.", shown))?;
    let mut rope = gpui_kit::component::input::Rope::from(text.as_str());
    for edit in edits {
        let start = rope.position_to_offset(&edit.range.start);
        let end = rope.position_to_offset(&edit.range.end);
        rope.remove(start..end);
        rope.insert(start, &edit.new_text);
    }
    let (bytes, _) = workspace::encode_text(&rope.to_string(), encoding);
    std::fs::write(path, bytes).map_err(|e| workspace::io_message(&e, &shown).message)
}

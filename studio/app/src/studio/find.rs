//! « Rechercher dans les fichiers » (Ctrl+Maj+F): a view of the side bar. The search runs
//! in the background as the text is typed (`nexul_studio::search`); the open editors are
//! searched with their unsaved text. « Tout remplacer » changes the open editors (Ctrl+Z
//! undoes it) and saves the other files, after a confirmation.

use super::*;
use gpui_kit::component::input::{InputState, Position, RopeExt as _};
use gpui_kit::component::Disableable as _;
use nexul_studio::search::{self, Found, Query};
use std::collections::HashMap;
use std::time::Instant;

/// The view shown in the side bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SideView {
    Explorer,
    Search,
}

/// At most this many matches are drawn (the count gives the others).
const SHOWN: usize = 2000;

pub(super) struct Find {
    pub(super) query: Entity<InputState>,
    pub(super) replace: Entity<InputState>,
    case: bool,
    word: bool,
    pub(super) found: Option<Found>,
    /// Stops the search in progress.
    stop: Option<Arc<AtomicBool>>,
    /// Number of the last search started: older results are dropped.
    generation: u64,
    /// The query changed: search after a short pause in the typing.
    due: Option<Instant>,
    _subscriptions: Vec<Subscription>,
}

impl Studio {
    pub(super) fn on_find_in_files(&mut self, _: &FindInFiles, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_none() {
            let query = cx.new(|cx| InputState::new(window, cx).placeholder("Rechercher"));
            let replace = cx.new(|cx| InputState::new(window, cx).placeholder("Remplacer par"));
            let subscriptions = vec![cx.subscribe_in(&query, window, |this: &mut Self, _, event: &InputEvent, _, cx| match event {
                InputEvent::Change => {
                    if let Some(f) = this.find.as_mut() {
                        f.due = Some(Instant::now());
                    }
                }
                InputEvent::PressEnter { .. } => this.run_search(cx),
                _ => {}
            })];
            self.find = Some(Find { query, replace, case: false, word: false, found: None, stop: None, generation: 0, due: None, _subscriptions: subscriptions });
        }
        self.side_view = SideView::Search;
        self.settings.sidebar = true;
        // The word under the cursor, or the selection, is a good start.
        let start = self.active.and_then(|i| self.docs.get(i)).map(|d| {
            let e = d.editor.read(cx);
            let selected = e.selected_value().to_string();
            if selected.is_empty() || selected.contains('\n') {
                e.text().word_at(e.cursor()).to_string()
            } else {
                selected
            }
        });
        let Some(f) = self.find.as_ref() else { return };
        let query = f.query.clone();
        query.update(cx, |i, cx| {
            if let Some(s) = start.filter(|s| !s.trim().is_empty()) {
                i.set_value(s, window, cx);
            }
            i.focus(window, cx);
            i.select_all(window, cx);
        });
        cx.notify();
    }

    /// Shows a view of the side bar, or hides the side bar if it already shows it.
    pub(super) fn show_side_view(&mut self, view: SideView, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.sidebar && self.side_view == view {
            self.settings.sidebar = false;
        } else if view == SideView::Search {
            return self.on_find_in_files(&FindInFiles, window, cx);
        } else {
            self.side_view = view;
            self.settings.sidebar = true;
        }
        self.settings.save();
        cx.notify();
    }

    /// Several times a second: starts the search once the typing pauses.
    pub(super) fn tick_find(&mut self, cx: &mut Context<Self>) {
        if self.find.as_ref().and_then(|f| f.due).is_some_and(|t| t.elapsed() >= Duration::from_millis(250)) {
            self.run_search(cx);
        }
    }

    fn query(&self, cx: &App) -> Option<Query> {
        let f = self.find.as_ref()?;
        Some(Query { text: f.query.read(cx).value().to_string(), case: f.case, word: f.word })
    }

    pub(super) fn run_search(&mut self, cx: &mut Context<Self>) {
        let (Some(root), Some(q)) = (self.root.clone(), self.query(cx)) else { return };
        let open: HashMap<String, String> = self.docs.iter().map(|d| (d.rel.clone(), d.editor.read(cx).value().to_string())).collect();
        let Some(f) = self.find.as_mut() else { return };
        f.due = None;
        if let Some(stop) = f.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        f.generation += 1;
        if q.text.is_empty() {
            f.found = None;
            return cx.notify();
        }
        let (generation, stop) = (f.generation, Arc::new(AtomicBool::new(false)));
        f.stop = Some(stop.clone());
        let task = cx.background_spawn(async move { search::search(&root, &q, &open, &stop) });
        cx.spawn(async move |this, cx| {
            let found = task.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(f) = this.find.as_mut().filter(|f| f.generation == generation) {
                    f.found = Some(found);
                    f.stop = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn toggle_find_option(&mut self, word: bool, cx: &mut Context<Self>) {
        if let Some(f) = self.find.as_mut() {
            if word {
                f.word = !f.word;
            } else {
                f.case = !f.case;
            }
        }
        self.run_search(cx);
    }

    pub(super) fn open_hit(&mut self, rel: &str, line: usize, start: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        self.open_path_at(&root.join(rel), Some(Position { line: line as u32, character: start as u32 }), window, cx);
    }

    // -------------------------------------------------------------- replacing

    fn ask_replace_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = self.find.as_ref() else { return };
        let Some(found) = f.found.as_ref().filter(|found| !found.hits.is_empty()) else { return };
        let with = f.replace.read(cx).value().to_string();
        let (count, files) = (found.hits.len(), found.files);
        let studio = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let studio = studio.clone();
            alert
                .title(format!("Remplacer {} occurrence{} dans {} fichier{} par « {} » ?", count, if count > 1 { "s" } else { "" }, files, if files > 1 { "s" } else { "" }, with))
                .description("Les fichiers ouverts sont modifiés dans l'éditeur (Ctrl+Z annule) ; les autres sont enregistrés sur le disque.")
                .show_cancel(true)
                .ok_text("Tout remplacer")
                .cancel_text("Annuler")
                .on_ok(move |_, window, cx| {
                    let _ = studio.update(cx, |this, cx| this.replace_all_found(window, cx));
                    true
                })
        });
    }

    /// Replaces every match of the query in the files of the last results.
    pub(super) fn replace_all_found(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(root), Some(q)) = (self.root.clone(), self.query(cx)) else { return };
        let Some(f) = self.find.as_ref() else { return };
        let with = f.replace.read(cx).value().to_string();
        let mut rels: Vec<String> = f.found.iter().flat_map(|found| found.hits.iter().map(|h| h.rel.clone())).collect();
        rels.dedup();
        let mut total = 0;
        let mut on_disk = Vec::new();
        for rel in rels {
            match self.docs.iter().find(|d| d.rel == rel) {
                Some(doc) => doc.editor.update(cx, |e, cx| {
                    let (new, n) = search::replace_in_text(&e.value(), &q, &with);
                    if n > 0 {
                        let at = e.cursor_position();
                        e.replace_all(new, window, cx);
                        e.set_cursor_position(at, window, cx);
                        total += n;
                    }
                }),
                None => on_disk.push(rel),
            }
        }
        let (n, failed) = search::replace_in_files(&root, &on_disk, &q, &with);
        total += n;
        for (rel, message) in failed {
            self.notify_error(&format!("« {} » : {}", rel, message), window, cx);
        }
        self.notify_info(&format!("{} remplacement{}.", total, if total > 1 { "s" } else { "" }), window, cx);
        self.run_search(cx);
    }

    // -------------------------------------------------------------- view

    pub(super) fn render_find(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, sidebar, hover, selection) = (theme.muted_foreground, theme.sidebar, theme.list_hover, theme.selection);
        let header = h_flex().h(px(32.)).pl_3().text_xs().text_color(muted).child("RECHERCHE");
        let Some(f) = self.find.as_ref() else { return v_flex().size_full().bg(sidebar).child(header) };
        let option = |id: &'static str, label: &'static str, tip: &'static str, on: bool, word: bool| {
            Button::new(id).ghost().xsmall().label(label).selected(on).tooltip(tip).on_click(cx.listener(move |this, _, _, cx| this.toggle_find_option(word, cx)))
        };
        let can_replace = f.found.as_ref().is_some_and(|found| !found.hits.is_empty());
        let inputs = v_flex()
            .px_2()
            .gap_1()
            .child(h_flex().gap_0p5().child(div().flex_1().child(Input::new(&f.query).small())).child(option("find-case", "Aa", "Respecter la casse", f.case, false)).child(option("find-word", "ab", "Mot entier", f.word, true)))
            .child(
                h_flex().gap_0p5().child(div().flex_1().child(Input::new(&f.replace).small())).child(
                    Button::new("find-replace-all")
                        .ghost()
                        .xsmall()
                        .label("Tout remplacer")
                        .disabled(!can_replace)
                        .on_click(cx.listener(|this, _, window, cx| this.ask_replace_all(window, cx))),
                ),
            );
        let status = match (&f.found, f.stop.is_some()) {
            (_, true) => "Recherche…".to_string(),
            (None, _) if self.root.is_none() => "Ouvrez un dossier pour y chercher.".into(),
            (None, _) => String::new(),
            (Some(found), _) if found.hits.is_empty() => "Aucun résultat.".into(),
            (Some(found), _) => format!(
                "{} résultat{} dans {} fichier{}{}",
                found.hits.len(),
                if found.hits.len() > 1 { "s" } else { "" },
                found.files,
                if found.files > 1 { "s" } else { "" },
                if found.truncated { " (arrêté, affinez la recherche)" } else { "" }
            ),
        };
        let mut list = v_flex();
        if let Some(found) = &f.found {
            let mut last: Option<&str> = None;
            for (i, h) in found.hits.iter().take(SHOWN).enumerate() {
                if last != Some(h.rel.as_str()) {
                    last = Some(h.rel.as_str());
                    let count = found.hits.iter().filter(|o| o.rel == h.rel).count();
                    let dir = files::parent_of(&h.rel);
                    list = list.child(
                        h_flex()
                            .px_2()
                            .pt_1()
                            .gap_1()
                            .text_sm()
                            .child(div().whitespace_nowrap().child(files::name_of(&h.rel).to_string()))
                            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_xs().text_color(muted).child(dir.to_string()))
                            .child(div().text_xs().text_color(muted).child(count.to_string())),
                    );
                }
                let chars: Vec<char> = h.preview.chars().collect();
                let part = |a: usize, b: usize| chars[a.min(chars.len())..b.min(chars.len())].iter().collect::<String>();
                let (rel, line, start) = (h.rel.clone(), h.line, h.start);
                list = list.child(
                    h_flex()
                        .id(("hit", i))
                        .pl_5()
                        .pr_2()
                        .text_xs()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .child(part(0, h.preview_start))
                        .child(div().bg(selection).child(part(h.preview_start, h.preview_end)))
                        .child(part(h.preview_end, chars.len()))
                        .on_click(cx.listener(move |this, _, window, cx| this.open_hit(&rel, line, start, window, cx))),
                );
            }
            if found.hits.len() > SHOWN {
                list = list.child(div().px_2().py_1().text_xs().text_color(muted).child(format!("… et {} autres", found.hits.len() - SHOWN)));
            }
        }
        v_flex()
            .size_full()
            .bg(sidebar)
            .child(header)
            .child(inputs)
            .child(div().px_3().py_1().text_xs().text_color(muted).child(status))
            .child(div().id("find-results").flex_1().min_h_0().overflow_y_scroll().child(list))
    }
}

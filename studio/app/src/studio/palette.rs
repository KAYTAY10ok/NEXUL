//! The palette, over the window: the commands (Ctrl+Shift+P, F1) and the files of the
//! folder (Ctrl+P), found with a fuzzy search. It is drawn inside the view of Studio so
//! that the commands chosen reach its actions.

use super::*;
use gpui_kit::component::command::{Command, CommandItem, CommandState};
use gpui_kit::component::kbd::Kbd;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PaletteMode {
    Commands,
    Files,
}

pub(super) struct Palette {
    pub(super) mode: PaletteMode,
    pub(super) state: Entity<CommandState>,
    /// What had the focus before: it gets it back.
    previous: Option<FocusHandle>,
    /// Files mode: the relative paths of the files of the folder.
    files: Vec<String>,
    /// The entries shown, in order: indexes in `commands()` or in `files`.
    shown: Vec<usize>,
}

/// A command of the palette.
pub(super) struct PaletteCommand {
    pub(super) label: &'static str,
    pub(super) action: Box<dyn Action>,
    /// Other words that find it.
    keywords: &'static str,
    /// The context of its shortcut, if it has one only there.
    context: Option<&'static str>,
}

fn cmd(label: &'static str, action: impl Action, keywords: &'static str, context: Option<&'static str>) -> PaletteCommand {
    PaletteCommand { label, action: Box::new(action), keywords, context }
}

pub(super) fn commands() -> Vec<PaletteCommand> {
    use gpui_kit::component::input::GoToDefinition;
    vec![
        cmd("Fichier : Ouvrir un dossier…", OpenFolder, "projet répertoire", None),
        cmd("Fichier : Nouveau fichier…", NewFile, "créer", None),
        cmd("Fichier : Nouveau dossier…", NewFolder, "créer répertoire", None),
        cmd("Fichier : Enregistrer", Save, "sauvegarder", None),
        cmd("Fichier : Tout enregistrer", SaveAll, "sauvegarder", None),
        cmd("Fichier : Fermer l'éditeur", CloseEditor, "onglet", None),
        cmd("Fichier : Aller au fichier…", QuickOpen, "ouvrir rechercher", None),
        cmd("Affichage : Explorateur", ToggleSidebar, "barre latérale arbre", None),
        cmd("Affichage : Panneau du bas (sortie, problèmes)", TogglePanel, "console", None),
        cmd("Affichage : Problèmes", ShowProblems, "erreurs diagnostics avertissements", None),
        cmd("Affichage : Mode minimaliste", ToggleMinimal, "zen concentration", None),
        cmd("Affichage : Thème clair ou sombre", ToggleTheme, "couleurs apparence", None),
        cmd("Affichage : Onglet suivant", NextTab, "éditeur", None),
        cmd("Code : Mettre en forme le document", FormatDocument, "formater indenter", Some("CodeEditor")),
        cmd("Code : Renommer le symbole…", RenameSymbol, "variable fonction", Some("CodeEditor")),
        cmd("Code : Aller à la définition", GoToDefinition, "déclaration", Some("CodeEditor")),
        cmd("Exécution : Exécuter le programme", Run, "lancer démarrer nxc run", None),
        cmd("Exécution : Arrêter", Stop, "tuer interrompre", None),
        cmd("Explorateur : Renommer…", RenameEntry, "fichier dossier", Some("Explorer")),
        cmd("Explorateur : Supprimer", DeleteEntry, "corbeille effacer", Some("Explorer")),
        cmd("Explorateur : Copier", CopyEntry, "presse-papiers", Some("Explorer")),
        cmd("Explorateur : Couper", CutEntry, "déplacer", Some("Explorer")),
        cmd("Explorateur : Coller", PasteEntry, "presse-papiers", Some("Explorer")),
        cmd("Explorateur : Copier le chemin", CopyPath, "", Some("Explorer")),
        cmd("Explorateur : Copier le chemin relatif", CopyRelativePath, "", Some("Explorer")),
        cmd("Explorateur : Afficher dans l'Explorateur Windows", RevealEntry, "dossier", Some("Explorer")),
        cmd("Explorateur : Actualiser", RefreshExplorer, "recharger", None),
        cmd("Explorateur : Tout replier", CollapseExplorer, "fermer dossiers", None),
        cmd("Quitter NEXUL Studio", Quit, "fermer sortir", None),
    ]
}

/// How well `query` matches `text`: its characters in order, without case; consecutive
/// characters, the start of the text and starts of words count more. `None`: no match.
pub(super) fn fuzzy(query: &str, text: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let mut score = 0;
    let mut ti = 0;
    let mut last: Option<usize> = None;
    for q in query.chars().flat_map(char::to_lowercase).filter(|c| !c.is_whitespace()) {
        let found = (ti..t.len()).find(|&i| t[i] == q)?;
        score += 1;
        if last.is_some_and(|l| l + 1 == found) {
            score += 5;
        }
        if found == 0 || matches!(t[found - 1], '/' | '\\' | '_' | '-' | '.' | ' ' | ':') {
            score += 4;
        }
        last = Some(found);
        ti = found + 1;
    }
    // Shorter texts first, for the same match.
    Some(score * 100 - t.len() as i32)
}

/// The files of the folder (relative paths), without the ignored folders; at most 50 000.
fn all_files(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>, depth: usize) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            if out.len() >= 50_000 {
                return;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if nexul_studio::watch::IGNORED.contains(&name.as_str()) || nexul_studio::watch::is_hidden_name(&name) {
                continue;
            }
            let rel = if prefix.is_empty() { name.clone() } else { format!("{}/{}", prefix, name) };
            match e.file_type() {
                Ok(t) if t.is_dir() && depth < 24 => walk(&e.path(), &rel, out, depth + 1),
                Ok(t) if t.is_file() => out.push(rel),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(root, "", &mut out, 0);
    out.sort_by_key(|r| r.to_lowercase());
    out
}

impl Studio {
    pub(super) fn open_palette(&mut self, mode: PaletteMode, window: &mut Window, cx: &mut Context<Self>) {
        let files = match (mode, &self.root) {
            (PaletteMode::Files, Some(root)) => all_files(root),
            (PaletteMode::Files, None) => return self.notify_error("Ouvrez d'abord un dossier.", window, cx),
            _ => Vec::new(),
        };
        let previous = window.focused(cx);
        let state = cx.new(|cx| CommandState::new(window, cx));
        state.update(cx, |s, cx| s.focus(window, cx));
        self.palette = Some(Palette { mode, state, previous, files, shown: Vec::new() });
        cx.notify();
    }

    pub(super) fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.palette.take() {
            match p.previous {
                Some(f) => f.focus(window, cx),
                None => self.focus.focus(window, cx),
            }
            cx.notify();
        }
    }

    /// The entry `row` (in the order shown) was chosen.
    pub(super) fn palette_confirm(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.palette.as_ref() else { return };
        let Some(&ix) = p.shown.get(row) else { return };
        match p.mode {
            PaletteMode::Commands => {
                let action = commands().swap_remove(ix).action;
                self.close_palette(window, cx);
                // After the focus is back where the command acts (an editor, the tree…).
                window.defer(cx, move |window, cx| window.dispatch_action(action, cx));
            }
            PaletteMode::Files => {
                let rel = p.files[ix].clone();
                self.close_palette(window, cx);
                self.open_file(&rel, window, cx);
                self.select(&rel, cx);
            }
        }
    }

    pub(super) fn render_palette(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let studio = cx.entity().downgrade();
        let p = self.palette.as_mut()?;
        let query = p.state.read(cx).query(cx).to_string();
        let mut items = Vec::new();
        match p.mode {
            PaletteMode::Commands => {
                let all = commands();
                let mut scored: Vec<(i32, usize)> = all
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| {
                        let s = fuzzy(&query, c.label).max(fuzzy(&query, c.keywords).map(|s| s - 50));
                        s.map(|s| (s, i))
                    })
                    .collect();
                if !query.is_empty() {
                    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
                }
                p.shown = scored.into_iter().map(|(_, i)| i).collect();
                for &i in &p.shown {
                    let c = &all[i];
                    let action = c.action.boxed_clone();
                    let (label, context) = (c.label, c.context);
                    items.push(CommandItem::new().label(label).child(move |window, cx| {
                        let kbd = Kbd::binding_for_action(action.as_ref(), context, window).or_else(|| Kbd::binding_for_action(action.as_ref(), None, window));
                        h_flex().w_full().gap_2().child(div().flex_1().text_sm().child(label)).children(kbd).text_color(cx.theme().foreground)
                    }));
                }
            }
            PaletteMode::Files => {
                let mut scored: Vec<(i32, usize)> = p
                    .files
                    .iter()
                    .enumerate()
                    .filter_map(|(i, rel)| {
                        // The name counts more than the folders.
                        let name = files::name_of(rel);
                        let s = fuzzy(&query, name).map(|s| s + 1000).max(fuzzy(&query, rel));
                        s.map(|s| (s, i))
                    })
                    .collect();
                if !query.is_empty() {
                    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
                }
                scored.truncate(200);
                p.shown = scored.into_iter().map(|(_, i)| i).collect();
                for &i in &p.shown {
                    let rel = p.files[i].clone();
                    let name = files::name_of(&rel).to_string();
                    let dir = files::parent_of(&rel).to_string();
                    items.push(CommandItem::new().label(rel.clone()).child(move |_, cx| {
                        h_flex()
                            .w_full()
                            .gap_2()
                            .text_sm()
                            .child(gpui_kit::component::Icon::new(IconName::File).xsmall())
                            .child(name.clone())
                            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(dir.clone()))
                    }));
                }
            }
        }
        let placeholder = match p.mode {
            PaletteMode::Commands => "Tapez une commande…",
            PaletteMode::Files => "Nom du fichier à ouvrir…",
        };
        let (s1, s2, s3, s4) = (studio.clone(), studio.clone(), studio.clone(), studio);
        let command = Command::new(&p.state)
            .items(items)
            .filterable(false)
            .placeholder(placeholder)
            .max_h(px(420.))
            .empty(|_, _, _| div().p_3().text_sm().child("Aucun résultat."))
            .on_query(move |_, _, cx| {
                let _ = s1.update(cx, |_, cx| cx.notify());
            })
            .on_confirm(move |ix, window, cx| {
                let _ = s2.update(cx, |this, cx| this.palette_confirm(ix.row, window, cx));
            })
            .on_cancel(move |window, cx| {
                let _ = s3.update(cx, |this, cx| this.close_palette(window, cx));
            });
        Some(
            div()
                .absolute()
                .top(px(44.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    div()
                        .id("palette")
                        .w(px(620.))
                        .max_w(relative(0.9))
                        .rounded_lg()
                        .shadow_lg()
                        .bg(cx.theme().popover)
                        .occlude()
                        .on_mouse_down_out(move |_, window, cx| {
                            let _ = s4.update(cx, |this, cx| this.close_palette(window, cx));
                        })
                        .child(command),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use core::prelude::v1::test;

    #[test]
    fn fuzzy_search() {
        assert!(fuzzy("", "x").is_some());
        assert!(fuzzy("nf", "Fichier : Nouveau fichier…").is_some());
        assert!(fuzzy("zz", "Fichier").is_none());
        // Consecutive characters and starts of words rank higher.
        assert!(fuzzy("main", "src/main.nx") > fuzzy("main", "src/my_animal.nx"));
        assert!(fuzzy("calc", "calcul.nx") > fuzzy("calc", "src/notes/calcul.nx"));
        assert!(fuzzy("ÉTÉ", "été.txt").is_some());
    }

    #[test]
    fn every_command_has_a_label() {
        let all = commands();
        assert!(all.len() > 20);
        assert!(all.iter().all(|c| !c.label.is_empty()));
    }
}

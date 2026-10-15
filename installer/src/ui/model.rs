//! What the wizard shows and what the user chose, without any window: the pages and their
//! order, the components, the earlier versions, the location and the options, and the
//! texts that depend on them. The window (`window.rs`) displays this model and forwards the
//! actions of the user to it. Same rules as the HTML interface it replaces.

use crate::old::Item;
use crate::payload::Component;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Page {
    Welcome,
    OldVersions,
    Components,
    Location,
    Progress,
    Done,
    /// The page of the uninstaller.
    Uninstall,
}

/// An earlier version or an item that points at one, and the choice made for it.
#[derive(Clone, Debug)]
pub struct OldChoice {
    pub item: Item,
    /// `remove`, `backup` or `keep`.
    pub choices: Vec<&'static str>,
    pub choice: Option<&'static str>,
}

/// The result of the wizard: what `install()` needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Choices {
    pub dir: String,
    pub components: Vec<String>,
    pub path: bool,
    pub desktop: bool,
    pub start_menu: bool,
    pub associate: bool,
    /// `(index of the item in the scan, action)`.
    pub old: Vec<(usize, String)>,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub uninstall: bool,
    pub version: String,
    pub page: Page,
    pub accepted: bool,
    pub components: Vec<(Component, bool)>,
    /// `None` while the earlier versions are looked for.
    pub old: Option<Vec<OldChoice>>,
    pub dir: String,
    pub path: bool,
    pub desktop: bool,
    pub start_menu: bool,
    pub associate: bool,
}

/// `12,5 Mo`, `1,2 Go` (at least `0,1 Mo`), as the HTML interface wrote sizes.
pub fn format_size(bytes: u64) -> String {
    let (value, unit) = if bytes >= 1 << 30 {
        (bytes as f64 / (1u64 << 30) as f64, "Go")
    } else {
        ((bytes as f64 / (1u64 << 20) as f64).max(0.1), "Mo")
    };
    format!("{value:.1} {unit}").replace('.', ",")
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n > 1 { many } else { one })
}

impl Model {
    pub fn new(uninstall: bool, version: &str, components: &[Component], dir: &str) -> Model {
        Model {
            uninstall,
            version: version.to_string(),
            page: if uninstall { Page::Uninstall } else { Page::Welcome },
            accepted: false,
            components: components.iter().map(|c| (c.clone(), c.required || c.default)).collect(),
            old: None,
            dir: dir.to_string(),
            path: true,
            desktop: true,
            start_menu: true,
            associate: true,
        }
    }

    /// The earlier versions have been looked for.
    pub fn set_old(&mut self, items: &[Item]) {
        self.old = Some(
            items
                .iter()
                .map(|item| OldChoice {
                    item: item.clone(),
                    choices: if item.kind.is_version() { vec!["remove", "backup"] } else { vec!["remove", "keep"] },
                    choice: None,
                })
                .collect(),
        );
    }

    fn has_old(&self) -> bool {
        self.old.as_ref().is_some_and(|o| !o.is_empty())
    }

    /// The pages, in order (shown in the side bar).
    pub fn pages(&self) -> Vec<Page> {
        if self.uninstall {
            return vec![Page::Uninstall, Page::Progress, Page::Done];
        }
        let mut p = vec![Page::Welcome];
        if self.has_old() {
            p.push(Page::OldVersions);
        }
        p.extend([Page::Components, Page::Location, Page::Progress, Page::Done]);
        p
    }

    /// The name of a page in the side bar.
    pub fn step_name(&self, page: Page) -> &'static str {
        match (self.uninstall, page) {
            (true, Page::Uninstall) => "Désinstallation",
            (true, Page::Progress) => "Suppression",
            (_, Page::Welcome) => "Bienvenue",
            (_, Page::OldVersions) => "Anciennes versions",
            (_, Page::Components) => "Composants",
            (_, Page::Location) => "Emplacement",
            (_, Page::Progress) => "Installation",
            (_, Page::Done) => "Terminé",
            (_, Page::Uninstall) => "Désinstallation",
        }
    }

    pub fn can_go_back(&self) -> bool {
        matches!(self.page, Page::OldVersions | Page::Components | Page::Location)
    }

    /// The label of the main button of the page.
    pub fn next_label(&self) -> &'static str {
        match self.page {
            Page::Location => "Installer",
            Page::Done => "Fermer",
            Page::Uninstall => "Désinstaller",
            _ => "Suivant",
        }
    }

    /// The main button can be used.
    pub fn can_go_next(&self) -> bool {
        match self.page {
            Page::Welcome => self.accepted && self.old.is_some(),
            Page::OldVersions => self.choices_left() == 0,
            Page::Progress => false,
            _ => true,
        }
    }

    /// The previous page.
    pub fn back(&mut self) {
        if !self.can_go_back() {
            return;
        }
        let pages = self.pages();
        if let Some(i) = pages.iter().position(|p| *p == self.page) {
            if i > 0 {
                self.page = pages[i - 1];
            }
        }
    }

    /// The next page, for the pages without action (welcome, earlier versions, components).
    /// `Location` and `Uninstall` start the work: see [`Model::choices`].
    pub fn next(&mut self) {
        if !self.can_go_next() {
            return;
        }
        self.page = match self.page {
            Page::Welcome if self.has_old() => Page::OldVersions,
            Page::Welcome | Page::OldVersions => Page::Components,
            Page::Components => Page::Location,
            other => other,
        };
    }

    // ---- Earlier versions.

    pub fn choices_left(&self) -> usize {
        self.old.as_ref().map_or(0, |o| o.iter().filter(|c| c.choice.is_none()).count())
    }

    pub fn choose(&mut self, index: usize, action: &str) {
        if let Some(c) = self.old.as_mut().and_then(|o| o.get_mut(index)) {
            if let Some(a) = c.choices.iter().find(|a| **a == action) {
                c.choice = Some(a);
            }
        }
    }

    /// « Tout supprimer » / « Tout sauvegarder »: what cannot be backed up is removed.
    pub fn choose_all(&mut self, action: &str) {
        for c in self.old.iter_mut().flatten() {
            c.choice = Some(c.choices.iter().find(|a| **a == action).copied().unwrap_or("remove"));
        }
    }

    /// `2 choix restants` / `Tous les choix sont faits.`
    pub fn choices_left_text(&self) -> String {
        match self.choices_left() {
            0 => "Tous les choix sont faits.".into(),
            n => format!("{} restant{}", plural(n, "choix", "choix"), if n > 1 { "s" } else { "" }),
        }
    }

    /// The note of the welcome page about what was found (empty if nothing).
    pub fn found_text(&self) -> String {
        let Some(old) = &self.old else { return String::new() };
        if old.is_empty() {
            return String::new();
        }
        let versions = old.iter().filter(|o| o.item.kind.is_version()).count();
        let others = old.len() - versions;
        let mut what = Vec::new();
        if versions > 0 {
            what.push(plural(versions, "version précédente", "versions précédentes"));
        }
        if others > 0 {
            what.push(plural(others, "élément résiduel", "éléments résiduels"));
        }
        // « trouvée(s) » with versions only, « trouvé(s) » as soon as there is an element.
        format!(
            "{} de NEXUL trouvé{}{} sur cet ordinateur : à l'étape suivante, vous choisirez de {}.",
            what.join(" et "),
            if others == 0 { "e" } else { "" },
            if old.len() > 1 { "s" } else { "" },
            if versions > 0 { "supprimer ou de sauvegarder chaque version" } else { "les retirer ou de les conserver" }
        )
    }

    // ---- Components.

    pub fn is_chosen(&self, id: &str) -> bool {
        self.components.iter().any(|(c, on)| c.id == id && *on)
    }

    fn set(&mut self, id: &str, on: bool) {
        for (c, chosen) in &mut self.components {
            if c.id == id && !c.required {
                *chosen = on;
            }
        }
    }

    /// Checks or unchecks a component. The debugger works inside NEXUL Studio: without
    /// Studio, no debugger; with the debugger, Studio.
    pub fn toggle(&mut self, id: &str) {
        let Some((c, on)) = self.components.iter().find(|(c, _)| c.id == id) else { return };
        if c.required {
            return;
        }
        let on = !*on;
        self.set(id, on);
        if id == "studio" && !on {
            self.set("debugger", false);
        }
        if id == "debugger" && on {
            self.set("studio", true);
        }
        if !self.is_chosen("studio") {
            self.desktop = false;
            self.associate = false;
        }
    }

    pub fn total_size(&self) -> u64 {
        self.components.iter().filter(|(_, on)| *on).map(|(c, _)| c.size).sum()
    }

    /// `Espace nécessaire : 12,5 Mo`.
    pub fn space_text(&self) -> String {
        format!("Espace nécessaire : {}", format_size(self.total_size()))
    }

    // ---- Location and options.

    /// The options of the shortcut on the Desktop and of the association need Studio.
    pub fn studio_options_enabled(&self) -> bool {
        self.is_chosen("studio")
    }

    /// `Seront installés : A, B — 12,5 Mo.`
    pub fn summary_text(&self) -> String {
        let names: Vec<&str> = self.components.iter().filter(|(_, on)| *on).map(|(c, _)| c.name.as_str()).collect();
        format!("Seront installés : {} — {}.", names.join(", "), format_size(self.total_size()))
    }

    /// What `install()` needs, or the message to show if the folder is not valid.
    pub fn choices(&self) -> Result<Choices, String> {
        let dir = self.dir.trim().to_string();
        let b = dir.as_bytes();
        if !(b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\') {
            return Err("Indiquez un dossier complet, par exemple C:\\NEXUL.".into());
        }
        let studio = self.studio_options_enabled();
        Ok(Choices {
            dir,
            components: self.components.iter().filter(|(_, on)| *on).map(|(c, _)| c.id.clone()).collect(),
            path: self.path,
            desktop: self.desktop && studio,
            start_menu: self.start_menu,
            associate: self.associate && studio,
            old: self
                .old
                .iter()
                .flatten()
                .enumerate()
                .filter_map(|(i, c)| c.choice.map(|a| (i, a.to_string())))
                .collect(),
        })
    }
}

/// The text shown for an earlier version: `(title, location, details)`.
pub fn old_texts(c: &OldChoice) -> (String, String, String) {
    let it = &c.item;
    let title = if it.version.is_empty() { it.name.clone() } else { format!("{} {}", it.name, it.version) };
    let mut details = Vec::new();
    if it.size > 0 {
        details.push(format_size(it.size));
    }
    details.extend(it.details.iter().cloned());
    if it.admin {
        details.push("droits d'administrateur demandés".into());
    }
    (title, it.location.display().to_string(), details.join(" · "))
}

/// The label of an action on an earlier version.
pub fn action_name(action: &str) -> &'static str {
    match action {
        "remove" => "Supprimer",
        "backup" => "Sauvegarder",
        _ => "Conserver",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::old::Kind;
    use std::path::PathBuf;

    fn comp(id: &str, required: bool, default: bool, size: u64) -> Component {
        Component { id: id.into(), required, default, size, name: id.to_uppercase(), description: String::new() }
    }

    fn components() -> Vec<Component> {
        vec![
            comp("core", true, true, 10 << 20),
            comp("llvm", true, true, 100 << 20),
            comp("studio", false, true, 20 << 20),
            comp("debugger", false, true, 30 << 20),
            comp("samples", false, false, 1 << 20),
        ]
    }

    fn item(kind: Kind, name: &str) -> Item {
        Item {
            kind,
            name: name.into(),
            version: if kind.is_version() { "0.1.0".into() } else { String::new() },
            location: PathBuf::from(r"C:\Program Files\NEXUL Studio"),
            size: if kind.is_version() { 5 << 20 } else { 0 },
            admin: kind == Kind::Folder,
            details: vec!["dans le PATH".into()],
            scope: String::new(),
        }
    }

    #[test]
    fn sizes() {
        assert_eq!(format_size(0), "0,1 Mo");
        assert_eq!(format_size(12 << 20), "12,0 Mo");
        assert_eq!(format_size((3 << 20) / 2), "1,5 Mo");
        assert_eq!(format_size(3 << 30), "3,0 Go");
    }

    #[test]
    fn pages_without_earlier_versions() {
        let mut m = Model::new(false, "0.3.0", &components(), r"C:\NEXUL");
        assert_eq!(m.page, Page::Welcome);
        // Not before the licenses are accepted and the search is over.
        assert!(!m.can_go_next());
        m.accepted = true;
        assert!(!m.can_go_next());
        m.set_old(&[]);
        assert!(m.can_go_next());
        assert_eq!(m.pages(), [Page::Welcome, Page::Components, Page::Location, Page::Progress, Page::Done]);
        assert!(m.found_text().is_empty());
        m.next();
        assert_eq!(m.page, Page::Components);
        assert!(m.can_go_back());
        m.next();
        assert_eq!((m.page, m.next_label()), (Page::Location, "Installer"));
        m.back();
        m.back();
        assert_eq!(m.page, Page::Welcome);
        m.back();
        assert_eq!(m.page, Page::Welcome);
    }

    #[test]
    fn earlier_versions() {
        let mut m = Model::new(false, "0.3.0", &components(), r"C:\NEXUL");
        m.accepted = true;
        m.set_old(&[item(Kind::Install, "NEXUL"), item(Kind::Folder, "NEXUL Studio"), item(Kind::Shortcut, "Raccourci")]);
        assert_eq!(
            m.found_text(),
            "2 versions précédentes et 1 élément résiduel de NEXUL trouvés sur cet ordinateur : à l'étape suivante, \
             vous choisirez de supprimer ou de sauvegarder chaque version."
        );
        m.next();
        assert_eq!(m.page, Page::OldVersions);
        assert_eq!(m.pages()[1], Page::OldVersions);
        // No choice by default: each must be made.
        assert_eq!((m.can_go_next(), m.choices_left_text()), (false, "3 choix restants".to_string()));
        m.choose(0, "backup");
        m.choose(2, "backup"); // not offered for a shortcut: ignored
        assert_eq!(m.choices_left(), 2);
        m.choose_all("backup");
        let c: Vec<_> = m.old.as_ref().unwrap().iter().map(|c| c.choice).collect();
        assert_eq!(c, [Some("backup"), Some("backup"), Some("remove")]);
        assert_eq!(m.choices_left_text(), "Tous les choix sont faits.");
        m.choose(1, "remove");
        m.next();
        assert_eq!(m.page, Page::Components);
        let ch = m.choices().unwrap();
        assert_eq!(ch.old, [(0, "backup".to_string()), (1, "remove".to_string()), (2, "remove".to_string())]);
        // The texts of a version.
        let (title, location, details) = old_texts(&m.old.as_ref().unwrap()[1]);
        assert_eq!(title, "NEXUL Studio 0.1.0");
        assert_eq!(location, r"C:\Program Files\NEXUL Studio");
        assert_eq!(details, "5,0 Mo · dans le PATH · droits d'administrateur demandés");
        assert_eq!(action_name("keep"), "Conserver");
    }

    #[test]
    fn only_leftovers() {
        let mut m = Model::new(false, "0.3.0", &components(), r"C:\NEXUL");
        m.set_old(&[item(Kind::PathEntry, "Entrée du PATH")]);
        assert_eq!(
            m.found_text(),
            "1 élément résiduel de NEXUL trouvé sur cet ordinateur : à l'étape suivante, vous choisirez de les \
             retirer ou de les conserver."
        );
        assert_eq!(m.old.as_ref().unwrap()[0].choices, ["remove", "keep"]);
        m.set_old(&[item(Kind::Install, "NEXUL")]);
        assert!(m.found_text().starts_with("1 version précédente de NEXUL trouvée sur cet ordinateur"));
        m.set_old(&[item(Kind::Install, "NEXUL"), item(Kind::Folder, "NEXUL Studio")]);
        assert!(m.found_text().starts_with("2 versions précédentes de NEXUL trouvées sur cet ordinateur"));
    }

    #[test]
    fn components_and_options() {
        let mut m = Model::new(false, "0.3.0", &components(), r"C:\NEXUL");
        assert_eq!(m.total_size(), 160 << 20);
        assert!(!m.is_chosen("samples"));
        m.toggle("core"); // required: unchanged
        assert!(m.is_chosen("core"));
        m.toggle("samples");
        assert_eq!(m.space_text(), "Espace nécessaire : 161,0 Mo");
        // Without Studio, no debugger, no shortcut on the Desktop, no association.
        m.toggle("studio");
        assert!(!m.is_chosen("debugger"));
        assert!(!m.studio_options_enabled() && !m.desktop && !m.associate);
        // The debugger brings Studio back.
        m.toggle("debugger");
        assert!(m.is_chosen("studio") && m.is_chosen("debugger"));
        assert_eq!(m.summary_text(), "Seront installés : CORE, LLVM, STUDIO, DEBUGGER, SAMPLES — 161,0 Mo.");
        m.desktop = true;
        let c = m.choices().unwrap();
        assert_eq!(c.components, ["core", "llvm", "studio", "debugger", "samples"]);
        assert_eq!((c.path, c.desktop, c.start_menu, c.associate), (true, true, true, false));
        assert!(c.old.is_empty());
        // The folder: complete path with a drive.
        for bad in ["", "NEXUL", r"\NEXUL", "C:NEXUL", r"\\serveur\partage"] {
            m.dir = bad.into();
            assert!(m.choices().is_err(), "{bad:?}");
        }
        m.dir = r"  D:\Outils\NEXUL ".into();
        assert_eq!(m.choices().unwrap().dir, r"D:\Outils\NEXUL");
    }

    #[test]
    fn uninstall_pages() {
        let m = Model::new(true, "0.3.0", &[], "");
        assert_eq!(m.page, Page::Uninstall);
        assert_eq!(m.pages(), [Page::Uninstall, Page::Progress, Page::Done]);
        assert_eq!(m.step_name(Page::Progress), "Suppression");
        assert_eq!(m.next_label(), "Désinstaller");
        assert!(!m.can_go_back());
    }
}

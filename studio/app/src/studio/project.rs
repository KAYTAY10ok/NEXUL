//! « Nouveau projet » : the templates of `studio/templates` (Codex Luna), offered according
//! to the tools found on this computer (detection of `studio/lsp`, `nxc targets`). A
//! template whose tools are missing is shown with what it needs and is not created.

use super::*;
use gpui_kit::component::input::InputState;
use nexul_studio_templates::{all_templates, ProjectTemplate, Tool, ToolchainRegistry};

/// The id of each tool in the detection of `studio/lsp`.
const TOOLS: &[(Tool, &str)] = &[
    (Tool::Nxc, "nxc"),
    (Tool::Nxpm, "nxpm"),
    (Tool::Python, "python"),
    (Tool::Node, "node"),
    (Tool::Gcc, "gcc"),
    (Tool::Gxx, "g++"),
    (Tool::Clang, "clang"),
    (Tool::Clangxx, "clang++"),
    (Tool::Cl, "cl"),
    (Tool::Cargo, "cargo"),
    (Tool::Rustc, "rustc"),
    (Tool::Dotnet, "dotnet"),
];

/// Finds the tools of the templates (slow: runs each candidate; in the background). The
/// `nxc` of Studio is preferred to the one found elsewhere.
pub(super) fn detect_tools(nxc: Option<PathBuf>) -> ToolchainRegistry {
    let found = nexul_studio_lsp::registry::Detector::from_env().detect();
    let mut tools = ToolchainRegistry::default();
    for &(tool, id) in TOOLS {
        if let Some(sdk) = found.sdk(id) {
            tools.insert(tool, sdk.path.clone());
        }
    }
    if let Some(nxc) = nxc {
        tools.insert(Tool::Nxc, nxc);
    }
    if let Some(nxc) = tools.get(Tool::Nxc).map(Path::to_path_buf) {
        // `nxc targets`: one target per line, its name first.
        if let Ok(out) = runner::command(&nxc, &["targets".to_string()], &std::env::temp_dir()).output() {
            let web = String::from_utf8_lossy(&out.stdout).lines().any(|l| l.split_whitespace().next() == Some("wasm32-web"));
            tools.set_target_support(Tool::Nxc, "wasm32-web", out.status.success() && web);
        }
    }
    tools
}

/// The file shown once the project is created: its main NEXUL file, else its first file.
fn first_file(template: &ProjectTemplate) -> Option<String> {
    let files: Vec<String> = template.files().iter().map(|f| f.path.to_string_lossy().replace('\\', "/")).collect();
    files.iter().find(|p| p.ends_with("main.nx")).or_else(|| files.iter().find(|p| p.ends_with(".nx"))).or_else(|| files.first()).cloned()
}

/// The content of the « Nouveau projet » dialog.
pub(super) struct ProjectForm {
    templates: Vec<ProjectTemplate>,
    /// The missing tools of each template; `None` while they are looked for.
    missing: Option<Vec<Vec<String>>>,
    selected: usize,
    pub(super) name: Entity<InputState>,
    parent: PathBuf,
    error: Option<String>,
}

impl ProjectForm {
    pub(super) fn new(parent: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).default_value("mon-projet"));
        ProjectForm { templates: all_templates(), missing: None, selected: 0, name, parent, error: None }
    }

    pub(super) fn set_tools(&mut self, tools: &ToolchainRegistry, cx: &mut Context<Self>) {
        let missing: Vec<Vec<String>> = self.templates.iter().map(|t| t.missing_tools(tools)).collect();
        // The first template that can be created, if the one chosen cannot.
        if !missing[self.selected].is_empty() {
            self.selected = missing.iter().position(Vec::is_empty).unwrap_or(self.selected);
        }
        self.missing = Some(missing);
        cx.notify();
    }

    pub(super) fn select(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(i) = self.templates.iter().position(|t| t.id.as_str() == id) {
            self.selected = i;
            self.error = None;
            cx.notify();
        }
    }

    /// The template and the folder of the project, or why it cannot be created.
    fn target(&self, cx: &App) -> Result<(ProjectTemplate, PathBuf), String> {
        let name = self.name.read(cx).value().trim().to_string();
        workspace::validate_name(&name).map_err(|e| e.message)?;
        let template = self.templates.get(self.selected).ok_or("Choisissez un modèle.")?;
        match &self.missing {
            None => return Err("Recherche des outils installés en cours…".into()),
            Some(m) if !m[self.selected].is_empty() => return Err(format!("Ce modèle demande des outils absents : {}.", m[self.selected].join(", "))),
            _ => {}
        }
        let dest = self.parent.join(&name);
        let used = std::fs::read_dir(&dest).map(|mut d| d.next().is_some()).unwrap_or(dest.exists());
        if used {
            return Err(format!("Le dossier « {} » existe déjà et n'est pas vide.", dest.display()));
        }
        Ok((template.clone(), dest))
    }

    fn browse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Choisir l'emplacement".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = paths.await {
                if let Some(p) = paths.pop() {
                    let _ = this.update(cx, |this, cx| {
                        this.parent = p;
                        this.error = None;
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }
}

impl Render for ProjectForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, warning, danger, accent) = (theme.muted_foreground, theme.warning, theme.danger, theme.accent);
        let mut list = v_flex().gap_1();
        for (i, t) in self.templates.iter().enumerate() {
            let note = match &self.missing {
                None => Some("Recherche des outils…".to_string()),
                Some(m) if m[i].is_empty() => None,
                Some(m) => Some(format!("Outils absents : {}", m[i].join(", "))),
            };
            let usable = self.missing.as_ref().is_some_and(|m| m[i].is_empty());
            list = list.child(
                div()
                    .id(("template", i))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .when(i == self.selected, |d| d.bg(accent))
                    .when(!usable, |d| d.opacity(0.6))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(t.name))
                    .child(div().text_xs().text_color(muted).child(t.description))
                    .children(note.map(|n| div().text_xs().text_color(warning).child(n)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = i;
                        this.error = None;
                        cx.notify();
                    })),
            );
        }
        v_flex()
            .gap_3()
            .child(div().id("templates").max_h(px(300.)).overflow_y_scroll().child(list))
            .child(v_flex().gap_1().child(div().text_sm().child("Nom du projet")).child(Input::new(&self.name)))
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().text_xs().text_color(muted).child(format!("Créé dans {}", self.parent.join(self.name.read(cx).value().trim()).display())))
                    .child(Button::new("project-browse").small().label("Emplacement…").on_click(cx.listener(|this, _, window, cx| this.browse(window, cx)))),
            )
            .children(self.error.clone().map(|e| div().text_sm().text_color(danger).child(e)))
    }
}

/// Creates the project of the form: its folder and the file to show; or shows why it
/// cannot be created and gives `None`.
fn make_project(form: &Entity<ProjectForm>, cx: &mut App) -> Option<(PathBuf, Option<String>)> {
    let made = form.read(cx).target(cx).and_then(|(template, dest)| template.create(&dest).map(|_| (dest, first_file(&template))).map_err(|e| format!("Création impossible : {}", e)));
    match made {
        Ok(m) => Some(m),
        Err(e) => {
            form.update(cx, |f, cx| {
                f.error = Some(e);
                cx.notify();
            });
            None
        }
    }
}

/// Creates the project of the form, closes the dialog and opens the project; or leaves
/// the dialog open with the reason.
pub(super) fn create_project(form: &Entity<ProjectForm>, studio: &WeakEntity<Studio>, window: &mut Window, cx: &mut App) {
    if let Some((dest, file)) = make_project(form, cx) {
        window.close_dialog(cx);
        let _ = studio.update(cx, |this, cx| this.open_new_project(dest, file, window, cx));
    }
}
impl Studio {
    /// Opens a project just created, and shows its main file.
    fn open_new_project(&mut self, dest: PathBuf, file: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_open = file.map(|rel| (dest.clone(), rel));
        self.request_open_root(dest, window, cx);
    }

    pub(super) fn on_new_project(&mut self, _: &NewProject, window: &mut Window, cx: &mut Context<Self>) {
        // Next to the open folder, else in Documents.
        let parent = self
            .root
            .as_ref()
            .and_then(|r| r.parent().map(Path::to_path_buf))
            .or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Documents")))
            .unwrap_or_else(std::env::temp_dir);
        let form = cx.new(|cx| ProjectForm::new(parent, window, cx));
        // The tools are looked for once, in the background.
        match self.tools.clone() {
            Some(tools) => form.update(cx, |f, cx| f.set_tools(&tools, cx)),
            None => {
                let nxc = self.nxc.clone();
                let task = cx.background_spawn(async move { detect_tools(nxc) });
                let weak = form.downgrade();
                cx.spawn_in(window, async move |this, cx| {
                    let tools = Rc::new(task.await);
                    let _ = weak.update(cx, |f, cx| f.set_tools(&tools, cx));
                    let _ = this.update(cx, |this, _| this.tools = Some(tools));
                })
                .detach();
            }
        }
        let studio = cx.entity().downgrade();
        // Enter in the name box creates the project.
        let name = form.read(cx).name.clone();
        let f = form.clone();
        self.dialog_input = Some(cx.subscribe_in(&name, window, move |this: &mut Self, _, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                if let Some((dest, file)) = make_project(&f, cx) {
                    window.close_dialog(cx);
                    this.open_new_project(dest, file, window, cx);
                }
            }
        }));
        let first = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first.replace(false) {
                name.update(cx, |i, cx| {
                    i.focus(window, cx);
                    i.select_all(window, cx);
                });
            }
            let (f1, s1, f2, s2) = (form.clone(), studio.clone(), form.clone(), studio.clone());
            dialog
                .title("Nouveau projet")
                .w(px(560.))
                .child(form.clone())
                .on_ok(move |_, window, cx| {
                    create_project(&f1, &s1, window, cx);
                    false
                })
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(Button::new("project-cancel").label("Annuler").on_click(|_, window, cx| window.close_dialog(cx)))
                        .child(Button::new("project-create").primary().label("Créer").on_click(move |_, window, cx| create_project(&f2, &s2, window, cx))),
                )
        });
    }
}

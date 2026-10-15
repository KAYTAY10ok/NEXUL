//! Functional starter projects for NEXUL Studio.
//!
//! This crate deliberately does not inspect `PATH`. Its caller supplies installed
//! executable paths, so Studio can adapt the SDK catalog from `studio/lsp` without
//! duplicating SDK discovery here.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tool {
    Nxc,
    Nxpm,
    Python,
    Node,
    Gcc,
    Clang,
    Cl,
    Gxx,
    Clangxx,
    Cargo,
    Rustc,
    Dotnet,
}

/// A small adapter boundary for Studio's SDK/LSP catalog. Implement this using the
/// `studio/lsp` discovery result; this crate performs no SDK detection itself.
pub trait ToolchainProvider {
    fn executable(&self, tool: Tool) -> Option<PathBuf>;
}

#[derive(Clone, Debug, Default)]
pub struct ToolchainRegistry {
    executables: BTreeMap<Tool, PathBuf>,
}

impl ToolchainRegistry {
    pub fn insert(&mut self, tool: Tool, executable: impl Into<PathBuf>) -> Option<PathBuf> {
        self.executables.insert(tool, executable.into())
    }
    pub fn get(&self, tool: Tool) -> Option<&Path> {
        self.executables.get(&tool).map(PathBuf::as_path)
    }
    pub fn contains(&self, tool: Tool) -> bool {
        self.executables.contains_key(&tool)
    }
}

impl ToolchainProvider for ToolchainRegistry {
    fn executable(&self, tool: Tool) -> Option<PathBuf> {
        self.executables.get(&tool).cloned()
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum TemplateId {
    NexulConsole,
    NexulLibrary,
    NxpmDependency,
    WebSite,
    Python,
    C,
    Cpp,
    Rust,
    CSharp,
}

impl TemplateId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NexulConsole => "nexul-console",
            Self::NexulLibrary => "nexul-library",
            Self::NxpmDependency => "nxpm-dependency",
            Self::WebSite => "web-site",
            Self::Python => "python",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Rust => "rust",
            Self::CSharp => "csharp",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectFile {
    pub path: PathBuf,
    pub contents: String,
}

#[derive(Clone, Debug)]
enum Requirement {
    One(Tool),
    Any(Vec<Tool>),
    All(Vec<Tool>),
}

#[derive(Clone, Debug)]
pub struct ProjectTemplate {
    pub id: TemplateId,
    pub name: &'static str,
    pub description: &'static str,
    files: Vec<ProjectFile>,
    requirement: Requirement,
    build: Vec<BuildRecipe>,
    run: Vec<RunRecipe>,
}

#[derive(Clone, Debug)]
enum BuildRecipe {
    Tool { tool: Tool, args: Vec<&'static str> },
    NexulConsole,
    CProgram { cpp: bool },
    RustCargo,
    Dotnet,
    WebNode,
    WebPython,
}

#[derive(Clone, Debug)]
enum RunRecipe {
    Tool {
        tool: Tool,
        args: Vec<&'static str>,
    },
    ProjectFile {
        path: &'static str,
        args: Vec<&'static str>,
    },
    CProgram {
        cpp: bool,
    },
    Cargo,
    Dotnet,
    WebNode,
    WebPython,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub label: String,
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub working_directory: PathBuf,
    pub env: Vec<(OsString, OsString)>,
}

impl CommandSpec {
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.working_directory)
            .envs(self.env.iter().cloned());
        command
    }

    pub fn run(&self) -> io::Result<ExitStatus> {
        self.command().status()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandPlan {
    pub label: String,
    pub commands: Vec<CommandSpec>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TemplateActions {
    pub build: Option<CommandPlan>,
    pub run: Vec<CommandPlan>,
}

#[derive(Debug)]
pub enum TemplateError {
    Io(io::Error),
    UnknownTemplate(String),
    MissingTools(Vec<String>),
    UnsafePath(PathBuf),
    FileExists(PathBuf),
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "template I/O error: {error}"),
            Self::UnknownTemplate(id) => write!(f, "unknown project template `{id}`"),
            Self::MissingTools(tools) => {
                write!(f, "required tools were not found: {}", tools.join(", "))
            }
            Self::UnsafePath(path) => {
                write!(f, "template contains an unsafe path: {}", path.display())
            }
            Self::FileExists(path) => {
                write!(f, "refusing to overwrite existing file {}", path.display())
            }
        }
    }
}

impl std::error::Error for TemplateError {}
impl From<io::Error> for TemplateError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn all_templates() -> Vec<ProjectTemplate> {
    vec![
        ProjectTemplate {
            id: TemplateId::NexulConsole, name: "NEXUL console",
            description: "Programme console NEXUL compilé avec les informations de débogage par défaut.",
            requirement: Requirement::One(Tool::Nxc),
            files: vec![
                file("nexul.toml", nexul_manifest("bonjour", "Application console NEXUL")),
                file("src/main.nx", "fn main() {\n    let nom = \"NEXUL\"\n    println(\"Bonjour depuis {nom} !\")\n}\n"),
                file(".gitignore", "/build/\n"),
                file("build/.gitkeep", ""),
            ],
            build: vec![BuildRecipe::NexulConsole],
            run: vec![RunRecipe::ProjectFile { path: "build/bonjour", args: vec![] }],
        },
        ProjectTemplate {
            id: TemplateId::NexulLibrary, name: "Bibliothèque NEXUL",
            description: "Module NEXUL public et petit programme d'exemple qui l'importe.",
            requirement: Requirement::All(vec![Tool::Nxc, Tool::Nxpm]),
            files: vec![
                file("nexul.toml", nexul_manifest("salutations", "Bibliothèque NEXUL avec exemple exécutable")),
                file("src/lib.nx", "pub fn dire_bonjour() {\n    println(\"Bonjour depuis la bibliothèque !\")\n}\n"),
                file("src/main.nx", "import lib\n\nfn main() {\n    lib.dire_bonjour()\n}\n"),
                file(".gitignore", "/build/\n"),
                file("build/.gitkeep", ""),
            ],
            build: vec![BuildRecipe::Tool { tool: Tool::Nxpm, args: vec!["build"] }],
            run: vec![RunRecipe::Tool { tool: Tool::Nxpm, args: vec!["run"] }],
        },
        ProjectTemplate {
            id: TemplateId::NxpmDependency, name: "Projet nxpm avec dépendance locale",
            description: "Application nxpm qui importe un package NEXUL local verrouillable.",
            requirement: Requirement::All(vec![Tool::Nxc, Tool::Nxpm]),
            files: vec![
                file("nexul.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\ndescription = \"Application avec dépendance locale\"\nmain = \"src/main.nx\"\n\n[dependencies]\nsalut = { path = \"vendor/salut\" }\n"),
                file("src/main.nx", "import salut\n\nfn main() {\n    salut.dire_bonjour()\n}\n"),
                file("vendor/salut/nexul.toml", nexul_manifest("salut", "Package de démonstration local")),
                file("vendor/salut/src/lib.nx", "pub fn dire_bonjour() {\n    println(\"Bonjour depuis la dépendance salut !\")\n}\n"),
                file(".gitignore", "/build/\n"),
                file("build/.gitkeep", ""),
            ],
            build: vec![BuildRecipe::Tool { tool: Tool::Nxpm, args: vec!["build"] }],
            run: vec![RunRecipe::Tool { tool: Tool::Nxpm, args: vec!["run"] }],
        },
        ProjectTemplate {
            id: TemplateId::WebSite, name: "Site HTML, CSS et JavaScript",
            description: "Site statique avec serveur local Node.js ou Python, sans dépendances à télécharger.",
            requirement: Requirement::Any(vec![Tool::Node, Tool::Python]),
            files: vec![
                file("index.html", "<!doctype html>\n<html lang=\"fr\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>Mon site NEXUL</title>\n  <link rel=\"stylesheet\" href=\"styles.css\">\n</head>\n<body>\n  <main><h1>Bonjour !</h1><p>Modifiez <code>index.html</code>, <code>styles.css</code> et <code>main.js</code>.</p><button id=\"hello\">Dire bonjour</button><output id=\"message\"></output></main>\n  <script type=\"module\" src=\"main.js\"></script>\n</body>\n</html>\n"),
                file("styles.css", "body { margin: 0; min-height: 100vh; display: grid; place-items: center; font: 16px system-ui, sans-serif; background: #f4f6fa; color: #172033; }\nmain { max-width: 42rem; padding: 3rem; background: white; border: 1px solid #d8deea; }\nbutton { padding: .6rem 1rem; }\noutput { display: block; margin-top: 1rem; }\n"),
                file("main.js", "document.querySelector('#hello').addEventListener('click', () => {\n  document.querySelector('#message').textContent = 'Le site fonctionne.';\n});\n"),
                file("server.mjs", NODE_SERVER),
                file("server.py", PYTHON_SERVER),
            ],
            build: vec![BuildRecipe::WebNode, BuildRecipe::WebPython],
            run: vec![RunRecipe::WebNode, RunRecipe::WebPython],
        },
        ProjectTemplate {
            id: TemplateId::Python, name: "Python",
            description: "Petit programme Python avec vérification syntaxique et exécution.",
            requirement: Requirement::One(Tool::Python),
            files: vec![file("main.py", "def main():\n    nom = \"NEXUL\"\n    print(f\"Bonjour depuis {nom} !\")\n\nif __name__ == \"__main__\":\n    main()\n"), file(".gitignore", "__pycache__/\n")],
            build: vec![BuildRecipe::Tool { tool: Tool::Python, args: vec!["-m", "py_compile", "main.py"] }],
            run: vec![RunRecipe::Tool { tool: Tool::Python, args: vec!["main.py"] }],
        },
        ProjectTemplate {
            id: TemplateId::C, name: "C",
            description: "Programme C construit par GCC, Clang ou MSVC selon le SDK découvert.",
            requirement: Requirement::Any(vec![Tool::Gcc, Tool::Clang, Tool::Cl]),
            files: vec![file("src/main.c", "#include <stdio.h>\n\nint main(void) {\n    puts(\"Bonjour depuis C !\");\n    return 0;\n}\n"), file(".gitignore", "/build/\n"), file("build/.gitkeep", "")],
            build: vec![BuildRecipe::CProgram { cpp: false }],
            run: vec![RunRecipe::CProgram { cpp: false }],
        },
        ProjectTemplate {
            id: TemplateId::Cpp, name: "C++",
            description: "Programme C++ construit par G++, Clang++ ou MSVC selon le SDK découvert.",
            requirement: Requirement::Any(vec![Tool::Gxx, Tool::Clangxx, Tool::Cl]),
            files: vec![file("src/main.cpp", "#include <iostream>\n\nint main() {\n    std::cout << \"Bonjour depuis C++ !\\n\";\n    return 0;\n}\n"), file(".gitignore", "/build/\n"), file("build/.gitkeep", "")],
            build: vec![BuildRecipe::CProgram { cpp: true }],
            run: vec![RunRecipe::CProgram { cpp: true }],
        },
        ProjectTemplate {
            id: TemplateId::Rust, name: "Rust",
            description: "Application Cargo autonome sans crate tierce.",
            requirement: Requirement::One(Tool::Cargo),
            files: vec![
                file("Cargo.toml", "[package]\nname = \"bonjour\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
                file("src/main.rs", "fn main() {\n    println!(\"Bonjour depuis Rust !\");\n}\n"),
                file(".gitignore", "/target/\n"),
            ],
            build: vec![BuildRecipe::RustCargo],
            run: vec![RunRecipe::Cargo],
        },
        ProjectTemplate {
            id: TemplateId::CSharp, name: "C#",
            description: "Application console .NET sans paquet tiers.",
            requirement: Requirement::One(Tool::Dotnet),
            files: vec![
                file("Bonjour.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <TargetFramework>net10.0</TargetFramework>\n    <ImplicitUsings>enable</ImplicitUsings>\n    <Nullable>enable</Nullable>\n  </PropertyGroup>\n</Project>\n"),
                file("Program.cs", "Console.WriteLine(\"Bonjour depuis C# !\");\n"),
                file(".gitignore", "/bin/\n/obj/\n"),
            ],
            build: vec![BuildRecipe::Dotnet],
            run: vec![RunRecipe::Dotnet],
        },
    ]
}

fn file(path: &str, contents: impl Into<String>) -> ProjectFile {
    ProjectFile {
        path: PathBuf::from(path),
        contents: contents.into(),
    }
}

fn nexul_manifest(name: &str, description: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\ndescription = \"{description}\"\nmain = \"src/main.nx\"\n\n[dependencies]\n")
}

const NODE_SERVER: &str = r#"import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(new URL('.', import.meta.url)));
const types = { '.html': 'text/html; charset=utf-8', '.css': 'text/css; charset=utf-8', '.js': 'text/javascript; charset=utf-8' };
const server = createServer(async (request, response) => {
  let pathname;
  try { pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname); }
  catch { response.writeHead(400).end('Bad request'); return; }
  if (pathname === '/') pathname = '/index.html';
  const file = resolve(root, `.${pathname}`);
  if (file !== root && !file.startsWith(root + sep)) { response.writeHead(403).end('Forbidden'); return; }
  try {
    const body = await readFile(file);
    response.writeHead(200, { 'Content-Type': types[extname(file)] || 'application/octet-stream' }).end(body);
  } catch { response.writeHead(404).end('Not found'); }
});
const port = Number(process.env.NEXUL_SITE_PORT || 8000);
server.listen(port, '127.0.0.1', () => console.log(`Site disponible sur http://127.0.0.1:${server.address().port}`));
"#;

const PYTHON_SERVER: &str = r#"from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import os

ROOT = Path(__file__).resolve().parent

class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(ROOT), **kwargs)

port = int(os.environ.get("NEXUL_SITE_PORT", "8000"))
server = ThreadingHTTPServer(("127.0.0.1", port), Handler)
print(f"Site disponible sur http://127.0.0.1:{server.server_address[1]}", flush=True)
server.serve_forever()
"#;

impl ProjectTemplate {
    pub fn files(&self) -> &[ProjectFile] {
        &self.files
    }

    pub fn is_available(&self, tools: &impl ToolchainProvider) -> bool {
        match &self.requirement {
            Requirement::One(tool) => tools.executable(*tool).is_some(),
            Requirement::Any(items) => items.iter().any(|tool| tools.executable(*tool).is_some()),
            Requirement::All(items) => items.iter().all(|tool| tools.executable(*tool).is_some()),
        }
    }

    pub fn missing_tools(&self, tools: &impl ToolchainProvider) -> Vec<String> {
        match &self.requirement {
            Requirement::One(tool) => {
                if tools.executable(*tool).is_some() {
                    vec![]
                } else {
                    vec![tool_name(*tool).into()]
                }
            }
            Requirement::All(items) => items
                .iter()
                .filter(|tool| tools.executable(**tool).is_none())
                .map(|tool| tool_name(*tool).to_owned())
                .collect(),
            Requirement::Any(items) => {
                if items.iter().any(|tool| tools.executable(*tool).is_some()) {
                    vec![]
                } else {
                    vec![items
                        .iter()
                        .map(|tool| tool_name(*tool))
                        .collect::<Vec<_>>()
                        .join(" or ")]
                }
            }
        }
    }

    /// Materialize this template without overwriting any existing file. The destination
    /// may already exist and contain files; only collisions with generated paths fail.
    pub fn create(&self, destination: impl AsRef<Path>) -> Result<(), TemplateError> {
        let destination = destination.as_ref();
        fs::create_dir_all(destination)?;
        let root = destination.canonicalize()?;
        for file in &self.files {
            if !is_safe_relative(&file.path) {
                return Err(TemplateError::UnsafePath(file.path.clone()));
            }
            let target = destination.join(&file.path);
            let parent = target
                .parent()
                .ok_or_else(|| TemplateError::UnsafePath(file.path.clone()))?;
            fs::create_dir_all(parent)?;
            if !parent.canonicalize()?.starts_with(&root) {
                return Err(TemplateError::UnsafePath(file.path.clone()));
            }
            let mut output = match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
            {
                Ok(output) => output,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    return Err(TemplateError::FileExists(target))
                }
                Err(error) => return Err(TemplateError::Io(error)),
            };
            output.write_all(file.contents.as_bytes())?;
        }
        Ok(())
    }

    pub fn actions(
        &self,
        tools: &impl ToolchainProvider,
        root: impl AsRef<Path>,
    ) -> Result<TemplateActions, TemplateError> {
        self.actions_with_site_port(tools, root, 8000)
    }

    pub fn actions_with_site_port(
        &self,
        tools: &impl ToolchainProvider,
        root: impl AsRef<Path>,
        port: u16,
    ) -> Result<TemplateActions, TemplateError> {
        let missing = self.missing_tools(tools);
        if !missing.is_empty() {
            return Err(TemplateError::MissingTools(missing));
        }
        let root = root.as_ref().to_path_buf();
        let mut build = Vec::new();
        for recipe in &self.build {
            if let Some(plan) = resolve_build(recipe, tools, &root, port) {
                build.push(plan);
            }
        }
        let mut run = Vec::new();
        for recipe in &self.run {
            if let Some(plan) = resolve_run(recipe, tools, &root, port) {
                run.push(plan);
            }
        }
        Ok(TemplateActions {
            build: build.into_iter().next(),
            run,
        })
    }
}

fn is_safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn resolve_build(
    recipe: &BuildRecipe,
    tools: &impl ToolchainProvider,
    root: &Path,
    _port: u16,
) -> Option<CommandPlan> {
    match recipe {
        BuildRecipe::Tool { tool, args } => Some(plan(
            "Build",
            spec(tools, *tool, args.iter().copied(), root)?,
        )),
        BuildRecipe::NexulConsole => {
            let output = if cfg!(windows) {
                "build/bonjour.exe"
            } else {
                "build/bonjour"
            };
            Some(plan(
                "Build",
                spec(
                    tools,
                    Tool::Nxc,
                    ["build", "src/main.nx", "-o", output],
                    root,
                )?,
            ))
        }
        BuildRecipe::CProgram { cpp } => c_build(tools, root, *cpp),
        BuildRecipe::RustCargo => Some(plan("Build", spec(tools, Tool::Cargo, ["build"], root)?)),
        BuildRecipe::Dotnet => Some(plan(
            "Build",
            spec(tools, Tool::Dotnet, ["build", "Bonjour.csproj"], root)?,
        )),
        BuildRecipe::WebNode => Some(plan("Vérifier JavaScript", web_node_check(tools, root)?)),
        BuildRecipe::WebPython => Some(plan(
            "Vérifier le serveur Python",
            spec(tools, Tool::Python, ["-m", "py_compile", "server.py"], root)?,
        )),
    }
}

fn resolve_run(
    recipe: &RunRecipe,
    tools: &impl ToolchainProvider,
    root: &Path,
    port: u16,
) -> Option<CommandPlan> {
    match recipe {
        RunRecipe::Tool { tool, args } => {
            Some(plan("Run", spec(tools, *tool, args.iter().copied(), root)?))
        }
        RunRecipe::ProjectFile { path, args } => {
            Some(plan("Run", project_file(path, args.iter().copied(), root)))
        }
        RunRecipe::CProgram { cpp } => c_run(tools, root, *cpp),
        RunRecipe::Cargo => Some(plan("Run", spec(tools, Tool::Cargo, ["run"], root)?)),
        RunRecipe::Dotnet => Some(plan(
            "Run",
            spec(
                tools,
                Tool::Dotnet,
                ["run", "--no-build", "--project", "Bonjour.csproj"],
                root,
            )?,
        )),
        RunRecipe::WebNode => Some(plan(
            "Serveur web (Node.js)",
            with_env(
                spec(tools, Tool::Node, ["server.mjs"], root)?,
                "NEXUL_SITE_PORT",
                port.to_string(),
            ),
        )),
        RunRecipe::WebPython => Some(plan(
            "Serveur web (Python)",
            with_env(
                spec(tools, Tool::Python, ["server.py"], root)?,
                "NEXUL_SITE_PORT",
                port.to_string(),
            ),
        )),
    }
}

fn spec<'a>(
    tools: &impl ToolchainProvider,
    tool: Tool,
    args: impl IntoIterator<Item = &'a str>,
    root: &Path,
) -> Option<CommandSpec> {
    Some(CommandSpec {
        label: String::new(),
        program: tools.executable(tool)?,
        args: args.into_iter().map(OsString::from).collect(),
        working_directory: root.to_path_buf(),
        env: Vec::new(),
    })
}

fn with_env(mut command: CommandSpec, key: &str, value: String) -> CommandSpec {
    command
        .env
        .push((OsString::from(key), OsString::from(value)));
    command
}

fn project_file(
    path: &str,
    args: impl IntoIterator<Item = &'static str>,
    root: &Path,
) -> CommandSpec {
    let executable = if cfg!(windows)
        && !Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        PathBuf::from(format!("{path}.exe"))
    } else {
        PathBuf::from(path)
    };
    CommandSpec {
        label: String::new(),
        program: root.join(executable),
        args: args.into_iter().map(OsString::from).collect(),
        working_directory: root.to_path_buf(),
        env: Vec::new(),
    }
}

fn plan(label: &str, mut command: CommandSpec) -> CommandPlan {
    command.label = label.to_owned();
    CommandPlan {
        label: label.to_owned(),
        commands: vec![command],
    }
}

fn web_node_check(tools: &impl ToolchainProvider, root: &Path) -> Option<CommandSpec> {
    let mut command = spec(tools, Tool::Node, ["--check", "server.mjs"], root)?;
    command.label = "Vérifier JavaScript".into();
    Some(command)
}

fn c_build(tools: &impl ToolchainProvider, root: &Path, cpp: bool) -> Option<CommandPlan> {
    let source = if cpp { "src/main.cpp" } else { "src/main.c" };
    if let Some(cl) = tools.executable(Tool::Cl) {
        let output = if cfg!(windows) {
            "build\\bonjour.exe"
        } else {
            "build/bonjour"
        };
        let mut args = vec![OsString::from("/nologo")];
        if cpp {
            args.push(OsString::from("/EHsc"));
        }
        args.push(OsString::from(format!("/Fe:{output}")));
        args.push(OsString::from(source));
        return Some(plan(
            "Build",
            CommandSpec {
                label: String::new(),
                program: cl,
                args,
                working_directory: root.to_path_buf(),
                env: Vec::new(),
            },
        ));
    }
    let tool = if cpp {
        if tools.executable(Tool::Clangxx).is_some() {
            Tool::Clangxx
        } else {
            Tool::Gxx
        }
    } else if tools.executable(Tool::Clang).is_some() {
        Tool::Clang
    } else {
        Tool::Gcc
    };
    let program = tools.executable(tool)?;
    let output = if cfg!(windows) {
        "build/bonjour.exe"
    } else {
        "build/bonjour"
    };
    Some(plan(
        "Build",
        CommandSpec {
            label: String::new(),
            program,
            args: [source, "-o", output]
                .into_iter()
                .map(OsString::from)
                .collect(),
            working_directory: root.to_path_buf(),
            env: Vec::new(),
        },
    ))
}

fn c_run(tools: &impl ToolchainProvider, root: &Path, cpp: bool) -> Option<CommandPlan> {
    let has_compiler = if cpp {
        tools.executable(Tool::Cl).is_some()
            || tools.executable(Tool::Clangxx).is_some()
            || tools.executable(Tool::Gxx).is_some()
    } else {
        tools.executable(Tool::Cl).is_some()
            || tools.executable(Tool::Clang).is_some()
            || tools.executable(Tool::Gcc).is_some()
    };
    has_compiler.then(|| plan("Run", project_file("build/bonjour", [], root)))
}

fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Nxc => "nxc",
        Tool::Nxpm => "nxpm",
        Tool::Python => "Python",
        Tool::Node => "Node.js",
        Tool::Gcc => "gcc",
        Tool::Clang => "clang",
        Tool::Cl => "cl",
        Tool::Gxx => "g++",
        Tool::Clangxx => "clang++",
        Tool::Cargo => "cargo",
        Tool::Rustc => "rustc",
        Tool::Dotnet => ".NET SDK",
    }
}

pub fn available_templates(tools: &impl ToolchainProvider) -> Vec<ProjectTemplate> {
    all_templates()
        .into_iter()
        .filter(|template| template.is_available(tools))
        .collect()
}

pub fn find_template(id: &str) -> Option<ProjectTemplate> {
    all_templates()
        .into_iter()
        .find(|template| template.id.as_str() == id)
}

/// Create a unique scratch directory for integrations and examples.
pub fn unique_temp_dir(prefix: &str) -> io::Result<PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct EmptyTools;
    impl ToolchainProvider for EmptyTools {
        fn executable(&self, _: Tool) -> Option<PathBuf> {
            None
        }
    }

    #[test]
    fn template_catalog_has_all_requested_project_types_and_no_unvalidated_wasm() {
        let ids: Vec<_> = all_templates()
            .into_iter()
            .map(|template| template.id)
            .collect();
        assert!(ids.contains(&TemplateId::NexulConsole));
        assert!(ids.contains(&TemplateId::NexulLibrary));
        assert!(ids.contains(&TemplateId::NxpmDependency));
        assert!(ids.contains(&TemplateId::WebSite));
        assert!(ids.contains(&TemplateId::Python));
        assert!(ids.contains(&TemplateId::C));
        assert!(ids.contains(&TemplateId::Cpp));
        assert!(ids.contains(&TemplateId::Rust));
        assert!(ids.contains(&TemplateId::CSharp));
        assert!(!ids.iter().any(|id| id.as_str().contains("wasm")));
    }

    #[test]
    fn empty_sdk_catalog_does_not_claim_external_templates_are_available() {
        assert!(available_templates(&EmptyTools).is_empty());
        let python = find_template("python").unwrap();
        assert_eq!(python.missing_tools(&EmptyTools), vec!["Python"]);
    }

    #[test]
    fn project_creation_writes_files_without_overwriting() {
        let root = unique_temp_dir("nexul-template-unit").unwrap();
        let template = find_template("web-site").unwrap();
        template.create(&root).unwrap();
        assert!(root.join("index.html").is_file());
        assert!(root.join("server.mjs").is_file());
        assert!(matches!(
            template.create(&root),
            Err(TemplateError::FileExists(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn toolchain_registry_is_only_a_catalog_and_resolves_commands() {
        let mut tools = ToolchainRegistry::default();
        tools.insert(Tool::Python, PathBuf::from("python"));
        let template = find_template("python").unwrap();
        assert!(template.is_available(&tools));
        let actions = template.actions(&tools, "project").unwrap();
        assert_eq!(
            actions.build.as_ref().unwrap().commands[0].program,
            PathBuf::from("python")
        );
        assert_eq!(
            actions.run[0].commands[0].args,
            vec![OsString::from("main.py")]
        );
    }
}

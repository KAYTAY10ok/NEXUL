//! The languages Studio edits, their language servers, the SDKs, and their detection on
//! this computer. Nothing is ever downloaded or installed: a server is found on `PATH` or
//! in the usual installation folders, and checked by running its version command when it
//! has one (a rustup proxy without its component, or the Microsoft Store alias of Python,
//! is not a server). A language without a server is "coloration seulement".

use crate::client::ServerCommand;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How to check that a found executable works.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Probe {
    /// Run it with these arguments: it must succeed; the version is read from its output.
    Version(&'static [&'static str]),
    /// Run it with these arguments: whatever its exit code, its output must contain the
    /// marker (`cl.exe` prints its banner and fails without input files).
    Banner(&'static [&'static str], &'static str),
    /// Existence is enough (servers without a version option, that would start serving).
    Exists,
}

/// A language server Studio knows how to start.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ServerSpec {
    pub id: &'static str,
    pub name: &'static str,
    /// Executable names tried in order (without extension).
    pub commands: &'static [&'static str],
    /// Arguments that start the server on stdio.
    pub args: &'static [&'static str],
    pub probe: Probe,
    /// How the user can install it (shown by Studio; never run).
    pub install_hint: &'static str,
}

/// A language, its file extensions and its servers in order of preference.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Language {
    pub id: &'static str,
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    /// `languageId` of `textDocument/didOpen` (see [`Language::lsp_language_id`]).
    pub lsp_id: &'static str,
    /// Ids of [`SERVERS`], preferred first.
    pub servers: &'static [&'static str],
}

impl Language {
    /// The `languageId` for a file: `typescriptreact` for `.tsx`, `scss` for `.scss`...
    pub fn lsp_language_id(&self, path: &Path) -> &'static str {
        let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        match ext.as_deref() {
            Some("tsx") => "typescriptreact",
            Some("jsx") => "javascriptreact",
            Some("scss") => "scss",
            Some("less") => "less",
            Some("jsonc") => "jsonc",
            _ => self.lsp_id,
        }
    }
}

/// A software development kit (or tool) Studio's project templates use. The ids are the
/// names of the executables: `nxc`, `nxpm`, `python`, `node`, `gcc`, `g++`, `clang`,
/// `clang++`, `cl`, `cargo`, `rustc`, `dotnet`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SdkSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub commands: &'static [&'static str],
    pub probe: Probe,
}

pub const SERVERS: &[ServerSpec] = &[
    ServerSpec {
        id: "rust-analyzer",
        name: "rust-analyzer",
        commands: &["rust-analyzer"],
        args: &[],
        probe: Probe::Version(&["--version"]),
        install_hint: "rustup component add rust-analyzer",
    },
    ServerSpec {
        id: "clangd",
        name: "clangd",
        commands: &["clangd"],
        args: &[],
        probe: Probe::Version(&["--version"]),
        install_hint: "winget install LLVM.LLVM (ou https://clangd.llvm.org/installation)",
    },
    ServerSpec {
        id: "pyright",
        name: "Pyright",
        commands: &["pyright-langserver", "basedpyright-langserver"],
        args: &["--stdio"],
        probe: Probe::Exists,
        install_hint: "npm install -g pyright",
    },
    ServerSpec {
        id: "pylsp",
        name: "Python LSP Server",
        commands: &["pylsp"],
        args: &[],
        probe: Probe::Version(&["--version"]),
        install_hint: "python -m pip install python-lsp-server",
    },
    ServerSpec {
        id: "pylsp-module",
        name: "Python LSP Server (module)",
        commands: &["python", "python3", "py"],
        args: &["-m", "pylsp"],
        probe: Probe::Version(&["-m", "pylsp", "--version"]),
        install_hint: "python -m pip install python-lsp-server",
    },
    ServerSpec {
        id: "typescript-language-server",
        name: "TypeScript Language Server",
        commands: &["typescript-language-server"],
        args: &["--stdio"],
        probe: Probe::Version(&["--version"]),
        install_hint: "npm install -g typescript-language-server typescript",
    },
    ServerSpec {
        id: "vscode-html",
        name: "VS Code HTML Language Server",
        commands: &["vscode-html-language-server"],
        args: &["--stdio"],
        probe: Probe::Exists,
        install_hint: "npm install -g vscode-langservers-extracted",
    },
    ServerSpec {
        id: "vscode-css",
        name: "VS Code CSS Language Server",
        commands: &["vscode-css-language-server"],
        args: &["--stdio"],
        probe: Probe::Exists,
        install_hint: "npm install -g vscode-langservers-extracted",
    },
    ServerSpec {
        id: "vscode-json",
        name: "VS Code JSON Language Server",
        commands: &["vscode-json-language-server"],
        args: &["--stdio"],
        probe: Probe::Exists,
        install_hint: "npm install -g vscode-langservers-extracted",
    },
    ServerSpec {
        id: "csharp-ls",
        name: "csharp-ls",
        commands: &["csharp-ls"],
        args: &[],
        probe: Probe::Exists,
        install_hint: "dotnet tool install --global csharp-ls",
    },
    ServerSpec {
        id: "omnisharp",
        name: "OmniSharp",
        commands: &["OmniSharp", "omnisharp"],
        args: &["-lsp"],
        probe: Probe::Exists,
        install_hint: "https://github.com/OmniSharp/omnisharp-roslyn/releases",
    },
    ServerSpec {
        id: "sqls",
        name: "sqls",
        commands: &["sqls"],
        args: &[],
        probe: Probe::Exists,
        install_hint: "go install github.com/sqls-server/sqls@latest",
    },
    ServerSpec {
        id: "sql-language-server",
        name: "SQL Language Server",
        commands: &["sql-language-server"],
        args: &["up", "--method", "stdio"],
        probe: Probe::Exists,
        install_hint: "npm install -g sql-language-server",
    },
];

/// The languages served through LSP. NEXUL is not among them: Studio uses `nxc ide` until
/// `nxc lsp` exists.
pub const LANGUAGES: &[Language] = &[
    Language { id: "rust", name: "Rust", extensions: &["rs"], lsp_id: "rust", servers: &["rust-analyzer"] },
    Language { id: "c", name: "C", extensions: &["c", "h"], lsp_id: "c", servers: &["clangd"] },
    Language {
        id: "cpp",
        name: "C++",
        extensions: &["cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++"],
        lsp_id: "cpp",
        servers: &["clangd"],
    },
    Language {
        id: "python",
        name: "Python",
        extensions: &["py", "pyi", "pyw"],
        lsp_id: "python",
        servers: &["pyright", "pylsp", "pylsp-module"],
    },
    Language {
        id: "javascript",
        name: "JavaScript",
        extensions: &["js", "mjs", "cjs", "jsx"],
        lsp_id: "javascript",
        servers: &["typescript-language-server"],
    },
    Language {
        id: "typescript",
        name: "TypeScript",
        extensions: &["ts", "mts", "cts", "tsx"],
        lsp_id: "typescript",
        servers: &["typescript-language-server"],
    },
    Language { id: "html", name: "HTML", extensions: &["html", "htm"], lsp_id: "html", servers: &["vscode-html"] },
    Language { id: "css", name: "CSS", extensions: &["css", "scss", "less"], lsp_id: "css", servers: &["vscode-css"] },
    Language { id: "json", name: "JSON", extensions: &["json", "jsonc"], lsp_id: "json", servers: &["vscode-json"] },
    Language { id: "csharp", name: "C#", extensions: &["cs"], lsp_id: "csharp", servers: &["csharp-ls", "omnisharp"] },
    Language { id: "sql", name: "SQL", extensions: &["sql"], lsp_id: "sql", servers: &["sqls", "sql-language-server"] },
];

pub const SDKS: &[SdkSpec] = &[
    SdkSpec { id: "nxc", name: "NEXUL (nxc)", commands: &["nxc"], probe: Probe::Version(&["--version"]) },
    SdkSpec { id: "nxpm", name: "NEXUL (nxpm)", commands: &["nxpm"], probe: Probe::Version(&["--version"]) },
    SdkSpec {
        id: "python",
        name: "Python",
        commands: &["python", "python3", "py"],
        probe: Probe::Version(&["--version"]),
    },
    SdkSpec { id: "node", name: "Node.js", commands: &["node"], probe: Probe::Version(&["--version"]) },
    SdkSpec { id: "gcc", name: "GCC", commands: &["gcc"], probe: Probe::Version(&["--version"]) },
    SdkSpec { id: "g++", name: "GCC (C++)", commands: &["g++"], probe: Probe::Version(&["--version"]) },
    SdkSpec { id: "clang", name: "Clang", commands: &["clang"], probe: Probe::Version(&["--version"]) },
    SdkSpec { id: "clang++", name: "Clang (C++)", commands: &["clang++"], probe: Probe::Version(&["--version"]) },
    SdkSpec { id: "cl", name: "MSVC (cl)", commands: &["cl"], probe: Probe::Banner(&[], "Microsoft") },
    SdkSpec { id: "cargo", name: "Cargo", commands: &["cargo"], probe: Probe::Version(&["--version"]) },
    SdkSpec { id: "rustc", name: "Rust (rustc)", commands: &["rustc"], probe: Probe::Version(&["--version"]) },
    SdkSpec { id: "dotnet", name: ".NET", commands: &["dotnet"], probe: Probe::Version(&["--version"]) },
];

/// The language of a file, from its extension (case-insensitive).
pub fn language_for_path(path: &Path) -> Option<&'static Language> {
    let ext = path.extension()?.to_str()?;
    LANGUAGES.iter().find(|l| l.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
}

pub fn language(id: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.id == id)
}

pub fn server_spec(id: &str) -> Option<&'static ServerSpec> {
    SERVERS.iter().find(|s| s.id == id)
}

/// A language server found on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedServer {
    pub spec: &'static ServerSpec,
    /// Ready for `Client::start`.
    pub command: ServerCommand,
    /// From its version command, when it has one.
    pub version: Option<String>,
}

/// An SDK found on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedSdk {
    pub spec: &'static SdkSpec,
    pub path: PathBuf,
    /// Version number (`3.14.0`), when found in its output.
    pub version: Option<String>,
    /// First line of its version output (`Python 3.14.0`).
    pub banner: Option<String>,
}

/// What Studio can offer for a language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageStatus {
    pub language: &'static Language,
    /// The preferred server found, or `None`: highlighting only.
    pub server: Option<DetectedServer>,
}

impl LanguageStatus {
    /// For the status bar: `"rust-analyzer 1.90.0"`, or `"coloration seulement"`.
    pub fn label(&self) -> String {
        match &self.server {
            Some(s) => match &s.version {
                Some(v) => format!("{} {v}", s.spec.name),
                None => s.spec.name.to_string(),
            },
            None => "coloration seulement".to_string(),
        }
    }

    /// Installation suggestions when no server is found (never run by Studio).
    pub fn install_hints(&self) -> Vec<&'static str> {
        if self.server.is_some() {
            return Vec::new();
        }
        let mut hints: Vec<&'static str> =
            self.language.servers.iter().filter_map(|id| server_spec(id)).map(|s| s.install_hint).collect();
        hints.dedup();
        hints
    }
}

/// The result of [`Detector::detect`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detection {
    /// Every language of [`LANGUAGES`], in order.
    pub languages: Vec<LanguageStatus>,
    /// Every server found, in the order of [`SERVERS`].
    pub servers: Vec<DetectedServer>,
    /// Every SDK found, in the order of [`SDKS`].
    pub sdks: Vec<DetectedSdk>,
}

impl Detection {
    pub fn language(&self, id: &str) -> Option<&LanguageStatus> {
        self.languages.iter().find(|l| l.language.id == id)
    }

    pub fn sdk(&self, id: &str) -> Option<&DetectedSdk> {
        self.sdks.iter().find(|s| s.spec.id == id)
    }
}

/// Finds executables and runs their probes.
#[derive(Clone, Debug)]
pub struct Detector {
    /// Folders searched, in order.
    pub dirs: Vec<PathBuf>,
    /// Extensions tried on Windows, in order (`exe`, `cmd`, `bat`, `com`: what can be
    /// started directly).
    pub extensions: Vec<String>,
    /// Time allowed to each probe (the process is killed after it).
    pub probe_timeout: Duration,
}

impl Detector {
    /// Searches `PATH`, then the usual installation folders: NEXUL (`bin`, `llvm\bin`),
    /// LLVM, Cargo, .NET tools, npm, Go...
    pub fn from_env() -> Detector {
        let mut dirs: Vec<PathBuf> =
            std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
        dirs.extend(usual_dirs());
        Detector::with_dirs(dirs)
    }

    /// Searches only `dirs` (tests, or a PATH configured in Studio).
    pub fn with_dirs(dirs: Vec<PathBuf>) -> Detector {
        let mut seen = HashSet::new();
        let dirs = dirs
            .into_iter()
            .filter(|d| !d.as_os_str().is_empty())
            .filter(|d| seen.insert(d.to_string_lossy().to_lowercase().trim_end_matches(['\\', '/']).to_string()))
            .collect();
        let extensions = if cfg!(windows) {
            let pathext = std::env::var("PATHEXT").unwrap_or_default().to_ascii_lowercase();
            let mut ext: Vec<String> = pathext
                .split(';')
                .map(|e| e.trim_start_matches('.').to_string())
                .filter(|e| matches!(e.as_str(), "exe" | "cmd" | "bat" | "com"))
                .collect();
            for e in ["exe", "cmd", "bat", "com"] {
                if !ext.iter().any(|x| x == e) {
                    ext.push(e.to_string());
                }
            }
            ext
        } else {
            Vec::new()
        };
        Detector { dirs, extensions, probe_timeout: Duration::from_secs(10) }
    }

    /// Every executable named `name` in the folders, in order.
    pub fn find_all(&self, name: &str) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for dir in &self.dirs {
            if cfg!(windows) {
                // The Microsoft Store aliases of Python open the Store when Python is not
                // installed from it: never run them.
                if name.starts_with("python") && dir.to_string_lossy().to_lowercase().contains("microsoft\\windowsapps")
                {
                    continue;
                }
                for ext in &self.extensions {
                    let p = dir.join(format!("{name}.{ext}"));
                    if p.is_file() {
                        out.push(p);
                    }
                }
            } else {
                let p = dir.join(name);
                if is_executable(&p) {
                    out.push(p);
                }
            }
        }
        out
    }

    /// The first executable named `name`.
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        self.find_all(name).into_iter().next()
    }

    /// The first candidate (by command, then folder) whose probe succeeds.
    fn first_working(&self, commands: &[&str], probe: Probe, extra: Vec<PathBuf>) -> Option<(PathBuf, Option<String>)> {
        let candidates = commands.iter().flat_map(|c| self.find_all(c)).chain(extra);
        for path in candidates {
            match probe {
                Probe::Exists => return Some((path, None)),
                Probe::Version(args) => {
                    if let Some((true, out)) = run_probe(&path, args, self.probe_timeout) {
                        return Some((path, first_line(&out)));
                    }
                }
                Probe::Banner(args, marker) => {
                    if let Some((_, out)) = run_probe(&path, args, self.probe_timeout) {
                        if out.contains(marker) {
                            return Some((path, first_line(&out)));
                        }
                    }
                }
            }
        }
        None
    }

    /// Looks for a server.
    pub fn server(&self, spec: &'static ServerSpec) -> Option<DetectedServer> {
        let (path, banner) = self.first_working(spec.commands, spec.probe, Vec::new())?;
        Some(DetectedServer {
            spec,
            command: ServerCommand {
                program: path,
                args: spec.args.iter().map(|a| a.to_string()).collect(),
                env: Vec::new(),
            },
            version: banner.as_deref().and_then(version_number),
        })
    }

    /// Looks for an SDK. `cl` is also looked for in the Visual Studio installations (it
    /// then needs the developer environment, `vcvars64.bat`, to compile).
    pub fn sdk(&self, spec: &'static SdkSpec) -> Option<DetectedSdk> {
        let extra = if spec.id == "cl" { msvc_cl() } else { Vec::new() };
        let (path, banner) = self.first_working(spec.commands, spec.probe, extra)?;
        Some(DetectedSdk { spec, path, version: banner.as_deref().and_then(version_number), banner })
    }

    /// Detects every server and SDK, in parallel (a few seconds at most), and the status
    /// of every language.
    pub fn detect(&self) -> Detection {
        let (servers, sdks) = thread::scope(|s| {
            let servers: Vec<_> = SERVERS.iter().map(|spec| s.spawn(move || self.server(spec))).collect();
            let sdks: Vec<_> = SDKS.iter().map(|spec| s.spawn(move || self.sdk(spec))).collect();
            (
                servers.into_iter().filter_map(|h| h.join().ok().flatten()).collect::<Vec<_>>(),
                sdks.into_iter().filter_map(|h| h.join().ok().flatten()).collect::<Vec<_>>(),
            )
        });
        let languages = LANGUAGES
            .iter()
            .map(|language| LanguageStatus {
                language,
                server: language.servers.iter().find_map(|id| servers.iter().find(|s| s.spec.id == *id)).cloned(),
            })
            .collect();
        Detection { languages, servers, sdks }
    }
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

fn usual_dirs() -> Vec<PathBuf> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let mut dirs = Vec::new();
    let home = env(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    if cfg!(windows) {
        if let Some(local) = env("LOCALAPPDATA") {
            let nexul = local.join("Programs").join("NEXUL");
            dirs.push(nexul.join("bin"));
            dirs.push(nexul.join("llvm").join("bin"));
        }
        if let Some(pf) = env("ProgramFiles") {
            dirs.push(pf.join("LLVM").join("bin"));
            dirs.push(pf.join("nodejs"));
            dirs.push(pf.join("dotnet"));
        }
        if let Some(appdata) = env("APPDATA") {
            dirs.push(appdata.join("npm"));
        }
    } else {
        dirs.extend(["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin", "/bin"].map(PathBuf::from));
    }
    if let Some(h) = home {
        dirs.push(h.join(".cargo").join("bin"));
        dirs.push(h.join(".dotnet").join("tools"));
        dirs.push(h.join("go").join("bin"));
        dirs.push(h.join(".local").join("bin"));
    }
    dirs.into_iter().filter(|d| d.is_dir()).collect()
}

/// `cl.exe` of the Visual Studio installations that have the C++ tools (found with
/// `vswhere.exe`, which comes with Visual Studio).
fn msvc_cl() -> Vec<PathBuf> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let Some(pf86) = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from) else { return Vec::new() };
    let vswhere = pf86.join("Microsoft Visual Studio").join("Installer").join("vswhere.exe");
    if !vswhere.is_file() {
        return Vec::new();
    }
    let args = [
        "-all",
        "-products",
        "*",
        "-requires",
        "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
        "-property",
        "installationPath",
    ];
    let Some((true, out)) = run_probe(&vswhere, &args, Duration::from_secs(10)) else { return Vec::new() };
    let mut found = Vec::new();
    for install in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let vc = Path::new(install).join("VC");
        let default = vc.join("Auxiliary").join("Build").join("Microsoft.VCToolsVersion.default.txt");
        if let Ok(version) = std::fs::read_to_string(default) {
            let cl = vc
                .join("Tools")
                .join("MSVC")
                .join(version.trim())
                .join("bin")
                .join("Hostx64")
                .join("x64")
                .join("cl.exe");
            if cl.is_file() {
                found.push(cl);
            }
        }
    }
    found
}

/// Runs `program args` without window and without input; returns whether it succeeded and
/// its output (stdout then stderr), or `None` if it could not be started or ran longer
/// than `timeout` (it is then killed).
pub fn run_probe(program: &Path, args: &[&str], timeout: Duration) -> Option<(bool, String)> {
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    // Linux refuses to run a file that another process still holds open for writing
    // (ETXTBSY, 26): a tool copied while another thread forks is briefly in that state.
    let mut attempt = 0;
    let mut child = loop {
        match cmd.spawn() {
            Ok(child) => break child,
            Err(e) if e.raw_os_error() == Some(26) && cfg!(unix) && attempt < 20 => {
                attempt += 1;
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return None,
        }
    };
    let read = |r: Option<Box<dyn Read + Send>>| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut r) = r {
                // Version outputs are short; do not keep reading a runaway program.
                let _ = r.by_ref().take(64 * 1024).read_to_end(&mut buf);
            }
            buf
        })
    };
    let out = read(child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    let err = read(child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            _ => {
                crate::client::kill_tree(&mut child);
                return None;
            }
        }
    };
    let mut text = String::from_utf8_lossy(&out.join().unwrap_or_default()).into_owned();
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&err.join().unwrap_or_default()));
    Some((status.success(), text))
}

fn first_line(out: &str) -> Option<String> {
    out.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_string)
}

/// The first word that looks like a version number: `rustc 1.99.0 (b940084d7)` gives
/// `1.99.0`, `v22.11.0` gives `22.11.0`.
pub fn version_number(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | ',' | '"'))
        .map(|w| w.trim_start_matches(['v', 'V']))
        .find(|w| {
            let mut parts = w.split('.');
            w.contains('.')
                && parts.next().is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
                && parts.next().is_some_and(|p| p.bytes().next().is_some_and(|b| b.is_ascii_digit()))
        })
        .map(|w| w.trim_end_matches('.').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_consistent() {
        for l in LANGUAGES {
            assert!(!l.servers.is_empty(), "{}", l.id);
            for s in l.servers {
                assert!(server_spec(s).is_some(), "{}: unknown server {s}", l.id);
            }
        }
        for s in SERVERS {
            assert!(LANGUAGES.iter().any(|l| l.servers.contains(&s.id)), "{} serves no language", s.id);
        }
        // Each extension belongs to one language.
        let mut seen = HashSet::new();
        for l in LANGUAGES {
            for e in l.extensions {
                assert!(seen.insert(*e), "extension {e} twice");
            }
        }
        // The SDK ids are the tools of studio/templates.
        let ids: Vec<&str> = SDKS.iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            ["nxc", "nxpm", "python", "node", "gcc", "g++", "clang", "clang++", "cl", "cargo", "rustc", "dotnet"]
        );
        assert!(language_for_path(Path::new("main.nx")).is_none(), "NEXUL is served by nxc ide");
    }

    #[test]
    fn languages_of_files() {
        let id = |p: &str| language_for_path(Path::new(p)).map(|l| l.id);
        assert_eq!(id("src/main.rs"), Some("rust"));
        assert_eq!(id("a.H"), Some("c"));
        assert_eq!(id("a.hpp"), Some("cpp"));
        assert_eq!(id("app.tsx"), Some("typescript"));
        assert_eq!(id("x.txt"), None);
        let ts = language("typescript").unwrap();
        assert_eq!(ts.lsp_language_id(Path::new("a.tsx")), "typescriptreact");
        assert_eq!(ts.lsp_language_id(Path::new("a.ts")), "typescript");
        assert_eq!(language("css").unwrap().lsp_language_id(Path::new("a.SCSS")), "scss");
    }

    #[test]
    fn version_numbers() {
        for (text, v) in [
            ("rustc 1.99.0 (b940084d7 2026-09-28)", Some("1.99.0")),
            ("v22.11.0", Some("22.11.0")),
            ("Python 3.14.0", Some("3.14.0")),
            ("nxc 0.2.0 (NEXUL 0.2)", Some("0.2.0")),
            ("gcc.exe (x86_64-posix-seh-rev0, Built by MinGW-Builds project) 14.2.0", Some("14.2.0")),
            ("clangd version 19.1.5", Some("19.1.5")),
            ("Microsoft (R) C/C++ Optimizing Compiler Version 19.44.35207 for x64", Some("19.44.35207")),
            ("10.0.100", Some("10.0.100")),
            ("no version here, x86_64", None),
        ] {
            assert_eq!(version_number(text).as_deref(), v, "{text}");
        }
    }

    #[test]
    fn labels() {
        let rust = language("rust").unwrap();
        let none = LanguageStatus { language: rust, server: None };
        assert_eq!(none.label(), "coloration seulement");
        assert_eq!(none.install_hints(), ["rustup component add rust-analyzer"]);
        let spec = server_spec("rust-analyzer").unwrap();
        let some = LanguageStatus {
            language: rust,
            server: Some(DetectedServer {
                spec,
                command: ServerCommand::new("ra", &[]),
                version: Some("1.2.3".into()),
            }),
        };
        assert_eq!(some.label(), "rust-analyzer 1.2.3");
        assert!(some.install_hints().is_empty());
    }
}

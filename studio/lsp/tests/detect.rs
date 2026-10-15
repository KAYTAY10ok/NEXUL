//! Detection of servers and SDKs, in a folder of fake tools, and on this computer.

mod common;

use common::*;
use nexul_studio_lsp::registry::{self, Detector};
use std::path::Path;
use std::time::{Duration, Instant};

/// A copy of the fake server named `name` (it answers `--version` with
/// `fake-lsp-server 1.2.3`).
fn fake_tool(dir: &Path, name: &str) {
    let target = dir.join(if cfg!(windows) { format!("{name}.exe") } else { name.to_string() });
    std::fs::copy(fake_exe(), &target).unwrap();
}

/// A script named `name` that prints `output` and exits with `code`.
fn script(dir: &Path, name: &str, output: &str, code: i32) {
    if cfg!(windows) {
        let body = if output.is_empty() { String::new() } else { format!("echo {output}\r\n") };
        std::fs::write(dir.join(format!("{name}.cmd")), format!("@echo off\r\n{body}exit /b {code}\r\n")).unwrap();
    } else {
        let p = dir.join(name);
        std::fs::write(&p, format!("#!/bin/sh\necho '{output}'\nexit {code}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
}

#[test]
fn detection_in_a_folder_of_fake_tools() {
    let dir = TempDir::new();
    for t in ["rust-analyzer", "clangd", "pylsp", "nxc"] {
        fake_tool(dir.path(), t);
    }
    // Like a rustup proxy without its component: present, but its version command fails.
    script(dir.path(), "typescript-language-server", "error: Unknown binary 'typescript-language-server.exe'", 1);
    // A server without version option: existence is enough (it is not run).
    script(dir.path(), "vscode-html-language-server", "", 0);
    // `cl` prints its banner and fails without input files.
    script(dir.path(), "cl", "Microsoft (R) C/C++ Optimizing Compiler Version 19.44.35207 for x64", 2);
    script(dir.path(), "dotnet", "10.0.100", 0);

    let detector = Detector::with_dirs(vec![dir.path().to_path_buf()]);
    let d = detector.detect();
    let label = |id: &str| d.language(id).unwrap().label();
    assert_eq!(label("rust"), "rust-analyzer 1.2.3");
    assert_eq!(label("c"), "clangd 1.2.3");
    assert_eq!(label("cpp"), "clangd 1.2.3");
    assert_eq!(label("python"), "Python LSP Server 1.2.3");
    assert_eq!(label("html"), "VS Code HTML Language Server");
    for id in ["javascript", "typescript", "css", "json", "csharp", "sql"] {
        assert_eq!(label(id), "coloration seulement", "{id}");
    }
    assert_eq!(
        d.language("typescript").unwrap().install_hints(),
        ["npm install -g typescript-language-server typescript"]
    );
    assert_eq!(d.language("sql").unwrap().install_hints().len(), 2);

    // The command is ready for `Client::start`.
    let ra = d.language("rust").unwrap().server.as_ref().unwrap();
    assert!(ra.command.program.starts_with(dir.path()));
    assert!(ra.command.args.is_empty());
    let html = d.language("html").unwrap().server.as_ref().unwrap();
    assert_eq!(html.command.args, ["--stdio"]);

    let sdks: Vec<(&str, Option<&str>)> = d.sdks.iter().map(|s| (s.spec.id, s.version.as_deref())).collect();
    assert_eq!(sdks, [("nxc", Some("1.2.3")), ("cl", Some("19.44.35207")), ("dotnet", Some("10.0.100"))]);
    assert_eq!(d.sdk("nxc").unwrap().banner.as_deref(), Some("fake-lsp-server 1.2.3"));
}

#[test]
fn a_probe_that_hangs_is_killed() {
    let dir = TempDir::new();
    if cfg!(windows) {
        std::fs::write(dir.path().join("clangd.cmd"), "@echo off\r\nping -n 30 127.0.0.1 >nul\r\n").unwrap();
    } else {
        // `script` makes the file executable; its content is then replaced by a sleep.
        script(dir.path(), "clangd", "", 0);
        std::fs::write(dir.path().join("clangd"), "#!/bin/sh\nsleep 30\n").unwrap();
    }
    let mut detector = Detector::with_dirs(vec![dir.path().to_path_buf()]);
    detector.probe_timeout = Duration::from_millis(500);
    let t0 = Instant::now();
    assert!(detector.server(registry::server_spec("clangd").unwrap()).is_none());
    assert!(t0.elapsed() < Duration::from_secs(10), "{:?}", t0.elapsed());
}

#[cfg(windows)]
#[test]
fn store_aliases_of_python_are_never_run() {
    let dir = TempDir::new();
    let apps = dir.path().join("Microsoft").join("WindowsApps");
    std::fs::create_dir_all(&apps).unwrap();
    fake_tool(&apps, "python");
    fake_tool(&apps, "python3");
    fake_tool(&apps, "winget");
    let detector = Detector::with_dirs(vec![apps.clone()]);
    assert_eq!(detector.find("python"), None);
    assert_eq!(detector.find("python3"), None);
    assert!(detector.find("winget").is_some());
}

#[test]
fn search_order_and_extensions() {
    let (a, b) = (TempDir::new(), TempDir::new());
    fake_tool(a.path(), "tool");
    fake_tool(b.path(), "tool");
    let detector = Detector::with_dirs(vec![b.path().to_path_buf(), a.path().to_path_buf(), b.path().to_path_buf()]);
    // Folders in order, duplicates removed.
    assert_eq!(detector.dirs.len(), 2);
    let all = detector.find_all("tool");
    assert_eq!(all.len(), 2);
    assert!(all[0].starts_with(b.path()));
    assert_eq!(detector.find("absent"), None);
    if cfg!(windows) {
        // Only what can be started directly.
        std::fs::write(a.path().join("script.js"), "").unwrap();
        assert_eq!(detector.find("script"), None);
        assert!(detector.extensions.iter().any(|e| e == "cmd"));
    }
}

/// What this computer has (printed; the result depends on the machine).
#[test]
fn detection_on_this_computer() {
    let t0 = Instant::now();
    let d = Detector::from_env().detect();
    let took = t0.elapsed();
    for l in &d.languages {
        eprintln!("{:<11} {}", l.language.name, l.label());
    }
    for s in &d.sdks {
        eprintln!("SDK {:<14} {:<12} {}", s.spec.name, s.version.as_deref().unwrap_or("?"), s.path.display());
    }
    eprintln!("detection in {took:?}");
    assert_eq!(d.languages.len(), registry::LANGUAGES.len());
    assert!(took < Duration::from_secs(60), "{took:?}");
    // Never a Store alias of Python.
    assert!(d.sdks.iter().all(|s| !s.path.to_string_lossy().to_lowercase().contains("windowsapps\\python")));
}

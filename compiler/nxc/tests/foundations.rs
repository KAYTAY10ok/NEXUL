use nxc_driver::{compile_file, CompileOptions, Mode};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("nxc-foundations-{}-{}", std::process::id(), id));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_nxc(args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nxc"));
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().unwrap();
    if let Some(bytes) = stdin {
        child.stdin.take().unwrap().write_all(bytes).unwrap();
    }
    child.wait_with_output().unwrap()
}

fn create_project(dir: &Path, main: &str, calc: &str) -> (PathBuf, PathBuf) {
    std::fs::create_dir_all(dir.join("src/tools")).unwrap();
    std::fs::write(
        dir.join("nexul.toml"),
        "[package]\nname = \"foundations\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let main_path = dir.join("src/main.nx");
    let calc_path = dir.join("src/tools/calc.nx");
    std::fs::write(&main_path, main).unwrap();
    std::fs::write(&calc_path, calc).unwrap();
    (main_path, calc_path)
}

#[test]
fn check_stdin_overlay_is_seen_through_the_project_main() {
    let scratch = Scratch::new();
    let (_, calc) = create_project(
        scratch.path(),
        "import tools.calc\nfn main() {}\n",
        "pub fn value() -> i32 { 1 }\n",
    );
    let output = run_nxc(
        &["check", calc.to_str().unwrap(), "--stdin", "--json=1"],
        Some(b"pub fn value() -> i32 { missing }\n"),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    assert!(
        stdout.contains("\"schema\":\"nexul.diagnostics\""),
        "{stdout}"
    );
    assert!(stdout.contains("calc.nx"), "{stdout}");
    assert!(stdout.contains("\"diagnostics\":[{"), "{stdout}");

    let legacy = run_nxc(&["check", calc.to_str().unwrap(), "--stdin", "--json"], Some(b"pub fn value() -> i32 { missing }\n"));
    let legacy_stdout = String::from_utf8_lossy(&legacy.stdout);
    assert_eq!(legacy.status.code(), Some(1), "{legacy_stdout}");
    assert!(legacy_stdout.starts_with("[{\"level\":"), "{legacy_stdout}");
}

#[test]
fn check_of_imported_project_file_uses_the_project_main() {
    let scratch = Scratch::new();
    let (main, calc) = create_project(
        scratch.path(),
        "import tools.calc\nfn main() { missing }\n",
        "pub fn value() -> i32 { 1 }\n",
    );
    let output = run_nxc(&["check", calc.to_str().unwrap(), "--json=1"], None);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    assert!(
        stdout.contains("main.nx"),
        "expected diagnostics from {}: {stdout}",
        main.display()
    );
}

#[test]
fn ide_analysis_uses_overlays_for_open_imported_documents() {
    let scratch = Scratch::new();
    let (main, calc) = create_project(scratch.path(), "import tools.calc\nfn main() {}\n", "pub fn value() -> i32 { 1 }\n");
    let edited_main = "import tools.calc\nfn main() { } // changed without saving\n".to_string();
    let edited = "pub fn value() -> i32 { 2 }\n".to_string();
    let analysis = nxc_driver::ide::analyze_document(&calc, None, vec![(main.clone(), edited_main.clone()), (calc.clone(), edited.clone())]).unwrap();
    assert_eq!(analysis.sm.get(analysis.prog.modules[analysis.root.idx()].file).text, edited_main);
    let imported = analysis.prog.modules.iter().find_map(|module| {
        let source = analysis.sm.get(module.file);
        source.name.ends_with("tools/calc.nx").then_some(source.text.as_str())
    }).unwrap();
    assert_eq!(imported, edited);
}

#[test]
fn json_v1_and_version_are_versioned_while_json_v0_stays_unchanged() {
    let scratch = Scratch::new();
    let source = scratch.path().join("invalid.nx");
    std::fs::write(&source, "fn main() { missing }\n").unwrap();

    let expected_v0 = compile_file(&source, &CompileOptions::new(Mode::Check)).json;
    let old = run_nxc(&["check", source.to_str().unwrap(), "--json"], None);
    assert_eq!(String::from_utf8_lossy(&old.stdout).trim_end(), expected_v0);

    let v1 = run_nxc(&["check", source.to_str().unwrap(), "--json=1"], None);
    let stdout = String::from_utf8_lossy(&v1.stdout);
    assert!(
        stdout.starts_with("{\"schema\":\"nexul.diagnostics\",\"version\":1,"),
        "{stdout}"
    );
    assert!(stdout.contains("\"path\":"), "{stdout}");
    assert!(stdout.contains(&nxc_driver::json_str(&source.to_string_lossy())), "{stdout}");
    assert!(stdout.contains("\"range\":{"), "{stdout}");

    let version = run_nxc(&["version", "--json"], None);
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        "{\"nxc\":\"0.2.0\",\"nexul\":\"0.2\",\"json\":[0,1],\"lsp\":true,\"editions\":[\"0.2\",\"1\"]}"
    );
}

#[test]
fn ide_json_v1_wraps_existing_results() {
    let scratch = Scratch::new();
    let source = scratch.path().join("main.nx");
    std::fs::write(&source, "fn main() {}\n").unwrap();
    let output = run_nxc(
        &["ide", "symbols", source.to_str().unwrap(), "--json=1"],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("{\"schema\":\"nexul.ide\",\"version\":1,"),
        "{stdout}"
    );
    assert!(stdout.contains("\"result\":["), "{stdout}");
}

#[test]
fn diagnostics_json_v1_uses_standard_library_uris() {
    let mut sources = nxc_syntax::SourceMap::new();
    let file = sources.add("<std.vec>", "Vec");
    let diagnostic = nxc_syntax::Diagnostic::error("E0000", "test diagnostic")
        .primary(nxc_syntax::Span::new(file, 0, 3), "test span");
    let json = nxc_driver::diagnostics_json_v1(&[diagnostic], &sources, None, &nxc_driver::Target::host());
    assert!(json.contains("\"path\":\"nexul-std:///vec.nx\""), "{json}");
}

//! Self-hosting, step SH2: the lexer written in NEXUL (`selfhost/nxc/lexer.nx`, through
//! the tool `selfhost/lex.nx`) gives, for every `.nx` file of the repository, the same
//! tokens and the same diagnostics as the lexer of `nxc` (`nxc debug tokens --json`):
//! kinds, values, positions, line starts, string parts and format specifications, codes
//! and messages of the errors.
//!
//! The corpus is every `.nx` file under the root of the repository: the standard library,
//! the tests (`tests/ui` holds the errors of the lexer), the examples, the self-hosted
//! compiler itself, and `selfhost/corpus`, written to hit every rule of the lexer.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if !name.starts_with('.') && name != "target" && name != "node_modules" {
                walk(&p, out);
            }
        } else if name.ends_with(".nx") {
            out.push(p);
        }
    }
}

fn output(cmd: &mut Command, what: &str) -> String {
    let out = cmd.output().unwrap_or_else(|e| panic!("{}: {}", what, e));
    assert!(out.status.success(), "{} failed ({}):\n{}", what, out.status, String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap_or_else(|_| panic!("{}: output is not UTF-8", what))
}

#[test]
fn the_lexer_in_nexul_gives_the_tokens_of_nxc() {
    let nxc = env!("CARGO_BIN_EXE_nxc");
    let dir = std::env::temp_dir().join(format!("nxc-selfhost-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let lex = dir.join(if cfg!(windows) { "lex.exe" } else { "lex" });
    output(Command::new(nxc).arg("build").arg(root().join("selfhost").join("lex.nx")).arg("--release").arg("-o").arg(&lex), "nxc build selfhost/lex.nx");

    let mut files = Vec::new();
    walk(&root(), &mut files);
    files.sort();
    assert!(files.len() > 200, "only {} .nx files found", files.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let failures = std::sync::Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(f) = files.get(i) else { break };
                // Files that are not UTF-8 are refused by nxc before its lexer.
                if std::fs::read_to_string(f).is_err() {
                    continue;
                }
                let expected = output(Command::new(nxc).arg("debug").arg("tokens").arg("--json").arg(f), "nxc debug tokens");
                let edition = expected.lines().next().unwrap_or("").trim_start_matches("{\"edition\":\"").trim_end_matches("\"}").to_string();
                let got = output(Command::new(&lex).arg(f).arg("--edition").arg(&edition), "selfhost lex");
                if got != expected {
                    let line = expected.lines().zip(got.lines()).position(|(a, b)| a != b).unwrap_or_else(|| expected.lines().count().min(got.lines().count()));
                    failures.lock().unwrap().push(format!(
                        "{}: first difference at line {}\n  nxc:  {}\n  NEXUL: {}",
                        f.display(),
                        line + 1,
                        expected.lines().nth(line).unwrap_or("(end)"),
                        got.lines().nth(line).unwrap_or("(end)")
                    ));
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(&dir);
    let failures = failures.into_inner().unwrap();
    assert!(failures.is_empty(), "{} of {} files differ:\n{}", failures.len(), files.len(), failures.join("\n"));
}

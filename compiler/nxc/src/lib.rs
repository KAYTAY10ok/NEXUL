//! The NEXUL compiler driver: front end, NIR, C generation and the C toolchain.

pub mod codes;

use nxc_codegen_c::{Entry, Options};
use nxc_sema::defs::{FnId, ItemRef};
use nxc_sema::ProgramSource;
use nxc_syntax::diag::render_all;
use nxc_syntax::{DiagSink, Diagnostic, Level, SourceMap, Span};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Analysis only.
    Check,
    /// A program with `main`.
    Program,
    /// The `@test` functions.
    Tests,
}

pub struct Compilation {
    /// Rendered diagnostics (errors and warnings).
    pub diagnostics: String,
    pub errors: usize,
    pub warnings: usize,
    pub c_source: Option<String>,
    pub links: Vec<String>,
    /// Names of the test functions, in the order of their index.
    pub tests: Vec<String>,
}

/// Runs the whole front end and generates C (unless `mode` is `Check`).
pub fn compile_file(path: &Path, mode: Mode, color: bool) -> Compilation {
    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "main.nx".into());
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            return Compilation {
                diagnostics: format!("error: cannot read `{}`: {}\n", path.display(), e),
                errors: 1,
                warnings: 0,
                c_source: None,
                links: vec![],
                tests: vec![],
            }
        }
    };
    compile_source(&name, bytes, path.parent().map(|p| p.to_path_buf()), mode, color)
}

pub fn compile_source(name: &str, bytes: Vec<u8>, root_dir: Option<PathBuf>, mode: Mode, color: bool) -> Compilation {
    let mut sm = SourceMap::new();
    let diags = DiagSink::new();
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => {
            let pos = e.utf8_error().valid_up_to();
            let lossy = String::from_utf8_lossy(e.as_bytes()).to_string();
            let f = sm.add(name, lossy);
            diags.emit(
                Diagnostic::error("E0001", "source file is not valid UTF-8")
                    .primary(Span::new(f, pos as u32, pos as u32 + 1), "invalid byte sequence")
                    .help("save the file with the UTF-8 encoding"),
            );
            return finish(&sm, &diags, None, vec![], vec![], color);
        }
    };
    let text = text.strip_prefix('\u{feff}').map(|s| s.to_string()).unwrap_or(text);
    let src = ProgramSource { root_name: name.to_string(), root_text: text, root_dir };
    let prog = nxc_sema::analyze(src, &mut sm, &diags);
    if diags.has_errors() {
        return finish(&sm, &diags, None, vec![], vec![], color);
    }
    let bodies = nxc_nir::lower_program(&prog, &sm, &diags);
    if diags.has_errors() || mode == Mode::Check {
        return finish(&sm, &diags, None, vec![], vec![], color);
    }
    let entry = match mode {
        Mode::Program => match prog.main_fn {
            Some(f) => Entry::Main(f),
            None => {
                let root_file = prog.modules[prog.root.idx()].file;
                diags.emit(
                    Diagnostic::error("E0230", "no `main` function")
                        .primary(Span::new(root_file, 0, 0), "the program starts with `fn main()` in this file")
                        .help("add `fn main() { ... }`, or use `nxc test` to run `@test` functions"),
                );
                return finish(&sm, &diags, None, vec![], vec![], color);
            }
        },
        Mode::Tests => {
            let mut tests: Vec<FnId> = Vec::new();
            for m in prog.modules.iter().filter(|m| !m.is_std) {
                let mut fns: Vec<(u32, FnId)> = m
                    .items
                    .values()
                    .filter_map(|(it, span, _)| match it {
                        ItemRef::Fn(f) if prog.func(*f).is_test => Some((span.lo, *f)),
                        _ => None,
                    })
                    .collect();
                fns.sort();
                tests.extend(fns.into_iter().map(|(_, f)| f));
            }
            Entry::Tests(tests)
        }
        Mode::Check => unreachable!(),
    };
    let test_names = match &entry {
        Entry::Tests(t) => t
            .iter()
            .map(|&f| {
                let fd = prog.func(f);
                let m = &prog.modules[fd.module.idx()].path;
                format!("{}.{}", m, fd.name)
            })
            .collect(),
        _ => vec![],
    };
    let (c, links) = nxc_codegen_c::generate(&prog, &sm, &bodies, &entry, &Options::default());
    finish(&sm, &diags, Some(c), links, test_names, color)
}

fn finish(sm: &SourceMap, diags: &DiagSink, c: Option<String>, links: Vec<String>, tests: Vec<String>, color: bool) -> Compilation {
    let mut list = diags.take_sorted();
    let errors = list.iter().filter(|d| d.level == Level::Error).count();
    // Warnings are often consequences of errors (code skipped by recovery): show them
    // once the errors are fixed.
    if errors > 0 {
        list.retain(|d| d.level == Level::Error);
    }
    let warnings = list.len() - errors;
    Compilation { diagnostics: render_all(&list, sm, color), errors, warnings, c_source: if errors == 0 { c } else { None }, links, tests }
}

// ---------------------------------------------------------------------------
// C toolchain

pub struct BuildOptions {
    pub release: bool,
    pub target: Option<String>,
}

/// The C compiler command: `NEXUL_CC` (e.g. `clang`), or `zig cc`.
pub fn find_c_compiler() -> Result<Vec<String>, String> {
    if let Ok(cc) = std::env::var("NEXUL_CC") {
        let parts: Vec<String> = cc.split_whitespace().map(|s| s.to_string()).collect();
        if !parts.is_empty() {
            return Ok(parts);
        }
    }
    let mut candidates: Vec<PathBuf> = vec![PathBuf::from("zig")];
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(Path::new(&local).join("Microsoft").join("WinGet").join("Links").join("zig.exe"));
    }
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(Path::new(&home).join(".local").join("bin").join("zig"));
    }
    for c in candidates {
        if Command::new(&c).arg("version").output().map(|o| o.status.success()).unwrap_or(false) {
            return Ok(vec![c.to_string_lossy().to_string(), "cc".into()]);
        }
    }
    Err("no C toolchain found: install Zig (https://ziglang.org) or set NEXUL_CC to a C11 compiler such as `clang`".into())
}

fn target_is_windows(target: &Option<String>) -> bool {
    match target {
        Some(t) => t.contains("windows"),
        None => cfg!(windows),
    }
}

/// Compiles C source into an executable.
pub fn c_compile(c_source: &str, links: &[String], out: &Path, opts: &BuildOptions) -> Result<(), String> {
    let cc = find_c_compiler()?;
    let dir = out.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create `{}`: {}", dir.display(), e))?;
    let stem = out.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "out".into());
    let c_path = std::env::temp_dir().join(format!("nexul-{}-{}.c", stem, std::process::id()));
    std::fs::write(&c_path, c_source).map_err(|e| format!("cannot write `{}`: {}", c_path.display(), e))?;
    let mut cmd = Command::new(&cc[0]);
    cmd.args(&cc[1..]);
    cmd.arg("-std=c11").arg("-w").arg("-fno-strict-aliasing").arg("-fwrapv");
    if opts.release {
        cmd.arg("-O2");
    } else {
        cmd.arg("-O0").arg("-g");
    }
    if let Some(t) = &opts.target {
        cmd.arg("-target").arg(t);
    }
    cmd.arg(&c_path).arg("-o").arg(out);
    if target_is_windows(&opts.target) {
        cmd.arg("-lshell32");
    } else {
        cmd.arg("-lm");
    }
    for l in links {
        cmd.arg(format!("-l{}", l));
    }
    let output = cmd.output().map_err(|e| format!("cannot run the C compiler `{}`: {}", cc.join(" "), e))?;
    let _ = std::fs::remove_file(&c_path);
    if !output.status.success() {
        return Err(format!(
            "the C compiler failed (this is a bug in nxc, please report it):\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

pub fn exe_name(stem: &str, target: &Option<String>) -> String {
    if target_is_windows(target) {
        format!("{}.exe", stem)
    } else {
        stem.to_string()
    }
}

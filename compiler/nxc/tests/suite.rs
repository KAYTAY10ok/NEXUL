//! The NEXUL conformance suite (spec §10.4).
//!
//! - `tests/run/NAME.nx`: compiled and executed with the leak check enabled; stdout is
//!   compared with `NAME.out`; `NAME.exit` (optional) holds the expected exit code and
//!   `NAME.stderr` (optional) the expected standard error (for panics).
//! - `tests/ui/NAME.nx`: rejected (or warned about) by the compiler; the rendered
//!   diagnostics are compared with `NAME.stderr`.
//!
//! Set `NEXUL_BLESS=1` to (re)write the expectation files from the current results.

use nxc_driver::{build, compile_file, BuildOptions, CompileOptions, Mode, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn bless() -> bool {
    std::env::var_os("NEXUL_BLESS").is_some()
}

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
}

fn files(dir: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root().join(dir))
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "nx")).collect())
        .unwrap_or_default();
    if let Ok(filter) = std::env::var("NEXUL_TEST_FILTER") {
        v.retain(|p| p.to_string_lossy().contains(&filter));
    }
    v.sort();
    v
}

fn check_expect(path: &Path, actual: &str, failures: &mut Vec<String>, what: &str) {
    if bless() {
        if actual.is_empty() && !path.exists() {
            return;
        }
        std::fs::write(path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(path).map(|s| normalize(&s)).unwrap_or_default();
    if expected != actual {
        failures.push(format!(
            "{}: {} differs\n--- expected ---\n{}\n--- actual ---\n{}",
            path.display(),
            what,
            expected,
            actual
        ));
    }
}

/// The options for a test file: `// edition: 1` on one of its first lines selects the
/// syntax of edition 1 (ADR 0010), `// target: wasm32-wasi` the target of the analysis
/// (diagnostics that depend on the target, such as the width of `usize`).
fn opts_for(nx: &Path, mode: Mode) -> CompileOptions {
    let mut opts = CompileOptions::new(mode);
    let text = std::fs::read_to_string(nx).unwrap_or_default();
    if text.lines().take(3).any(|l| l.trim() == "// edition: 1") {
        opts.edition = nxc_syntax::Edition::V1;
    }
    if let Some(name) = text.lines().take(3).find_map(|l| l.trim().strip_prefix("// target:")) {
        opts.target = Target::from_name(name).unwrap_or_else(|e| panic!("{}: {}", nx.display(), e));
    }
    opts
}

/// Compares with an expectation file without ever rewriting it.
fn compare_expect(path: &Path, actual: &str, failures: &mut Vec<String>, what: &str) {
    let expected = std::fs::read_to_string(path).map(|s| normalize(&s)).unwrap_or_default();
    if expected != actual {
        failures.push(format!("{}: {} differs\n--- expected ---\n{}\n--- actual ---\n{}", path.display(), what, expected, actual));
    }
}

fn run_one(nx: &Path) -> Vec<String> {
    run_one_at(nx, false)
}

/// Runs a program of `tests/run`, built without optimization or, with `optimized`, at
/// `-O2` (contract K7.3: the guarantees hold at every optimization level). `@cfg` stays
/// `debug` in both cases, so the expected outputs are the same; only the unoptimized run
/// rewrites them under `NEXUL_BLESS`.
fn run_one_at(nx: &Path, optimized: bool) -> Vec<String> {
    let bless = bless() && !optimized;
    let mode = if optimized { " [-O2]" } else { "" };
    let mut failures = Vec::new();
    let c = compile_file(nx, &opts_for(nx, Mode::Program));
    if c.errors > 0 {
        return vec![format!("{}: compilation failed\n{}", nx.display(), c.diagnostics)];
    }
    let stem = nx.file_stem().unwrap().to_string_lossy().to_string();
    let dir = std::env::temp_dir().join(format!("nexul-suite-run-{}{}", std::process::id(), if optimized { "-O2" } else { "" }));
    let exe = dir.join(format!("{}{}", stem, if cfg!(windows) { ".exe" } else { "" }));
    if let Err(e) = build(c.output.as_ref().unwrap(), &exe, &BuildOptions::new(Target::host(), optimized)) {
        return vec![format!("{}: code generation failed\n{}", nx.display(), e)];
    }
    let stdin_path = nx.with_extension("in");
    let mut cmd = Command::new(&exe);
    cmd.env("NEXUL_TEST_ENV", "present").env_remove("NEXUL_DOES_NOT_EXIST").env("NEXUL_LEAK_CHECK", "1").arg("first-argument").current_dir(std::env::temp_dir());
    if stdin_path.exists() {
        cmd.stdin(std::fs::File::open(&stdin_path).unwrap());
    }
    let out = cmd.output().expect("run test program");
    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("pdb"));
    let stdout = normalize(&String::from_utf8_lossy(&out.stdout));
    let stderr = normalize(&String::from_utf8_lossy(&out.stderr));
    let code = out.status.code().unwrap_or(-1);
    if optimized {
        compare_expect(&nx.with_extension("out"), &stdout, &mut failures, "stdout [-O2]");
    } else {
        check_expect(&nx.with_extension("out"), &stdout, &mut failures, "stdout");
    }
    let exit_path = nx.with_extension("exit");
    let expected_code: i32 = std::fs::read_to_string(&exit_path).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    if bless {
        if code != 0 {
            std::fs::write(&exit_path, format!("{}\n", code)).unwrap();
        } else {
            let _ = std::fs::remove_file(&exit_path);
        }
    } else if code != expected_code {
        failures.push(format!("{}{}: exit code {} (expected {})\nstderr:\n{}", nx.display(), mode, code, expected_code, stderr));
    }
    let err_path = nx.with_extension("stderr");
    if bless {
        if !stderr.is_empty() {
            std::fs::write(&err_path, &stderr).unwrap();
        } else {
            let _ = std::fs::remove_file(&err_path);
        }
    } else if err_path.exists() || !stderr.is_empty() {
        if optimized {
            compare_expect(&err_path, &stderr, &mut failures, "stderr [-O2]");
        } else {
            check_expect(&err_path, &stderr, &mut failures, "stderr");
        }
    }
    if !c.diagnostics.is_empty() {
        failures.push(format!("{}: unexpected warnings\n{}", nx.display(), c.diagnostics));
    }
    failures
}

/// The targets a test program runs on: `// targets: windows, linux` on its first line
/// restricts it (programs calling a C library, for example).
fn runs_on(nx: &Path, target: &Target) -> bool {
    let text = std::fs::read_to_string(nx).unwrap_or_default();
    match text.lines().next().and_then(|l| l.strip_prefix("// targets:")) {
        Some(list) => list.split(',').any(|t| t.trim() == target.os.name()),
        None => true,
    }
}

/// Runs an executable of `target` from this machine: directly for the host, in WSL for
/// Linux on Windows, with Node.js for WebAssembly.
fn run_cross(target: &Target, exe: &Path, stdin: Option<&Path>) -> std::process::Output {
    let mut cmd = if target.os == nxc_driver::Os::Wasi {
        let mut c = Command::new("node");
        c.arg("--no-warnings").arg(root().join("tools").join("wasi-run.mjs")).arg(exe);
        c
    } else if *target == Target::host() {
        Command::new(exe)
    } else if cfg!(windows) && target.os == nxc_driver::Os::Linux {
        let p = std::fs::canonicalize(exe).unwrap().to_string_lossy().trim_start_matches(r"\\?\").replace('\\', "/");
        let drive = p[..1].to_ascii_lowercase();
        let mut c = Command::new("wsl");
        c.arg("-e").arg(format!("/mnt/{}{}", drive, &p[2..]));
        c.env("WSLENV", "NEXUL_LEAK_CHECK/u:NEXUL_TEST_ENV/u");
        c
    } else if target.os == nxc_driver::Os::None {
        return run_qemu(target, exe, stdin);
    } else {
        panic!("cannot run programs for `{}` on this machine", target.name);
    };
    cmd.env("NEXUL_TEST_ENV", "present").env_remove("NEXUL_DOES_NOT_EXIST").env("NEXUL_LEAK_CHECK", "1").arg("first-argument").current_dir(std::env::temp_dir());
    if let Some(i) = stdin {
        cmd.stdin(std::fs::File::open(i).unwrap());
    }
    cmd.output().expect("run test program")
}

/// Runs a bare-metal executable in QEMU (`NEXUL_QEMU` names its directory), with
/// semihosting for the console, the command line and the exit code. A program still
/// running after 60 seconds is stopped.
fn run_qemu(target: &Target, exe: &Path, stdin: Option<&Path>) -> std::process::Output {
    use nxc_driver::Arch;
    let (system, machine): (&str, &[&str]) = match target.arch {
        Arch::Arm => ("arm", &["-machine", "mps2-an386"]),
        Arch::Aarch64 => ("aarch64", &["-machine", "virt", "-cpu", "cortex-a53"]),
        Arch::Riscv32 => ("riscv32", &["-machine", "virt", "-bios", "none"]),
        _ => ("riscv64", &["-machine", "virt", "-bios", "none"]),
    };
    let name = format!("qemu-system-{}{}", system, std::env::consts::EXE_SUFFIX);
    let mut dirs: Vec<PathBuf> = std::env::var_os("NEXUL_QEMU").map(PathBuf::from).into_iter().collect();
    if cfg!(windows) {
        dirs.push(PathBuf::from(r"C:\Program Files\qemu"));
    }
    let program = dirs.iter().map(|d| d.join(&name)).find(|p| p.exists()).unwrap_or_else(|| PathBuf::from(&name));
    let mut cmd = Command::new(program);
    cmd.args(machine)
        .args(["-nographic", "-monitor", "none", "-serial", "none"])
        .arg("-semihosting-config")
        .arg("enable=on,target=native,chardev=con,arg=program,arg=first-argument")
        .args(["-chardev", "stdio,id=con,mux=off"])
        .arg("-kernel")
        .arg(exe)
        .current_dir(std::env::temp_dir());
    cmd.stdin(match stdin {
        Some(i) => std::process::Stdio::from(std::fs::File::open(i).unwrap()),
        None => std::process::Stdio::null(),
    });
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("run QEMU (set NEXUL_QEMU to its directory)");
    let mut out_pipe = child.stdout.take().unwrap();
    let mut err_pipe = child.stderr.take().unwrap();
    let out_reader = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = std::io::Read::read_to_end(&mut out_pipe, &mut v);
        v
    });
    let err_reader = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = std::io::Read::read_to_end(&mut err_pipe, &mut v);
        v
    });
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait().unwrap() {
            break st;
        }
        if start.elapsed() > std::time::Duration::from_secs(60) {
            let _ = child.kill();
            break child.wait().unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    std::process::Output { status, stdout: out_reader.join().unwrap(), stderr: err_reader.join().unwrap() }
}

/// The programs of `tests/run` on other targets: `NEXUL_TEST_TARGETS=x86_64-linux,wasm32-wasi`.
#[test]
fn run_programs_cross() {
    let Ok(list) = std::env::var("NEXUL_TEST_TARGETS") else { return };
    let paths = files("tests/run");
    let mut failures = Vec::new();
    for name in list.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let target = Target::from_name(name).unwrap();
        let dir = std::env::temp_dir().join(format!("nexul-suite-{}", target.name));
        let _ = std::fs::create_dir_all(&dir);
        let results = parallel(&paths, |nx| {
            if !runs_on(nx, &target) {
                return vec![];
            }
            let mut opts = opts_for(nx, Mode::Program);
            opts.target = target.clone();
            let c = compile_file(nx, &opts);
            if c.errors > 0 {
                return vec![format!("[{}] {}: compilation failed\n{}", target.name, nx.display(), c.diagnostics)];
            }
            let stem = nx.file_stem().unwrap().to_string_lossy().to_string();
            let exe = dir.join(target.exe_name(&stem));
            if let Err(e) = build(c.output.as_ref().unwrap(), &exe, &BuildOptions::new(target.clone(), false)) {
                return vec![format!("[{}] {}: code generation failed\n{}", target.name, nx.display(), e)];
            }
            let stdin = nx.with_extension("in");
            let out = run_cross(&target, &exe, if stdin.exists() { Some(&stdin) } else { None });
            let mut f = Vec::new();
            let stdout = normalize(&String::from_utf8_lossy(&out.stdout));
            let stderr = normalize(&String::from_utf8_lossy(&out.stderr));
            let expected = std::fs::read_to_string(nx.with_extension("out")).map(|s| normalize(&s)).unwrap_or_default();
            let expected_err = std::fs::read_to_string(nx.with_extension("stderr")).map(|s| normalize(&s)).unwrap_or_default();
            if stdout != expected {
                f.push(format!("[{}] {}: stdout differs\n--- expected ---\n{}\n--- actual ---\n{}", target.name, nx.display(), expected, stdout));
            }
            let code = out.status.code().unwrap_or(-1);
            let expected_code: i32 = std::fs::read_to_string(nx.with_extension("exit")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            if code != expected_code {
                f.push(format!("[{}] {}: exit code {} (expected {})\n{}", target.name, nx.display(), code, expected_code, stderr));
            }
            if stderr != expected_err {
                f.push(format!("[{}] {}: stderr differs\n--- expected ---\n{}\n--- actual ---\n{}", target.name, nx.display(), expected_err, stderr));
            }
            f
        });
        failures.extend(results);
    }
    if !failures.is_empty() {
        panic!("{} failure(s):\n\n{}", failures.len(), failures.join("\n\n"));
    }
}

fn parallel<F: Fn(&Path) -> Vec<String> + Sync>(paths: &[PathBuf], f: F) -> Vec<String> {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let chunks: Vec<Vec<PathBuf>> = (0..n).map(|i| paths.iter().skip(i).step_by(n).cloned().collect()).collect();
    let results: Vec<Vec<String>> = std::thread::scope(|s| {
        // The analyses are recursive: compile on a stack as large as that of `nxc`
        // (contract K6.3; `tests/ui/nesting_limit.nx` reaches 1000 levels).
        let handles: Vec<_> = chunks
            .iter()
            .map(|chunk| std::thread::Builder::new().stack_size(256 << 20).spawn_scoped(s, || chunk.iter().flat_map(|p| f(p)).collect::<Vec<String>>()).unwrap())
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    results.into_iter().flatten().collect()
}

#[test]
fn run_programs() {
    let target = Target::host();
    let paths: Vec<PathBuf> = files("tests/run").into_iter().filter(|p| runs_on(p, &target)).collect();
    assert!(!paths.is_empty(), "no tests found");
    let failures = parallel(&paths, run_one);
    if !failures.is_empty() {
        panic!("{} failure(s):\n\n{}", failures.len(), failures.join("\n\n"));
    }
}

/// The same programs built at `-O2`: an optimization must not change what a program does
/// nor remove a safety check (contract K7.3). Skipped while blessing the expectations.
#[test]
fn run_programs_optimized() {
    if bless() {
        return;
    }
    let target = Target::host();
    let paths: Vec<PathBuf> = files("tests/run").into_iter().filter(|p| runs_on(p, &target)).collect();
    assert!(!paths.is_empty(), "no tests found");
    let failures = parallel(&paths, |nx| run_one_at(nx, true));
    if !failures.is_empty() {
        panic!("{} failure(s):\n\n{}", failures.len(), failures.join("\n\n"));
    }
}

/// `nxc migrate` keeps the meaning of host-compatible programs: every single-file program
/// of `tests/run` written in 0.2 (no user module imported), migrated to edition 1, compiles
/// and prints the same output with the same exit code. A program that tests a rule of
/// edition 0.2 only says so in its first lines (`// no-migrate: reason`).
#[test]
fn migrated_programs_behave_the_same() {
    let paths: Vec<PathBuf> = files("tests/run")
        .into_iter()
        .filter(|p| {
            if !runs_on(p, &Target::host()) {
                return false;
            }
            let t = std::fs::read_to_string(p).unwrap_or_default();
            !t.lines().take(3).any(|l| l.trim() == "// edition: 1" || l.starts_with("// no-migrate: "))
                && !t.lines().any(|l| l.trim_start().starts_with("import ") && !l.trim_start().starts_with("import std"))
        })
        .collect();
    let failures = parallel(&paths, |nx| {
        let name = nx.file_name().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(nx).unwrap();
        let m = match nxc_driver::migrate::migrate(&name, &src) {
            Ok(m) => m,
            Err(e) => return vec![format!("{}: not migrated\n{}", nx.display(), e)],
        };
        if !m.manual.is_empty() {
            return vec![format!("{}: left to do by hand: {:?}", nx.display(), m.manual)];
        }
        let mut opts = CompileOptions::new(Mode::Program);
        opts.edition = nxc_syntax::Edition::V1;
        let c = nxc_driver::compile_source(&name, m.text.clone().into_bytes(), nx.parent().map(Path::to_path_buf), &opts);
        if c.errors > 0 {
            return vec![format!("{}: the migrated program does not compile\n{}\n--- migrated ---\n{}", nx.display(), c.diagnostics, m.text)];
        }
        let stem = nx.file_stem().unwrap().to_string_lossy().to_string();
        let dir = std::env::temp_dir().join(format!("nexul-suite-migrate-{}", std::process::id()));
        let exe = dir.join(format!("{}-ed1{}", stem, if cfg!(windows) { ".exe" } else { "" }));
        if let Err(e) = build(c.output.as_ref().unwrap(), &exe, &BuildOptions::new(Target::host(), false)) {
            return vec![format!("{}: code generation failed\n{}", nx.display(), e)];
        }
        let mut cmd = Command::new(&exe);
        cmd.env("NEXUL_TEST_ENV", "present").env_remove("NEXUL_DOES_NOT_EXIST").env("NEXUL_LEAK_CHECK", "1").arg("first-argument").current_dir(std::env::temp_dir());
        let stdin_path = nx.with_extension("in");
        if stdin_path.exists() {
            cmd.stdin(std::fs::File::open(&stdin_path).unwrap());
        }
        let out = cmd.output().expect("run migrated program");
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::remove_file(exe.with_extension("pdb"));
        let mut f = Vec::new();
        let stdout = normalize(&String::from_utf8_lossy(&out.stdout));
        let expected = std::fs::read_to_string(nx.with_extension("out")).map(|s| normalize(&s)).unwrap_or_default();
        if stdout != expected {
            f.push(format!("{}: migrated program prints differently\n--- expected ---\n{}\n--- actual ---\n{}", nx.display(), expected, stdout));
        }
        let code = out.status.code().unwrap_or(-1);
        let expected_code: i32 = std::fs::read_to_string(nx.with_extension("exit")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        if code != expected_code {
            f.push(format!("{}: migrated program exits with {} (expected {})", nx.display(), code, expected_code));
        }
        f
    });
    eprintln!("{} programs migrated and run", paths.len());
    if !failures.is_empty() {
        panic!("{} failure(s):\n\n{}", failures.len(), failures.join("\n\n"));
    }
}

/// The examples and the standard library migrate with nothing left to do by hand, and the
/// migrated examples compile in edition 1 without warnings.
#[test]
fn migrated_examples_and_library() {
    let mut failures = Vec::new();
    let mut lib = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map_or(false, |x| x == "nx") {
                out.push(p);
            }
        }
    }
    walk(&root().join("lib"), &mut lib);
    for p in lib.iter().chain(files("examples").iter()) {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        match nxc_driver::migrate::migrate(&name, &std::fs::read_to_string(p).unwrap()) {
            Err(e) => failures.push(format!("{}: not migrated\n{}", p.display(), e)),
            Ok(m) if !m.manual.is_empty() => failures.push(format!("{}: left to do by hand: {:?}", p.display(), m.manual)),
            Ok(m) if p.starts_with(root().join("examples")) => {
                let mut opts = CompileOptions::new(Mode::Check);
                opts.edition = nxc_syntax::Edition::V1;
                let c = nxc_driver::compile_source(&name, m.text.into_bytes(), p.parent().map(Path::to_path_buf), &opts);
                if c.errors > 0 || c.warnings > 0 {
                    failures.push(format!("{}: the migrated example does not compile cleanly\n{}", p.display(), c.diagnostics));
                }
            }
            Ok(_) => {}
        }
    }
    assert!(failures.is_empty(), "{} failure(s):\n\n{}", failures.len(), failures.join("\n\n"));
}

#[test]
fn ui_diagnostics() {
    let paths = files("tests/ui");
    let failures = parallel(&paths, |nx| {
        let mut f = Vec::new();
        let c = compile_file(nx, &opts_for(nx, Mode::Check));
        let actual = normalize(&c.diagnostics);
        if actual.is_empty() {
            f.push(format!("{}: expected diagnostics, got none", nx.display()));
        }
        check_expect(&nx.with_extension("stderr"), &actual, &mut f, "diagnostics");
        f
    });
    if !failures.is_empty() {
        panic!("{} failure(s):\n\n{}", failures.len(), failures.join("\n\n"));
    }
}

#[test]
fn examples_compile_cleanly() {
    let paths = files("examples");
    let failures = parallel(&paths, |nx| {
        let c = compile_file(nx, &opts_for(nx, Mode::Program));
        if c.errors > 0 || c.warnings > 0 {
            vec![format!("{}:\n{}", nx.display(), c.diagnostics)]
        } else {
            vec![]
        }
    });
    if !failures.is_empty() {
        panic!("{}", failures.join("\n\n"));
    }
}

/// Every complete example (a ```nexul block with `fn main`) of the documentation compiles
/// without any diagnostic; a block marked `// erreur E0xxx` must be rejected with that code.
#[test]
fn documentation_examples_compile() {
    let mut docs = vec![root().join("README.md"), root().join("docs").join("guide.md")];
    for e in std::fs::read_dir(root().join("docs").join("spec")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map_or(false, |x| x == "md") {
            docs.push(p);
        }
    }
    docs.sort();
    let mut failures = Vec::new();
    let mut checked = 0;
    for doc in docs {
        let text = normalize(&std::fs::read_to_string(&doc).unwrap());
        let mut rest = text.as_str();
        while let Some(start) = rest.find("```nexul\n") {
            let body_start = start + "```nexul\n".len();
            let Some(len) = rest[body_start..].find("```") else { break };
            let block = &rest[body_start..body_start + len];
            rest = &rest[body_start + len + 3..];
            if !block.contains("fn main") {
                continue;
            }
            checked += 1;
            let line = text[..text.len() - rest.len()].lines().count();
            let name = format!("{}:{}", doc.file_name().unwrap().to_string_lossy(), line);
            let c = nxc_driver::compile_source("doc.nx", block.as_bytes().to_vec(), None, &CompileOptions::new(Mode::Check));
            match block.find("// erreur E") {
                Some(i) => {
                    let code = &block[i + "// erreur ".len()..i + "// erreur ".len() + 5];
                    if !c.diagnostics.contains(&format!("error[{}]", code)) {
                        failures.push(format!("{}: expected error {}, got:\n{}", name, code, c.diagnostics));
                    }
                }
                None => {
                    if !c.diagnostics.is_empty() {
                        failures.push(format!("{}: the example does not compile cleanly:\n{}", name, c.diagnostics));
                    }
                }
            }
        }
    }
    assert!(checked >= 10, "only {} documentation examples found", checked);
    if !failures.is_empty() {
        panic!("{} failure(s):\n\n{}", failures.len(), failures.join("\n\n"));
    }
}

#[test]
fn every_code_is_explained() {
    // Every diagnostic code emitted by the compiler sources has an explanation.
    let mut missing = Vec::new();
    for dir in ["compiler/nxc_syntax/src", "compiler/nxc_sema/src", "compiler/nxc_sema/src/check", "compiler/nxc_nir/src", "compiler/nxc/src"] {
        for e in std::fs::read_dir(root().join(dir)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().map_or(true, |x| x != "rs") || p.file_name().unwrap() == "codes.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap();
            for (i, _) in text.match_indices("\"E0") .chain(text.match_indices("\"W0")) {
                let code = &text[i + 1..i + 6];
                if code.chars().skip(1).all(|c| c.is_ascii_digit()) && nxc_driver::codes::explain(code).is_none() && !missing.contains(&code.to_string()) {
                    missing.push(code.to_string());
                }
            }
        }
    }
    assert!(missing.is_empty(), "codes without explanation: {:?}", missing);
}

/// `docs/spec/diagnostics.md` is generated from the compiler's table of codes.
/// E0001 needs a source that is not valid UTF-8; the files of `tests/ui` stay valid UTF-8
/// because other tools read them as text (`studio/syntax` compares its lexer with the
/// compiler's on every one).
#[test]
fn invalid_utf8_is_reported() {
    let mut bytes = b"fn main() {
    let s = \"".to_vec();
    bytes.extend_from_slice(&[0xFF, 0xFE]);
    bytes.extend_from_slice(b"\"
}
");
    let c = nxc_driver::compile_source("bytes.nx", bytes, None, &CompileOptions::new(Mode::Check));
    assert!(c.diagnostics.contains("error[E0001]: source file is not valid UTF-8"), "{}", c.diagnostics);
    assert!(c.diagnostics.contains("bytes.nx:2:14"), "{}", c.diagnostics);
}

/// Every diagnostic code is produced by at least one test of `tests/ui` (contract K7.1),
/// except the internal error E0900, which no valid standard library can produce, and
/// E0001, tested by `invalid_utf8_is_reported`.
#[test]
fn every_code_has_a_ui_test() {
    let mut seen = std::collections::HashSet::new();
    for p in std::fs::read_dir(root().join("tests/ui")).unwrap().flatten().map(|e| e.path()) {
        if p.extension().map_or(false, |x| x == "stderr") {
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            for (i, _) in text.match_indices('[') {
                if let Some(code) = text.get(i + 1..i + 6) {
                    if (code.starts_with('E') || code.starts_with('W')) && code[1..].chars().all(|c| c.is_ascii_digit()) && text.get(i + 6..i + 7) == Some("]") {
                        seen.insert(code.to_string());
                    }
                }
            }
        }
    }
    let missing: Vec<&str> = nxc_driver::codes::all_codes().filter(|c| !["E0900", "E0001"].contains(c) && !seen.contains(*c)).collect();
    assert!(missing.is_empty(), "codes without a test in tests/ui: {:?}", missing);
}

#[test]
fn diagnostics_catalogue_is_up_to_date() {
    let path = root().join("docs").join("spec").join("diagnostics.md");
    let expected = nxc_driver::codes::catalogue_markdown();
    if bless() {
        std::fs::write(&path, &expected).unwrap();
        return;
    }
    let actual = normalize(&std::fs::read_to_string(&path).unwrap_or_default());
    assert!(actual == expected, "docs/spec/diagnostics.md is out of date: run the suite with NEXUL_BLESS=1");
}

/// Floating-point formatting, parsing and mathematical functions written in NEXUL,
/// compared with Python (see `tests/numerics/README.md`): `NEXUL_TEST_NUMERICS=1`.
#[test]
fn numerics_against_python() {
    if std::env::var_os("NEXUL_TEST_NUMERICS").is_none() {
        return;
    }
    let dir = root().join("tests").join("numerics");
    let work = std::env::temp_dir().join("nexul-numerics");
    let _ = std::fs::create_dir_all(&work);
    let exe_of = |name: &str| -> PathBuf {
        let mut opts = CompileOptions::new(Mode::Program);
        opts.release = true;
        let c = compile_file(&dir.join(format!("{}.nx", name)), &opts);
        assert!(c.errors == 0 && c.warnings == 0, "{}", c.diagnostics);
        let exe = work.join(Target::host().exe_name(name));
        build(c.output.as_ref().unwrap(), &exe, &BuildOptions::new(Target::host(), true)).unwrap();
        exe
    };
    let python = |args: &[&std::ffi::OsStr]| {
        let out = Command::new("python").arg("-I").args(args).output().expect("python is required for NEXUL_TEST_NUMERICS");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(out.status.success(), "{}{}", text, String::from_utf8_lossy(&out.stderr));
        text
    };
    // Formatting.
    let out = Command::new(exe_of("format")).output().unwrap();
    assert!(out.status.success(), "format failed: {}", String::from_utf8_lossy(&out.stderr));
    let txt = work.join("format.txt");
    std::fs::write(&txt, &out.stdout).unwrap();
    python(&[dir.join("format_verify.py").as_os_str(), txt.as_os_str()]);
    // Parsing.
    let (inp, expected, actual) = (work.join("parse_in.txt"), work.join("parse_expected.txt"), work.join("parse_actual.txt"));
    python(&[dir.join("parse_gen.py").as_os_str(), inp.as_os_str(), expected.as_os_str()]);
    let out = Command::new(exe_of("parse")).stdin(std::fs::File::open(&inp).unwrap()).output().unwrap();
    assert!(out.status.success(), "parse failed: {}", String::from_utf8_lossy(&out.stderr));
    std::fs::write(&actual, &out.stdout).unwrap();
    python(&[dir.join("parse_compare.py").as_os_str(), inp.as_os_str(), expected.as_os_str(), actual.as_os_str()]);
    // Mathematical functions.
    let out = Command::new(exe_of("math")).output().unwrap();
    assert!(out.status.success(), "math failed: {}", String::from_utf8_lossy(&out.stderr));
    let txt = work.join("math.txt");
    std::fs::write(&txt, &out.stdout).unwrap();
    python(&[dir.join("math_verify.py").as_os_str(), txt.as_os_str()]);
}

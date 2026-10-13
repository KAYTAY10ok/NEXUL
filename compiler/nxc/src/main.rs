//! `nxc`: the NEXUL compiler command line (spec §10.3).

use nxc_driver::{c_compile, codes, compile_file, exe_name, BuildOptions, Mode};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

const USAGE: &str = "\
nxc — the NEXUL compiler, version 0.1

usage:
    nxc run    <file.nx> [options] [-- program arguments]   compile and run
    nxc build  <file.nx> [options]                          compile an executable
    nxc check  <file.nx>                                    check without generating code
    nxc test   <file.nx> [options]                          run the @test functions
    nxc explain <code>                                      explain a diagnostic (e.g. E0402)
    nxc version

options:
    -o <path>            output file
    --release            optimize (all safety checks remain enabled)
    --target <triple>    cross-compile, e.g. x86_64-linux-gnu, aarch64-macos, wasm32-wasi
    --emit c             write the generated C instead of an executable
    --json               print the diagnostics as JSON on stdout (for editors)
";

struct Args {
    cmd: String,
    file: Option<PathBuf>,
    output: Option<PathBuf>,
    release: bool,
    target: Option<String>,
    emit_c: bool,
    json: bool,
    rest: Vec<String>,
}

fn parse_args() -> Args {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_else(|| "help".into());
    let mut a = Args { cmd, file: None, output: None, release: false, target: None, emit_c: false, json: false, rest: vec![] };
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--" => {
                a.rest.extend(it.by_ref());
                break;
            }
            "-o" => a.output = it.next().map(PathBuf::from),
            "--release" => a.release = true,
            "--json" => a.json = true,
            "--target" => a.target = it.next(),
            "--emit" => match it.next().as_deref() {
                Some("c") => a.emit_c = true,
                other => fail(&format!("unknown --emit kind `{}` (expected `c`)", other.unwrap_or(""))),
            },
            s if s.starts_with('-') => fail(&format!("unknown option `{}`\n\n{}", s, USAGE)),
            _ => {
                if a.file.is_none() {
                    a.file = Some(PathBuf::from(arg));
                } else {
                    a.rest.push(arg);
                }
            }
        }
    }
    a
}

fn fail(msg: &str) -> ! {
    eprintln!("error: {}", msg);
    exit(1);
}

fn color() -> bool {
    std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn main() {
    let args = parse_args();
    match args.cmd.as_str() {
        "version" | "--version" | "-V" => println!("nxc 0.1.0 (NEXUL 0.1)"),
        "help" | "--help" | "-h" => print!("{}", USAGE),
        "explain" => {
            let code = args.file.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            match codes::explain(&code) {
                Some(text) => println!("{}: {}", code.to_ascii_uppercase(), text),
                None => fail(&format!("unknown diagnostic code `{}`", code)),
            }
        }
        "check" | "build" | "run" | "test" => {
            let file = args.file.clone().unwrap_or_else(|| fail(&format!("missing input file\n\n{}", USAGE)));
            let mode = match args.cmd.as_str() {
                "check" => Mode::Check,
                "test" => Mode::Tests,
                _ => Mode::Program,
            };
            let c = compile_file(&file, mode, color() && !args.json);
            if args.json {
                // Machine-readable diagnostics on stdout (for editors); no code generation.
                println!("{}", c.json);
                exit(if c.errors > 0 { 1 } else { 0 });
            }
            eprint!("{}", c.diagnostics);
            if c.errors > 0 {
                exit(1);
            }
            if mode == Mode::Check {
                return;
            }
            let c_source = c.c_source.unwrap();
            let stem = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "out".into());
            if args.emit_c {
                let out = args.output.clone().unwrap_or_else(|| PathBuf::from(format!("{}.c", stem)));
                if let Err(e) = std::fs::write(&out, &c_source) {
                    fail(&format!("cannot write `{}`: {}", out.display(), e));
                }
                return;
            }
            let opts = BuildOptions { release: args.release, target: args.target.clone() };
            let out = match (&args.output, args.cmd.as_str()) {
                (Some(o), _) => o.clone(),
                (None, "build") => PathBuf::from(exe_name(&stem, &args.target)),
                (None, _) => std::env::temp_dir().join("nexul-run").join(exe_name(&format!("{}-{}", stem, std::process::id()), &args.target)),
            };
            if let Err(e) = c_compile(&c_source, &c.links, &out, &opts) {
                fail(&e);
            }
            match args.cmd.as_str() {
                "run" => {
                    let status = Command::new(&out).args(&args.rest).status();
                    let _ = std::fs::remove_file(&out);
                    match status {
                        Ok(s) => exit(s.code().unwrap_or(1)),
                        Err(e) => fail(&format!("cannot run `{}`: {}", out.display(), e)),
                    }
                }
                "test" => run_tests(&out, &c.tests),
                _ => {}
            }
        }
        other => fail(&format!("unknown command `{}`\n\n{}", other, USAGE)),
    }
}

fn run_tests(exe: &Path, tests: &[String]) {
    if tests.is_empty() {
        println!("no @test functions found");
        let _ = std::fs::remove_file(exe);
        return;
    }
    println!("running {} test{}", tests.len(), if tests.len() == 1 { "" } else { "s" });
    let mut failed = Vec::new();
    for (i, name) in tests.iter().enumerate() {
        let out = Command::new(exe).arg(i.to_string()).output();
        match out {
            Ok(o) if o.status.success() => println!("test {} ... ok", name),
            Ok(o) => {
                println!("test {} ... FAILED", name);
                failed.push((name.clone(), String::from_utf8_lossy(&o.stderr).to_string()));
            }
            Err(e) => fail(&format!("cannot run tests: {}", e)),
        }
    }
    let _ = std::fs::remove_file(exe);
    for (name, err) in &failed {
        println!("\n---- {} ----\n{}", name, err.trim_end());
    }
    println!(
        "\ntest result: {}. {} passed; {} failed",
        if failed.is_empty() { "ok" } else { "FAILED" },
        tests.len() - failed.len(),
        failed.len()
    );
    if !failed.is_empty() {
        exit(1);
    }
}

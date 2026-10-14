//! `nxc`: the NEXUL compiler command line (spec §10.3).

use nxc_driver::{build, codes, compile_file, exe_name, lib_name, optimized_ir, BuildOptions, CompileOptions, Mode, OutputKind, Target};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

const USAGE: &str = "\
nxc — the NEXUL compiler, version 0.2

usage:
    nxc run    <file.nx> [options] [-- program arguments]   compile and run
    nxc build  <file.nx> [options]                          compile an executable or a library
    nxc check  <file.nx> [options]                          check without generating code
    nxc test   <file.nx> [options]                          run the @test functions
    nxc explain <code>                                      explain a diagnostic (e.g. E0402)
    nxc targets                                             list the supported targets
    nxc version

options:
    -o <path>            output file
    --release            optimize (all safety checks remain enabled)
    -O<level>            optimization level: 0, 1, 2, 3, s (size) or z (smallest)
    --target <name>      cross-compile: x86_64-linux, aarch64-linux, wasm32-wasi,
                         thumbv7em-none... (see `nxc targets`)
    --lib                build a shared library (.dll, .so) of the @export functions
    --staticlib          build a static library (.lib, .a) of the @export functions
    --emit llvm-ir       write the LLVM IR produced by NEXUL (after optimization)
    --no-debug           no debug information
    --link-arg <arg>     pass an argument to the linker
    --json               print the diagnostics as JSON on stdout (for editors)
";

/// The WASI runner for Node.js (`tools/wasi-run.mjs`).
const WASI_RUNNER: &str = include_str!("../../../tools/wasi-run.mjs");

struct Args {
    cmd: String,
    file: Option<PathBuf>,
    output: Option<PathBuf>,
    release: bool,
    opt_level: Option<char>,
    target: Option<String>,
    emit_ir: bool,
    kind: OutputKind,
    no_debug: bool,
    link_args: Vec<String>,
    json: bool,
    rest: Vec<String>,
}

fn parse_args() -> Args {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_else(|| "help".into());
    let mut a = Args {
        cmd,
        file: None,
        output: None,
        release: false,
        opt_level: None,
        target: None,
        emit_ir: false,
        kind: OutputKind::Executable,
        no_debug: false,
        link_args: vec![],
        json: false,
        rest: vec![],
    };
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--" => {
                a.rest.extend(it.by_ref());
                break;
            }
            "-o" => a.output = it.next().map(PathBuf::from),
            "--release" => a.release = true,
            "--json" => a.json = true,
            "--lib" => a.kind = OutputKind::SharedLibrary,
            "--staticlib" => a.kind = OutputKind::StaticLibrary,
            "--no-debug" => a.no_debug = true,
            "--target" => a.target = it.next(),
            "--link-arg" => a.link_args.extend(it.next()),
            "--emit" => match it.next().as_deref() {
                Some("llvm-ir") | Some("ir") => a.emit_ir = true,
                other => fail(&format!("unknown --emit kind `{}` (expected `llvm-ir`)", other.unwrap_or(""))),
            },
            s if s.starts_with("-O") && s.len() == 3 && "0123sz".contains(&s[2..]) => {
                let l = s.as_bytes()[2] as char;
                a.opt_level = Some(l);
                if l != '0' {
                    a.release = true;
                }
            }
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
    // The analyses are recursive: give them a large stack (the main thread of a Windows
    // program only has 1 MiB).
    let worker = std::thread::Builder::new().stack_size(256 << 20).spawn(real_main).unwrap_or_else(|e| fail(&format!("cannot start the compiler: {}", e)));
    if worker.join().is_err() {
        exit(101);
    }
}

fn real_main() {
    let args = parse_args();
    match args.cmd.as_str() {
        "version" | "--version" | "-V" => println!("nxc {} (NEXUL {})", env!("CARGO_PKG_VERSION"), &env!("CARGO_PKG_VERSION")[..3]),
        "help" | "--help" | "-h" => print!("{}", USAGE),
        "targets" => {
            let host = Target::host();
            for name in Target::known_names() {
                let t = Target::from_name(name).unwrap();
                println!("{:<18} {:<28}{}", name, t.llvm_triple, if t == host { " (host)" } else { "" });
            }
        }
        "explain" => {
            let code = args.file.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            match codes::explain(&code) {
                Some(text) => println!("{}: {}", code.to_ascii_uppercase(), text),
                None => fail(&format!("unknown diagnostic code `{}`", code)),
            }
        }
        "check" | "build" | "run" | "test" => compile_command(&args),
        other => fail(&format!("unknown command `{}`\n\n{}", other, USAGE)),
    }
}

fn compile_command(args: &Args) {
    let file = args.file.clone().unwrap_or_else(|| fail(&format!("missing input file\n\n{}", USAGE)));
    let target = match &args.target {
        Some(t) => Target::from_name(t).unwrap_or_else(|e| fail(&e)),
        None => Target::host(),
    };
    let mode = match args.cmd.as_str() {
        "check" => Mode::Check,
        "test" => Mode::Tests,
        _ if args.kind != OutputKind::Executable => Mode::Library,
        _ => Mode::Program,
    };
    let mut copts = CompileOptions::new(mode);
    copts.color = color() && !args.json;
    copts.target = target.clone();
    copts.release = args.release;
    copts.debug_info = !args.no_debug && !args.release;
    let c = compile_file(&file, &copts);
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
    let output = c.output.unwrap();
    let stem = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "out".into());
    let mut bopts = BuildOptions::new(target.clone(), args.release);
    bopts.opt_level = args.opt_level;
    bopts.debug_info = copts.debug_info;
    bopts.kind = args.kind;
    bopts.link_args = args.link_args.clone();
    if args.emit_ir {
        let out = args.output.clone().unwrap_or_else(|| PathBuf::from(format!("{}.ll", stem)));
        let text = if args.release { optimized_ir(&output, &bopts).unwrap_or_else(|e| fail(&e)) } else { output.ir.clone() };
        if let Err(e) = std::fs::write(&out, text) {
            fail(&format!("cannot write `{}`: {}", out.display(), e));
        }
        return;
    }
    let out = match (&args.output, args.cmd.as_str()) {
        (Some(o), _) => o.clone(),
        (None, "build") if args.kind != OutputKind::Executable => PathBuf::from(lib_name(&stem, &target, args.kind)),
        (None, "build") => PathBuf::from(exe_name(&stem, &target)),
        (None, _) => std::env::temp_dir().join("nexul-run").join(exe_name(&format!("{}-{}", stem, std::process::id()), &target)),
    };
    if let Err(e) = build(&output, &out, &bopts) {
        fail(&e);
    }
    match args.cmd.as_str() {
        "run" => {
            let mut cmd = if target.os == nxc_driver::Os::Wasi {
                // WebAssembly programs run with the WASI implementation of Node.js.
                let runner = std::env::temp_dir().join("nexul-wasi-run.mjs");
                if std::fs::write(&runner, WASI_RUNNER).is_err() {
                    fail("cannot write the WebAssembly runner");
                }
                let mut c = Command::new("node");
                c.arg("--no-warnings").arg(runner).arg(&out);
                c
            } else if target != Target::host() {
                fail(&format!("cannot run a program built for `{}` on this machine; use `nxc build`", target.name));
            } else {
                Command::new(&out)
            };
            let status = cmd.args(&args.rest).status();
            let _ = std::fs::remove_file(&out);
            let _ = std::fs::remove_file(out.with_extension("pdb"));
            match status {
                Ok(s) => exit(s.code().unwrap_or(1)),
                Err(e) => fail(&format!("cannot run `{}`: {}", out.display(), e)),
            }
        }
        "test" => run_tests(&out, &c.tests),
        _ => {}
    }
}

fn run_tests(exe: &Path, tests: &[String]) {
    let cleanup = || {
        let _ = std::fs::remove_file(exe);
        let _ = std::fs::remove_file(exe.with_extension("pdb"));
    };
    if tests.is_empty() {
        println!("no @test functions found");
        cleanup();
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
    cleanup();
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

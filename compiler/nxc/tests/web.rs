use nxc_driver::{build, compile_file, BuildOptions, CompileOptions, Mode, OutputKind, Target};
use std::{path::{Path, PathBuf}, process::Command};

#[test]
#[ignore = "requires LLVM, wasm-ld and Node: cargo test -p nxc --test web -- --ignored"]
fn browser_abi() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let work = root.join("target/web-tests");
    std::fs::create_dir_all(&work).unwrap();
    let target = Target::from_name("wasm32-web").unwrap();
    let compile = |source: &str, name: &str, library: bool, release: bool| -> PathBuf {
        let mut opts = CompileOptions::new(if library { Mode::Library } else { Mode::Program });
        opts.target = target.clone();
        if std::fs::read_to_string(root.join(source))
            .unwrap()
            .lines()
            .take(3)
            .any(|line| line.trim() == "// edition: 1")
        {
            opts.edition = nxc_syntax::Edition::V1;
        }
        opts.release = release;
        opts.debug_info = !release;
        let compilation = compile_file(&root.join(source), &opts);
        assert_eq!(compilation.errors + compilation.warnings, 0, "{}", compilation.diagnostics);
        let out = work.join(format!("{name}-{release}.wasm"));
        let mut build_opts = BuildOptions::new(target.clone(), release);
        if library { build_opts.kind = OutputKind::SharedLibrary; }
        build(compilation.output.as_ref().unwrap(), &out, &build_opts).unwrap();
        out
    };
    for release in [false, true] {
        let example = compile("examples/web/calcul.nx", "example", true, release);
        let library = compile("tests/web/interop.nx", "library", true, release);
        let distinct_library = compile("tests/web/interop_distinct.nx", "distinct-library", true, release);
        let program = compile("tests/web/interop.nx", "program", false, release);
        let result = Command::new("node").arg(root.join("tests/web/interop.mjs"))
            .args([example, library, program]).output().expect("Node is required");
        assert!(result.status.success(), "{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
        println!("{}", String::from_utf8_lossy(&result.stdout));

        let result = Command::new("node").arg(root.join("tests/web/interop_distinct.mjs"))
            .arg(distinct_library).output().expect("Node is required");
        assert!(result.status.success(), "{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
        println!("{}", String::from_utf8_lossy(&result.stdout));
    }
}

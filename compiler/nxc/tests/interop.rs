//! C and NEXUL calling each other (roadmap 0.6.0): `tests/interop/main.c`, compiled by
//! the C compiler of the system, calls the `@export` functions of `tests/interop/lib.nx`,
//! which call back into C (a function of the program, a function pointer it passes).
//! Linked once with the static library (`--staticlib`) and once with the shared one
//! (`--lib`).
//!
//! Without a C compiler (`cc`, or `clang` on Windows), the test says so and passes,
//! unless `NEXUL_REQUIRE_CC=1` (set by the CI).

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn c_compiler() -> Option<String> {
    let names: &[&str] = if cfg!(windows) { &["clang", "cc"] } else { &["cc", "clang"] };
    names.iter().find(|n| Command::new(n).arg("--version").output().map_or(false, |o| o.status.success())).map(|n| n.to_string())
}

fn run(cmd: &mut Command, what: &str) -> String {
    let out = cmd.output().unwrap_or_else(|e| panic!("{}: {}", what, e));
    assert!(out.status.success(), "{} failed ({}):\n{}{}", what, out.status, String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}

#[test]
fn c_calls_nexul_and_back() {
    let Some(cc) = c_compiler() else {
        assert!(std::env::var_os("NEXUL_REQUIRE_CC").is_none(), "NEXUL_REQUIRE_CC is set but no C compiler was found");
        eprintln!("c_calls_nexul_and_back: no C compiler (cc, clang), not tested");
        return;
    };
    let src = root().join("tests").join("interop");
    let expected = std::fs::read_to_string(src.join("expected.out")).unwrap();
    let dir = std::env::temp_dir().join(format!("nxc-interop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for kind in ["--staticlib", "--lib"] {
        let d = dir.join(kind.trim_start_matches('-'));
        std::fs::create_dir_all(&d).unwrap();
        let (lib, exe) = if cfg!(windows) {
            (d.join("nxlib.lib"), d.join("main.exe"))
        } else if cfg!(target_os = "macos") {
            (d.join(if kind == "--lib" { "libnxlib.dylib" } else { "libnxlib.a" }), d.join("main"))
        } else {
            (d.join(if kind == "--lib" { "libnxlib.so" } else { "libnxlib.a" }), d.join("main"))
        };
        let out_lib = if cfg!(windows) && kind == "--lib" { d.join("nxlib.dll") } else { lib.clone() };
        run(Command::new(env!("CARGO_BIN_EXE_nxc")).arg("build").arg(kind).arg(src.join("lib.nx")).arg("-o").arg(&out_lib), "nxc build");
        let mut c = Command::new(&cc);
        c.arg(src.join("main.c")).arg("-o").arg(&exe);
        if cfg!(windows) {
            // The import library of the DLL, or the static library, has the same name.
            c.arg(&lib);
        } else {
            // Each directory holds one kind of library only.
            c.arg(format!("-L{}", d.display())).arg("-lnxlib");
            if cfg!(target_os = "macos") {
                // The dylib is named `@rpath/libnxlib.dylib`.
                c.arg(format!("-Wl,-rpath,{}", d.display()));
            }
        }
        run(&mut c, &format!("{} (link with {})", cc, kind));
        let mut program = Command::new(&exe);
        program.current_dir(&d).env("LD_LIBRARY_PATH", &d).env("DYLD_LIBRARY_PATH", &d).env("PATH", {
            let mut p = std::ffi::OsString::from(&d);
            p.push(if cfg!(windows) { ";" } else { ":" });
            p.push(std::env::var_os("PATH").unwrap_or_default());
            p
        });
        let got = run(&mut program, &format!("the C program ({})", kind));
        assert_eq!(got, expected, "output of the C program linked with {}", kind);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

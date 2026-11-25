//! C and NEXUL calling each other (roadmap 0.6.0): `tests/interop/main.c`, compiled by
//! the C compiler of the system, calls the `@export` functions of `tests/interop/lib.nx`,
//! which call back into C through the function pointers it passes.
//! Linked once with the static library (`--staticlib`) and once with the shared one
//! (`--lib`). `main.c` includes the header that `nxc --header` writes, compiled with
//! `-Wall -Werror`.
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
        run(Command::new(env!("CARGO_BIN_EXE_nxc")).arg("build").arg(kind).arg(src.join("lib.nx")).arg("-o").arg(&out_lib).arg("--header").arg(d.join("nxlib.h")), "nxc build");
        let mut c = Command::new(&cc);
        // The header that nxc wrote declares the functions: C sees their real types.
        c.arg(src.join("main.c")).arg("-o").arg(&exe).arg(format!("-I{}", d.display())).arg("-Wall").arg("-Werror");
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

/// The header of `nxc --header` (spec §8.4): nested `@repr("C")` structures in the order
/// C needs, arrays, pointers, callbacks and `char`; compiled by C with `-Wall -Werror`
/// when a C compiler is there.
#[test]
fn the_header_declares_structures_and_callbacks() {
    let dir = std::env::temp_dir().join(format!("nxc-header-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("shapes.nx");
    std::fs::write(
        &src,
        "// edition: 1\n@repr(\"C\")\npub type Out {\n    pub In inner\n    pub Ptr[In] next\n    pub fn i32(i32) cb\n}\n@repr(\"C\")\npub type In {\n    pub [u8; 4] tag\n    pub f64 v\n}\n@export\nfn Out nx_wrap(owned In i, Ptr[()] raw, char c, usize n) {\n    let _ = raw\n    let _ = c\n    let _ = n\n    return Out(inner: i, next: Ptr.null(), cb: twice)\n}\nfn i32 twice(i32 x) {\n    return 2 * x\n}\n",
    )
    .unwrap();
    let header = dir.join("shapes.h");
    run(
        Command::new(env!("CARGO_BIN_EXE_nxc")).arg("build").arg("--staticlib").arg(&src).arg("-o").arg(dir.join(if cfg!(windows) { "shapes.lib" } else { "libshapes.a" })).arg("--header").arg(&header),
        "nxc build",
    );
    let h = std::fs::read_to_string(&header).unwrap();
    let inner = h.find("typedef struct In {\n    uint8_t tag[4];\n    double v;\n} In;").expect(&h);
    let outer = h.find("typedef struct Out {\n    In inner;\n    In *next;\n    int32_t (*cb)(int32_t);\n} Out;").expect(&h);
    assert!(inner < outer, "{h}");
    assert!(h.contains("Out nx_wrap(In i, void *raw, uint32_t c, size_t n);"), "{h}");
    assert!(h.contains("#ifndef SHAPES_H") && h.contains("extern \"C\""), "{h}");
    if let Some(cc) = c_compiler() {
        std::fs::write(dir.join("use.c"), "#include \"shapes.h\"\nint main(void) { return sizeof(Out) > 0 ? 0 : 1; }\n").unwrap();
        run(Command::new(&cc).arg("-Wall").arg("-Werror").arg("-fsyntax-only").arg(format!("-I{}", dir.display())).arg(dir.join("use.c")), "C compiler on the header");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

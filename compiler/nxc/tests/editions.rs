//! Each file is read in its own edition (ADR 0010): `// edition: X` on its first lines,
//! else the `edition` of the `nexul.toml` of its package, else 0.2 — also for imported
//! modules and dependencies, with or without a byte order mark.

use std::path::{Path, PathBuf};
use std::process::Command;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let d = std::env::temp_dir().join(format!("nexul-editions-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Dir(d)
    }

    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `nxc run <file>`: (exit code, standard output, standard error).
fn run(file: &Path) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_nxc")).arg("run").arg(file).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"), String::from_utf8_lossy(&out.stderr).to_string())
}

fn runs(file: &Path, expected: &str) {
    let (code, out, err) = run(file);
    assert_eq!((code, out.as_str()), (0, expected), "{}\n{}", file.display(), err);
}

#[test]
fn a_module_declares_its_own_edition() {
    let d = Dir::new("mods");
    let main = d.write("main.nx", "// edition: 1\nimport calcul\nfn main() { println(\"{calcul.carre(4)}\") }\n");
    d.write("calcul.nx", "// edition: 0.2\npub fn carre(x: i32) -> i32 { return x * x }\n");
    runs(&main, "16\n");
    // The other way round.
    let main = d.write("main2.nx", "import calcul1\nfn main() { println(\"{calcul1.carre(3)}\") }\n");
    d.write("calcul1.nx", "// edition: 1\npub fn i32 carre(i32 x) = x * x\n");
    runs(&main, "9\n");
}

#[test]
fn dependencies_keep_the_edition_of_their_package() {
    let d = Dir::new("deps");
    d.write("lib02/nexul.toml", "[package]\nname = \"lib02\"\n");
    d.write("lib02/src/lib.nx", "pub fn carre(x: i32) -> i32 { return x * x }\n");
    d.write("lib1/nexul.toml", "[package]\nname = \"lib1\"\nedition = \"1\"\n");
    d.write("lib1/src/lib.nx", "pub fn i32 cube(i32 x) = x * x * x\n");
    // An edition-1 application uses a 0.2 library and an edition-1 one.
    d.write("app/nexul.toml", "[package]\nname = \"app\"\nedition = \"1\"\n\n[dependencies]\nlib02 = { path = \"../lib02\" }\nlib1 = { path = \"../lib1\" }\n");
    let app = d.write("app/src/main.nx", "import lib02\nimport lib1\nfn main() {\n    let i32 x = lib02.carre(5)\n    println(\"{x} {lib1.cube(2)}\")\n}\n");
    runs(&app, "25 8\n");
    // A 0.2 application uses the edition-1 library; its manifest starts with a BOM.
    d.write("app02/nexul.toml", "\u{feff}[package]\nname = \"app02\"\n\n[dependencies]\nlib1 = { path = \"../lib1\" }\n");
    let app02 = d.write("app02/src/main.nx", "import lib1\nfn main() { println(\"{lib1.cube(3)}\") }\n");
    runs(&app02, "27\n");
}

#[test]
fn the_header_is_read_after_a_byte_order_mark() {
    let d = Dir::new("bom");
    let f = d.write("b.nx", "\u{feff}// edition: 1\r\nfn i32 carre(i32 x) = x * x\r\nfn main() { println(\"{carre(7)}\") }\r\n");
    runs(&f, "49\n");
}

#[test]
fn tests_that_expect_a_panic() {
    let d = Dir::new("panic");
    let f = d.write(
        "t.nx",
        "// edition: 1\ntest \"une addition\" {\n    assert(1 + 1 == 2)\n}\n\ntest \"un indice hors limites\" expects panic {\n    let Vec[i32] v = Vec.new()\n    println(\"{v[3]}\")\n}\n\ntest \"rien ne casse\" expects panic {\n    assert(true)\n}\n",
    );
    let out = Command::new(env!("CARGO_BIN_EXE_nxc")).arg("test").arg(&f).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    assert_eq!(out.status.code(), Some(1), "{}", text);
    assert!(text.contains("test t.une_addition ... ok\n"), "{}", text);
    assert!(text.contains("test t.un_indice_hors_limites ... ok (panicked as expected)"), "{}", text);
    assert!(text.contains("test t.rien_ne_casse ... FAILED"), "{}", text);
    assert!(text.contains("expected to panic"), "{}", text);
    assert!(text.contains("2 passed; 1 failed"), "{}", text);
}

#[test]
fn a_dependency_with_an_unknown_edition_is_reported() {
    let d = Dir::new("bad");
    d.write("bad/nexul.toml", "[package]\nname = \"bad\"\nedition = \"3\"\n");
    d.write("bad/src/lib.nx", "pub fn f() -> i32 { return 1 }\n");
    d.write("app/nexul.toml", "[package]\nname = \"app\"\n\n[dependencies]\nbad = { path = \"../bad\" }\n");
    let app = d.write("app/src/main.nx", "import bad\nfn main() { println(\"{bad.f()}\") }\n");
    let (code, _, err) = run(&app);
    assert_ne!(code, 0);
    assert!(err.contains("dependency `bad`") && err.contains("unknown edition `3`"), "{}", err);
}

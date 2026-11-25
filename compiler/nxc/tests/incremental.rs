//! The incremental build (ADR 0016), on Linux x86-64 and AArch64 at -O0:
//!
//! - `a_changed_body_recompiles_its_unit_only`: after a change to one function body, one
//!   unit is compiled again; the program is the one a monolithic build gives.
//! - `a_new_function_does_not_rename_the_others`: adding a function to the program
//!   leaves the units it does not touch as they were.
//! - `a_broken_object_is_compiled_again`: a truncated or overwritten object of the cache
//!   is not used.
//! - `two_builds_at_once_share_the_cache`: builds that run at the same time all succeed.
//!
//! The suite (`tests/suite.rs`) runs every program of `tests/run` through this build too.

use std::path::{Path, PathBuf};
use std::process::Command;

fn enabled() -> bool {
    cfg!(target_os = "linux") && cfg!(any(target_arch = "x86_64", target_arch = "aarch64"))
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nxc-incremental-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The program of `benchmarks/incremental/gen.py`, written here: `n` functions, the
/// body of `g<n/2>` changed by `variant`, and `extra` functions added at the start.
fn program(n: usize, variant: usize, extra: usize) -> String {
    let mut s = String::from("// edition: 1\n");
    for i in 0..extra {
        s.push_str(&format!("fn i64 added{i}(i64 x) {{\n    return x + {i}\n}}\n\n"));
    }
    for i in 0..n {
        let k = 7 + if i == n / 2 { variant } else { 0 };
        let prev = if i > 0 { format!("g{}(x + 1)", i - 1) } else { "x".into() };
        s.push_str(&format!(
            "fn i64 g{i}(i64 x) {{\n    var Vec[i64] v = Vec.new()\n    var i64 j = 0\n    while j < {k} {{\n        v.push(x * j + {i})\n        j += 1\n    }}\n    var i64 s = 0\n    for e in v {{\n        s += e % {m}\n    }}\n    let name = \"g{i}:{{s}}\"\n    if name.len() > 100 {{\n        return 0\n    }}\n    if x > {i} {{\n        return s\n    }}\n    return s + {prev}\n}}\n\n",
            m = i + 3
        ));
    }
    let extra_call = if extra > 0 { " + added0(1)" } else { "" };
    s.push_str(&format!("fn main() {{\n    println(\"{{g{}(0){}}}\")\n}}\n", n - 1, extra_call));
    s
}

struct Built {
    units: usize,
    compiled: usize,
    out: String,
}

fn build(dir: &Path, cache: &Path, src: &str, incremental: bool) -> Built {
    let file = dir.join("p.nx");
    std::fs::write(&file, src).unwrap();
    let exe = dir.join("p");
    let r = Command::new(env!("CARGO_BIN_EXE_nxc"))
        .arg("build")
        .arg(&file)
        .arg("-o")
        .arg(&exe)
        .env("NEXUL_CACHE_DIR", cache)
        .env("NEXUL_INCREMENTAL_REPORT", "1")
        .env("NEXUL_INCREMENTAL", if incremental { "1" } else { "0" })
        .env_remove("NEXUL_NO_CACHE")
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&r.stderr).to_string();
    assert!(r.status.success(), "{err}");
    let (mut units, mut compiled) = (0, 0);
    for line in err.lines() {
        if let Some(rest) = line.strip_prefix("incremental: ") {
            let w: Vec<&str> = rest.split_whitespace().collect();
            units = w[0].parse().unwrap();
            compiled = w[2].parse().unwrap();
        }
    }
    let run = Command::new(&exe).output().unwrap();
    assert!(run.status.success());
    Built { units, compiled, out: String::from_utf8_lossy(&run.stdout).to_string() }
}

#[test]
fn a_changed_body_recompiles_its_unit_only() {
    if !enabled() {
        return;
    }
    let d = scratch("body");
    let cache = d.join("cache");
    let first = build(&d, &cache, &program(300, 0, 0), true);
    assert!(first.units > 2, "{} units", first.units);
    assert_eq!(first.compiled, first.units);
    let second = build(&d, &cache, &program(300, 1, 0), true);
    assert_eq!(second.units, first.units);
    assert_eq!(second.compiled, 1, "one function changed");
    let mono = build(&d, &d.join("cache-mono"), &program(300, 1, 0), false);
    assert_eq!(second.out, mono.out);
    assert_ne!(second.out, first.out);
}

#[test]
fn a_new_function_does_not_rename_the_others() {
    if !enabled() {
        return;
    }
    let d = scratch("added");
    let cache = d.join("cache");
    let first = build(&d, &cache, &program(300, 0, 0), true);
    let second = build(&d, &cache, &program(300, 0, 1), true);
    // The unit of the new function and the common unit (it calls it from `main`), and
    // the units of the functions whose line moved (their debug information).
    assert!(second.compiled < first.units, "{} of {} compiled", second.compiled, first.units);
    let mono = build(&d, &d.join("cache-mono"), &program(300, 0, 1), false);
    assert_eq!(second.out, mono.out);
}

#[test]
fn a_broken_object_is_compiled_again() {
    if !enabled() {
        return;
    }
    let d = scratch("broken");
    let cache = d.join("cache");
    let src = program(100, 0, 0);
    let first = build(&d, &cache, &src, true);
    let objects: Vec<PathBuf> = std::fs::read_dir(cache.join("objects").join("v1")).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(objects.len(), first.units);
    std::fs::write(&objects[0], b"").unwrap();
    std::fs::write(&objects[1], b"not an object, but long enough to pass for one if only the size were checked").unwrap();
    // Without the cache of whole executables, which would answer first.
    for e in std::fs::read_dir(&cache).unwrap().flatten() {
        if e.file_name().to_string_lossy().starts_with("nx-") {
            std::fs::remove_file(e.path()).unwrap();
        }
    }
    let again = build(&d, &cache, &src, true);
    assert_eq!(again.compiled, 2);
    assert_eq!(again.out, first.out);
}

#[test]
fn two_builds_at_once_share_the_cache() {
    if !enabled() {
        return;
    }
    let d = scratch("parallel");
    let cache = d.join("cache");
    let src = program(200, 0, 0);
    let outs: Vec<String> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..3)
            .map(|i| {
                let dir = d.join(format!("b{i}"));
                std::fs::create_dir_all(&dir).unwrap();
                let (cache, src) = (&cache, &src);
                s.spawn(move || build(&dir, cache, src, true).out)
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert!(outs.iter().all(|o| o == &outs[0]));
    let _ = root();
}

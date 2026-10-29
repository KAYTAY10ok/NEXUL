//! `@repr("C")` structures passed by value between NEXUL and C (spec §8.2): the calling
//! convention of every target, checked two ways on 30 structure shapes.
//!
//! - `signatures_match_clang`: the LLVM signatures `nxc` gives the C functions are those
//!   clang gives the same C declarations, on every target (no execution needed).
//! - `c_and_nexul_exchange_structures`: a NEXUL program and C code compiled by clang pass
//!   structures to each other, by value and as results, with registers exhausted and
//!   mixed with scalars; run on this machine, and on the targets of `NEXUL_TEST_TARGETS`
//!   (QEMU, Node), as `tests/run` is.
//!
//! Without clang the tests say so and pass, unless `NEXUL_REQUIRE_CC=1` (set by the CI).

use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy)]
enum Sc {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    F32,
    F64,
    Ptr,
}

impl Sc {
    fn c(self) -> &'static str {
        match self {
            Sc::I8 => "int8_t",
            Sc::I16 => "int16_t",
            Sc::I32 => "int32_t",
            Sc::I64 => "int64_t",
            Sc::U8 => "uint8_t",
            Sc::U16 => "uint16_t",
            Sc::F32 => "float",
            Sc::F64 => "double",
            Sc::Ptr => "void *",
        }
    }

    fn nx(self) -> &'static str {
        match self {
            Sc::I8 => "i8",
            Sc::I16 => "i16",
            Sc::I32 => "i32",
            Sc::I64 => "i64",
            Sc::U8 => "u8",
            Sc::U16 => "u16",
            Sc::F32 => "f32",
            Sc::F64 => "f64",
            Sc::Ptr => "Ptr[u8]",
        }
    }

    /// The value of leaf `j` for `seed`, in C.
    fn c_value(self, j: usize) -> String {
        match self {
            Sc::F32 | Sc::F64 => format!("(({})seed + {}.5)", self.c(), j),
            Sc::Ptr => format!("((void *)(uintptr_t)((seed + {j}) * 16))"),
            _ => format!("(({})(seed + {}))", self.c(), j * 3 + 1),
        }
    }

    /// The same value, in NEXUL.
    fn nx_value(self, j: usize) -> String {
        match self {
            Sc::F32 | Sc::F64 => format!("(seed as {} + {}.5)", self.nx(), j),
            Sc::Ptr => format!("unsafe {{ Ptr[u8].from_addr(((seed + {j}) * 16) as usize) }}"),
            Sc::I32 => format!("(seed + {})", j * 3 + 1),
            _ => format!("((seed + {}) as {})", j * 3 + 1, self.nx()),
        }
    }

    fn nx_eq(self, place: &str, j: usize) -> String {
        match self {
            Sc::Ptr => format!("{place}.addr() == {}.addr()", self.nx_value(j)),
            _ => format!("{place} == {}", self.nx_value(j)),
        }
    }
}

#[derive(Clone, Copy)]
enum F {
    S(Sc),
    A(Sc, usize),
    /// A nested structure: shape 9, `{ float, float }`.
    N9,
}

fn shapes() -> Vec<Vec<F>> {
    use F::*;
    use Sc::*;
    vec![
        vec![S(I8)],
        vec![S(I16), S(I8)],
        vec![S(I32)],
        vec![S(I32), S(I32)],
        vec![S(I32), S(I32), S(I32)],
        vec![S(I64), S(I64)],
        vec![S(I64), S(I64), S(I64)],
        vec![S(F32)],
        vec![S(F32), S(F32)],
        vec![S(F32), S(F32), S(F32)],
        vec![A(F32, 4)],
        vec![A(F32, 5)],
        vec![S(F64)],
        vec![S(F64), S(F64)],
        vec![A(F64, 4)],
        vec![S(F64), S(I64)],
        vec![S(I32), S(F32)],
        vec![S(I8), S(F64)],
        vec![S(F32), S(I64)],
        vec![S(U8), S(U8), S(U8)],
        vec![S(U16), S(U16), S(U16)],
        vec![A(I32, 5)],
        vec![N9, S(F32)],
        vec![S(Ptr), S(I32)],
        vec![A(U8, 7)],
        vec![A(F32, 2), S(F64)],
        vec![S(F64), S(F32)],
        vec![S(I64), S(F64)],
        vec![A(U8, 17)],
        vec![S(I16), S(F32)],
    ]
}

/// The leaves of a shape, in order: (C access, NEXUL access, scalar).
fn leaves(fields: &[F]) -> Vec<(String, String, Sc)> {
    let mut out = Vec::new();
    for (i, f) in fields.iter().enumerate() {
        match *f {
            F::S(s) => out.push((format!("f{i}"), format!("f{i}"), s)),
            F::A(s, n) => {
                for k in 0..n {
                    out.push((format!("f{i}[{k}]"), format!("f{i}[{k}]"), s));
                }
            }
            F::N9 => {
                out.push((format!("f{i}.f0"), format!("f{i}.f0"), Sc::F32));
                out.push((format!("f{i}.f1"), format!("f{i}.f1"), Sc::F32));
            }
        }
    }
    out
}

/// The C side: each shape `S<k>`, and functions that make, check and return it.
fn c_source(shapes: &[Vec<F>]) -> String {
    let mut c = String::from("#include <stdint.h>\n\n");
    for (k, fields) in shapes.iter().enumerate() {
        let k = k + 1;
        let _ = write!(c, "typedef struct {{");
        for (i, f) in fields.iter().enumerate() {
            match *f {
                F::S(s) => {
                    let _ = write!(c, " {} f{i};", s.c());
                }
                F::A(s, n) => {
                    let _ = write!(c, " {} f{i}[{n}];", s.c());
                }
                F::N9 => {
                    let _ = write!(c, " S9 f{i};");
                }
            }
        }
        let _ = writeln!(c, " }} S{k};");
        let ls = leaves(fields);
        let _ = writeln!(c, "S{k} c_make{k}(int32_t seed) {{ S{k} s;");
        for (j, (acc, _, sc)) in ls.iter().enumerate() {
            let _ = writeln!(c, "  s.{acc} = {};", sc.c_value(j));
        }
        let _ = writeln!(c, "  return s; }}");
        let _ = writeln!(c, "int32_t c_check{k}(S{k} s, int32_t seed) {{ int32_t m = 0;");
        for (j, (acc, _, sc)) in ls.iter().enumerate() {
            let _ = writeln!(c, "  if (s.{acc} != {}) m |= 1 << {};", sc.c_value(j), j % 31);
        }
        let _ = writeln!(c, "  return m; }}");
        let _ = writeln!(c, "S{k} c_echo{k}(S{k} s) {{ return s; }}");
        let _ = writeln!(
            c,
            "int32_t c_many{k}(S{k} a, S{k} b, S{k} c, S{k} d, S{k} e, S{k} f, S{k} g, S{k} h, int32_t seed) {{\n  return c_check{k}(a, seed) | c_check{k}(b, seed + 1) | c_check{k}(c, seed + 2) | c_check{k}(d, seed + 3) | c_check{k}(e, seed + 4) | c_check{k}(f, seed + 5) | c_check{k}(g, seed + 6) | c_check{k}(h, seed + 7); }}"
        );
        let _ = writeln!(
            c,
            "int32_t c_mixed{k}(int64_t x, double y, S{k} s, int32_t z, double w, int32_t seed) {{\n  return c_check{k}(s, seed) | (x != 11) << 27 | (y != 12.5) << 28 | (z != 13) << 29 | (w != 14.5) << 30; }}"
        );
        let _ = writeln!(c, "S{k} nx_make{k}(int32_t seed);\nint32_t nx_check{k}(S{k} s, int32_t seed);");
        let _ = writeln!(
            c,
            "int32_t c_calls_nexul{k}(int32_t seed) {{ return c_check{k}(nx_make{k}(seed), seed) | nx_check{k}(c_make{k}(seed + 1), seed + 1); }}\n"
        );
    }
    c
}

/// The NEXUL side: the same shapes, the C functions, `nx_make<k>` and `nx_check<k>` for
/// C, and a `main` that prints one line per shape (`S<k> ok`, or the failing checks).
fn nx_source(shapes: &[Vec<F>]) -> String {
    let mut n = String::from("// edition: 1\n// Generated by compiler/nxc/tests/c_abi.rs.\n\n");
    for (k, fields) in shapes.iter().enumerate() {
        let k = k + 1;
        let _ = writeln!(n, "@repr(\"C\")\ntype S{k} {{");
        for (i, f) in fields.iter().enumerate() {
            match *f {
                F::S(s) => {
                    let _ = writeln!(n, "    {} f{i}", s.nx());
                }
                F::A(s, len) => {
                    let _ = writeln!(n, "    [{}; {len}] f{i}", s.nx());
                }
                F::N9 => {
                    let _ = writeln!(n, "    S9 f{i}");
                }
            }
        }
        let _ = writeln!(n, "}}\n");
        let _ = writeln!(n, "extern \"C\" fn S{k} c_make{k}(i32 seed)\nextern \"C\" fn i32 c_check{k}(S{k} s, i32 seed)\nextern \"C\" fn S{k} c_echo{k}(S{k} s)");
        let _ = writeln!(n, "extern \"C\" fn i32 c_many{k}(S{k} a, S{k} b, S{k} c, S{k} d, S{k} e, S{k} f, S{k} g, S{k} h, i32 seed)");
        let _ = writeln!(n, "extern \"C\" fn i32 c_mixed{k}(i64 x, f64 y, S{k} s, i32 z, f64 w, i32 seed)\nextern \"C\" fn i32 c_calls_nexul{k}(i32 seed)\n");
        let ls = leaves(fields);
        // A value built in NEXUL: fields assigned one by one from a value made by C (the
        // layout is the same), so that the literal syntax of nested arrays is not needed.
        let _ = writeln!(n, "@export\nfn S{k} nx_make{k}(i32 seed) {{\n    var s = unsafe {{ c_make{k}(0) }}");
        for (j, (_, acc, sc)) in ls.iter().enumerate() {
            let _ = writeln!(n, "    s.{acc} = {}", sc.nx_value(j));
        }
        let _ = writeln!(n, "    return s\n}}\n");
        let _ = writeln!(n, "@export\nfn i32 nx_check{k}(S{k} s, i32 seed) {{\n    var i32 m = 0");
        for (j, (_, acc, sc)) in ls.iter().enumerate() {
            let _ = writeln!(n, "    if not ({}) {{\n        m = m | (1 << {})\n    }}", sc.nx_eq(&format!("s.{acc}"), j), j % 31);
        }
        let _ = writeln!(n, "    return m\n}}\n");
        let _ = writeln!(
            n,
            "fn test{k}() {{\n    var i32 m = 0\n    unsafe {{\n        m = m | c_check{k}(nx_make{k}(3), 3)\n        m = m | nx_check{k}(c_make{k}(4), 4) << 1\n        m = m | nx_check{k}(c_echo{k}(nx_make{k}(5)), 5) << 2\n        m = m | c_many{k}(nx_make{k}(6), nx_make{k}(7), nx_make{k}(8), nx_make{k}(9), nx_make{k}(10), nx_make{k}(11), nx_make{k}(12), nx_make{k}(13), 6) << 3\n        m = m | c_mixed{k}(11, 12.5, nx_make{k}(14), 13, 14.5, 14) << 4\n        m = m | c_calls_nexul{k}(15) << 5\n    }}\n    if m == 0 {{\n        println(\"S{k} ok\")\n    }} else {{\n        println(\"S{k} failed: {{m}}\")\n    }}\n}}\n"
        );
    }
    let _ = writeln!(n, "fn main() {{");
    for k in 1..=shapes.len() {
        let _ = writeln!(n, "    test{k}()");
    }
    let _ = writeln!(n, "}}");
    n
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn clang() -> Option<String> {
    let names: &[&str] = if cfg!(target_os = "macos") { &["clang", "cc"] } else { &["clang"] };
    names.iter().find(|n| Command::new(n).arg("--version").output().map_or(false, |o| o.status.success())).map(|n| n.to_string())
}

fn require_clang(test: &str) -> Option<String> {
    let c = clang();
    if c.is_none() {
        assert!(std::env::var_os("NEXUL_REQUIRE_CC").is_none(), "NEXUL_REQUIRE_CC is set but clang was not found");
        eprintln!("{test}: no clang, not tested");
    }
    c
}

/// clang's triple and flags for a NEXUL target.
fn clang_target(name: &str) -> (&'static str, &'static [&'static str]) {
    match name {
        "x86_64-linux" => ("x86_64-unknown-linux-gnu", &[]),
        "aarch64-linux" => ("aarch64-unknown-linux-gnu", &[]),
        "riscv64-linux" => ("riscv64-unknown-linux-gnu", &[]),
        "x86_64-windows" => ("x86_64-pc-windows-msvc", &[]),
        "aarch64-windows" => ("aarch64-pc-windows-msvc", &[]),
        "x86_64-macos" => ("x86_64-apple-macosx11.0.0", &[]),
        "aarch64-macos" => ("arm64-apple-macosx11.0.0", &[]),
        "wasm32-wasi" => ("wasm32-unknown-wasi", &[]),
        "wasm32-web" => ("wasm32-unknown-unknown", &[]),
        "thumbv7em-none" => ("thumbv7em-none-eabihf", &["-mcpu=cortex-m4", "-mfpu=fpv4-sp-d16", "-mfloat-abi=hard"]),
        "thumbv7m-none" => ("thumbv7m-none-eabi", &["-mcpu=cortex-m3"]),
        "riscv32-none" => ("riscv32-unknown-none-elf", &["-march=rv32imac", "-mabi=ilp32"]),
        "aarch64-none" => ("aarch64-unknown-none", &[]),
        "riscv64-none" => ("riscv64-unknown-none-elf", &["-march=rv64imafdc", "-mabi=lp64d"]),
        other => panic!("no clang target for {other}"),
    }
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nxc-c-abi-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(cmd: &mut Command, what: &str) -> String {
    let out = cmd.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(out.status.success(), "{what} failed ({}):\n{}{}", out.status, String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}

/// Removes the attributes with a parenthesized argument that do not decide the calling
/// convention and that newer clangs add (`range(i32 0, 2)` from LLVM 19, `captures(none)`
/// from LLVM 21…): they would hide the `(` that opens the parameters.
fn strip_attr_groups(line: &str) -> String {
    const GROUPS: &[&str] = &["range(", "nofpclass(", "initializes(", "captures(", "dereferenceable(", "dereferenceable_or_null(", "memory("];
    let mut out = String::new();
    let mut i = 0;
    'scan: while i < line.len() {
        for g in GROUPS {
            let starts_word = i == 0 || !line.as_bytes()[i - 1].is_ascii_alphanumeric() && line.as_bytes()[i - 1] != b'_';
            if starts_word && line[i..].starts_with(g) {
                let mut depth = 0;
                for (j, ch) in line[i..].char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                i += j + 1;
                                continue 'scan;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        let ch = line[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// A signature line reduced to what the calling convention decides: types and the
/// attributes `byval`, `sret`, `align`, `alignstack`, `signext`, `zeroext`; structure
/// names without their decoration (`%struct.S9`, `%"S9#12"` → `%S9`).
fn normalize(line: &str) -> String {
    let line = strip_attr_groups(line.trim());
    let line = line.as_str();
    let at = line.find('@').unwrap();
    let open = at + line[at..].find('(').unwrap();
    let (head, rest) = (&line[..open], &line[open + 1..]);
    let (before, name) = head.rsplit_once('@').unwrap();
    let name = name.trim_matches('"');
    // The result type: what is left before the name once the keywords are removed.
    let ret_toks: Vec<&str> = before
        .split_whitespace()
        .filter(|t| !matches!(*t, "define" | "declare" | "dso_local" | "hidden" | "weak" | "internal" | "dllexport" | "local_unnamed_addr" | "noundef" | "signext" | "zeroext" | "arm_aapcs_vfpcc" | "arm_aapcscc") && !t.starts_with("range("))
        .collect();
    let ret = ret_toks.join(" ");
    let depth_end = {
        let mut depth = 1;
        let mut end = 0;
        for (i, ch) in rest.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        end
    };
    let params = &rest[..depth_end];
    let keep = |tok: &str| {
        !matches!(tok, "noundef" | "nocapture" | "readonly" | "readnone" | "writeonly" | "writable" | "dead_on_unwind" | "noalias" | "returned" | "nonnull" | "inreg")
            && !tok.starts_with("dereferenceable")
            && !tok.starts_with("range(")
            && !tok.starts_with("nofpclass(")
            && !tok.starts_with('%') || tok.starts_with("%struct.") || tok.starts_with("%\"") && tok.contains('#')
    };
    let clean = |s: &str| -> String {
        let s = s.replace("%struct.", "%");
        // %"S9#12" → %S9
        let mut out = String::new();
        let mut it = s.split('"');
        let mut quoted = false;
        for part in &mut it {
            if quoted {
                out.push_str(part.split('#').next().unwrap());
            } else {
                out.push_str(part);
            }
            quoted = !quoted;
        }
        out
    };
    let mut ps = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for ch in params.chars() {
        match ch {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth -= 1,
            _ => {}
        }
        if ch == ',' && depth == 0 {
            ps.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() {
        ps.push(cur.trim().to_string());
    }
    let ps: Vec<String> = ps
        .iter()
        .map(|p| {
            // Drop the parameter name (a trailing `%x`), keep types and attributes.
            let toks: Vec<&str> = p.split_whitespace().collect();
            let toks: Vec<&str> = toks.iter().enumerate().filter(|(i, t)| !(*i > 0 && t.starts_with('%') && !t.starts_with("%struct.") && !t.contains('#') && !t.contains("byval") && !t.contains("sret"))).map(|(_, t)| *t).collect();
            clean(&toks.into_iter().filter(|t| keep(t)).collect::<Vec<_>>().join(" "))
        })
        .collect();
    format!("{} {}({})", clean(&ret), name, ps.join(", "))
}

#[test]
fn signatures_match_clang() {
    let Some(clang) = require_clang("signatures_match_clang") else { return };
    let shapes = shapes();
    let dir = scratch("sig");
    std::fs::write(dir.join("c.c"), c_source(&shapes)).unwrap();
    std::fs::write(dir.join("m.nx"), nx_source(&shapes)).unwrap();
    let mut failures = Vec::new();
    for target in ["x86_64-linux", "aarch64-linux", "riscv64-linux", "x86_64-windows", "aarch64-windows", "x86_64-macos", "aarch64-macos", "wasm32-wasi", "thumbv7em-none", "thumbv7m-none", "riscv32-none", "aarch64-none", "riscv64-none"] {
        let (triple, flags) = clang_target(target);
        let ll = run(Command::new(&clang).arg(format!("--target={triple}")).args(flags).args(["-ffreestanding", "-O1", "-S", "-emit-llvm", "-o", "-"]).arg(dir.join("c.c")), "clang -emit-llvm");
        let ours = dir.join(format!("{target}.ll"));
        run(Command::new(env!("CARGO_BIN_EXE_nxc")).args(["build", "--target", target, "--emit", "llvm-ir", "-o"]).arg(&ours).arg(dir.join("m.nx")), "nxc --emit llvm-ir");
        let ours = std::fs::read_to_string(&ours).unwrap();
        let sigs = |text: &str, kw: &str| -> Vec<(String, String)> {
            text.lines()
                .filter(|l| l.starts_with(kw))
                .filter_map(|l| {
                    let n = normalize(l);
                    let name = n.split('(').next()?.rsplit(' ').next()?.to_string();
                    (name.starts_with("c_") && !name.starts_with("c_calls") || name.starts_with("nx_")).then_some((name, n))
                })
                .collect()
        };
        // clang defines the C functions and declares the NEXUL ones; nxc the reverse.
        let theirs: std::collections::HashMap<String, String> = sigs(&ll, "define").into_iter().chain(sigs(&ll, "declare")).collect();
        let mine: Vec<(String, String)> = sigs(&ours, "declare").into_iter().chain(sigs(&ours, "define")).collect();
        let mut seen = 0;
        for (name, sig) in mine {
            if let Some(t) = theirs.get(&name) {
                seen += 1;
                if *t != sig {
                    failures.push(format!("[{target}] {name}\n  clang: {t}\n  nxc:   {sig}"));
                }
            }
        }
        assert!(seen >= shapes.len() * 6, "[{target}] only {seen} signatures compared");
    }
    assert!(failures.is_empty(), "{} signature(s) differ from clang:\n{}", failures.len(), failures.join("\n"));
}

/// Builds the C side for `target` (an object) and the NEXUL program with it, and runs it.
fn exchange_on(clang: &str, target: &str) -> Result<String, String> {
    let shapes = shapes();
    let dir = scratch(target);
    std::fs::write(dir.join("c.c"), c_source(&shapes)).unwrap();
    std::fs::write(dir.join("m.nx"), nx_source(&shapes)).unwrap();
    let host = nxc_driver::Target::host();
    let t = nxc_driver::Target::from_name(target).unwrap();
    let obj = dir.join(if t.os == nxc_driver::Os::Windows { "c.obj" } else { "c.o" });
    let (triple, flags) = clang_target(target);
    let mut cc = Command::new(clang);
    if t != host || t.os != nxc_driver::Os::MacOs {
        cc.arg(format!("--target={triple}"));
    }
    run(cc.args(flags).args(["-ffreestanding", "-fno-stack-protector", "-O1", "-c", "-o"]).arg(&obj).arg(dir.join("c.c")), "clang -c");
    let exe = dir.join(t.exe_name("m"));
    run(Command::new(env!("CARGO_BIN_EXE_nxc")).args(["build", "--target", target, "--link-arg"]).arg(&obj).arg("-o").arg(&exe).arg(dir.join("m.nx")), "nxc build");
    let out = if t == host {
        Command::new(&exe).output()
    } else if t.os == nxc_driver::Os::Wasi {
        Command::new("node").arg("--no-warnings").arg(root().join("tools").join("wasi-run.mjs")).arg(&exe).output()
    } else if t.os == nxc_driver::Os::Linux {
        let arch = if t.arch == nxc_driver::Arch::Aarch64 { "aarch64" } else { "riscv64" };
        Command::new(format!("qemu-{arch}")).arg(&exe).output()
    } else {
        // Bare metal: QEMU (MPS2-AN386) with semihosting, as tests/run does; a program
        // that faults spins in its handler, so it is stopped after two minutes.
        Command::new("timeout")
            .args(["120", "qemu-system-arm", "-M", "mps2-an386", "-nographic", "-monitor", "none", "-serial", "none"])
            .args(["-semihosting-config", "enable=on,target=native,chardev=con", "-chardev", "stdio,id=con,mux=off", "-kernel"])
            .arg(&exe)
            .output()
    }
    .map_err(|e| format!("cannot run: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    if !out.status.success() {
        return Err(format!("exit {}\n{}{}", out.status, stdout, String::from_utf8_lossy(&out.stderr)));
    }
    Ok(stdout)
}

#[test]
fn c_and_nexul_exchange_structures() {
    let Some(clang) = require_clang("c_and_nexul_exchange_structures") else { return };
    let expected: String = (1..=shapes().len()).map(|k| format!("S{k} ok\n")).collect();
    let mut targets = vec![nxc_driver::Target::host().name.to_string()];
    if let Ok(list) = std::env::var("NEXUL_TEST_TARGETS") {
        targets.extend(list.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
    }
    let mut failures = Vec::new();
    for target in &targets {
        match exchange_on(&clang, target) {
            Ok(out) if out == expected => {}
            Ok(out) => failures.push(format!("[{target}] output:\n{out}")),
            Err(e) => failures.push(format!("[{target}] {e}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

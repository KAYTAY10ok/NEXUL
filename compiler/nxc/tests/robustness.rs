//! Robustness of the front end (spec §7.4, contract K6.3): `nxc` never crashes, whatever
//! the source. Every program of the corpus (`tests/ui`, `tests/run`, `examples`) is
//! mutated deterministically (truncation, deletion, duplication, line swap, inserted
//! token, replaced word) and compiled in both editions, by `nxc check` and up to the
//! LLVM IR text; a panic of the compiler is a failure.
//!
//! `NEXUL_FUZZ_ROUNDS` (default 6) sets the number of mutants per file and
//! `NEXUL_FUZZ_SEED` changes the series. Failing mutants are written to
//! `<temp>/nexul-fuzz/` to be replayed with `nxc check`.

use nxc_driver::{compile_source, CompileOptions, Mode};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn corpus() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for dir in ["tests/ui", "tests/run", "examples"] {
        if let Ok(rd) = std::fs::read_dir(root().join(dir)) {
            v.extend(rd.flatten().map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "nx")));
        }
    }
    v.sort();
    v
}

/// xorshift64*: a small deterministic generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

/// Tokens that often break the structure of a program in interesting ways.
const TOKENS: &[&str] = &[
    "{", "}", "(", ")", "[", "]", ",", ".", "..", "..=", "?", "!", "&", "=>", "->", "|>", ":", ";", "\n", "\"", "'", "\\",
    "fn", "let", "var", "match", "when", "then", "if", "else", "for", "in", "while", "loop", "return", "break", "move", "owned", "inout",
    "self", "Self", "type", "is", "any", "dyn", "impl", "trait", "struct", "enum", "indirect", "unsafe", "static", "pub", "import", "@",
    "0x", "1_", "9999999999999999999999", "-", "+%", "<<", "as", "_", "Vec[", "Option[", "x.0", "{x:", "\"{", "}\"",
];

/// Types, keywords and values that keep a program parseable but change its meaning.
const WORDS: &[&str] = &[
    "i8", "u8", "i32", "i64", "u64", "usize", "isize", "f32", "f64", "bool", "char", "String", "Never", "Vec", "Option", "Result",
    "Map", "Set", "Ptr", "Self", "self", "move", "owned", "inout", "var", "let", "true", "false", "None", "Some", "Ok", "Err", "0",
    "1", "255", "main", "drop", "clone", "unwrap", "len", "push", "x",
];

/// The byte ranges of the words (identifiers, keywords, numbers) of a source.
fn word_spans(src: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in src.char_indices() {
        let word = c.is_ascii_alphanumeric() || c == '_';
        match (word, start) {
            (true, None) => start = Some(i),
            (false, Some(st)) => {
                out.push((st, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(st) = start {
        out.push((st, src.len()));
    }
    out
}

fn floor_char(s: &str, mut i: usize) -> usize {
    i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// One mutant of `src` and the description of the mutation.
fn mutate(src: &str, rng: &mut Rng) -> (String, String) {
    let n = src.len();
    let a = floor_char(src, rng.below(n + 1));
    let b = floor_char(src, a + rng.below(64.min(n - a) + 1));
    match rng.below(8) {
        // Replace a word by another word of the program, or by a type or keyword: the
        // syntax often stays valid, which reaches the type checker and the NIR.
        6 | 7 => {
            let words: Vec<(usize, usize)> = word_spans(src);
            if words.is_empty() {
                return (src.to_string(), "unchanged".to_string());
            }
            let (ws, we) = words[rng.below(words.len())];
            let by = if rng.below(2) == 0 {
                let (os, oe) = words[rng.below(words.len())];
                src[os..oe].to_string()
            } else {
                WORDS[rng.below(WORDS.len())].to_string()
            };
            (format!("{}{}{}", &src[..ws], by, &src[we..]), format!("word {:?} at byte {} replaced by {:?}", &src[ws..we], ws, by))
        }
        0 => (src[..a].to_string(), format!("truncated at byte {}", a)),
        1 => (format!("{}{}", &src[..a], &src[b..]), format!("bytes {}..{} deleted", a, b)),
        2 => (format!("{}{}{}", &src[..b], &src[a..b], &src[b..]), format!("bytes {}..{} duplicated", a, b)),
        3 => {
            let mut lines: Vec<&str> = src.lines().collect();
            if lines.len() >= 2 {
                let i = rng.below(lines.len());
                let j = rng.below(lines.len());
                lines.swap(i, j);
                (lines.join("\n"), format!("lines {} and {} swapped", i + 1, j + 1))
            } else {
                (src.to_string(), "unchanged".to_string())
            }
        }
        _ => {
            let t = TOKENS[rng.below(TOKENS.len())];
            (format!("{}{}{}", &src[..a], t, &src[a..]), format!("{:?} inserted at byte {}", t, a))
        }
    }
}

/// Runs `f` on a thread with a large stack, as `nxc` does: `Err` holds the panic message.
fn isolated<F: FnOnce() + Send + 'static>(f: F) -> Result<(), String> {
    let handle = std::thread::Builder::new().stack_size(256 << 20).spawn(f).expect("start a thread");
    handle.join().map_err(|payload| {
        payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic without a message".to_string())
    })
}

/// Compiles a source: analysis only (`Mode::Check`) or, with `program`, up to the LLVM IR
/// text (`Mode::Program`: NIR, monomorphization and IR generation, which run without the
/// LLVM library).
fn compile(src: String, edition1: bool, program: bool) -> Result<(), String> {
    isolated(move || {
        let mut opts = CompileOptions::new(if program { Mode::Program } else { Mode::Check });
        if edition1 {
            opts.edition = nxc_syntax::Edition::V1;
        }
        let _ = compile_source("fuzz.nx", src.into_bytes(), None, &opts);
    })
}

/// Generated sources far beyond the limits of E0113 (100 000 nested parentheses, blocks
/// or `Vec[…]`, a sum of 200 000 terms) used to overflow the stack of `nxc`: they must be
/// refused with E0113, by the analysis and by the code generation.
#[test]
fn pathological_sources_are_refused_cleanly() {
    let n = 100_000;
    let sources = [
        format!("fn main() {{\n    let x = {}1{}\n    println(x)\n}}\n", "(".repeat(n), ")".repeat(n)),
        format!("fn main() {{\n    let x = {}\n    println(x)\n}}\n", vec!["1"; 2 * n].join(" + ")),
        format!("fn main() {{\n{}\n", "{\n".repeat(n)),
        format!("fn main() {{\n{}println(1)\n{}}}\n", "if true {\n".repeat(n), "}\n".repeat(n)),
        format!("fn main() {{\n    let v: {}i32{} = Vec.new()\n}}\n", "Vec[".repeat(n), "]".repeat(n)),
        format!("fn main() {{\n    let x = {}1\n}}\n", "-".repeat(n)),
    ];
    for (i, src) in sources.iter().enumerate() {
        for program in [false, true] {
            let src = src.clone();
            let result = isolated(move || {
                let c = compile_source("deep.nx", src.into_bytes(), None, &CompileOptions::new(if program { Mode::Program } else { Mode::Check }));
                assert!(c.diagnostics.contains("error[E0113]"), "no E0113:\n{}", c.diagnostics.chars().take(2000).collect::<String>());
            });
            assert_eq!(result, Ok(()), "source {} ({})", i, if program { "program" } else { "check" });
        }
    }
}

#[test]
fn the_harness_reports_a_panic() {
    std::panic::set_hook(Box::new(|_| {}));
    let caught = isolated(|| panic!("boom"));
    let _ = std::panic::take_hook();
    assert_eq!(caught, Err("boom".to_string()));
    assert_eq!(isolated(|| {}), Ok(()));
}

#[test]
fn the_front_end_never_panics_on_mutated_programs() {
    let rounds: usize = std::env::var("NEXUL_FUZZ_ROUNDS").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
    let seed: u64 = std::env::var("NEXUL_FUZZ_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(0x4E45_5855_4C21);
    let files = corpus();
    assert!(files.len() > 100, "corpus too small: {} files", files.len());
    // Keep the panic messages of the compiler out of the test output; they are reported
    // with each failure.
    std::panic::set_hook(Box::new(|_| {}));
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let chunks: Vec<Vec<PathBuf>> = (0..threads).map(|i| files.iter().skip(i).step_by(threads).cloned().collect()).collect();
    let out_dir = std::env::temp_dir().join("nexul-fuzz");
    let failures: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|chunk| {
                let out_dir = &out_dir;
                s.spawn(move || {
                    let mut failures = Vec::new();
                    for path in chunk {
                        let src = std::fs::read_to_string(path).unwrap_or_default();
                        let name = path.file_name().unwrap().to_string_lossy().to_string();
                        let mut rng = Rng(seed ^ name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3)) | 1);
                        for round in 0..rounds {
                            let (mutant, what) = mutate(&src, &mut rng);
                            let edition1 = round % 2 == 1;
                            let program = round % 4 >= 2;
                            if let Err(msg) = compile(mutant.clone(), edition1, program) {
                                let _ = std::fs::create_dir_all(out_dir);
                                let file = out_dir.join(format!("{}.{}.nx", name.trim_end_matches(".nx"), round));
                                let _ = std::fs::write(&file, &mutant);
                                failures.push(format!(
                                    "{} ({}, edition {}, {}): {}\n  mutant: {}",
                                    name,
                                    what,
                                    if edition1 { "1" } else { "0.2" },
                                    if program { "program" } else { "check" },
                                    msg.lines().next().unwrap_or(""),
                                    file.display()
                                ));
                            }
                        }
                    }
                    failures
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let _ = std::panic::take_hook();
    assert!(failures.is_empty(), "{} panic(s) of the front end:\n\n{}", failures.len(), failures.join("\n\n"));
}

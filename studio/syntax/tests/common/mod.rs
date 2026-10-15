//! Helpers shared by the integration tests.

#![allow(dead_code)]

use nexul_studio_syntax::{Kind, Token};
use std::path::{Path, PathBuf};

/// Root of the repository (this crate is `studio/syntax`).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Every `.nx` file of the repository (examples, tests, standard library), sorted.
pub fn nx_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            let name = e.file_name();
            let name = name.to_string_lossy();
            if p.is_dir() {
                if !matches!(name.as_ref(), ".git" | "target" | "node_modules") {
                    walk(&p, out);
                }
            } else if name.ends_with(".nx") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo_root(), &mut out);
    out.sort();
    out
}

/// The `.nx` files directly in `dir` (relative to the repository root), sorted.
pub fn nx_files_in(dir: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(repo_root().join(dir))
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "nx"))
        .collect();
    out.sort();
    out
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Checks the guarantees of [`Token`]: in order, not overlapping, not empty, on character
/// boundaries, without line break, and nothing but blanks (and a byte order mark at the
/// very start) between tokens: no text is lost.
pub fn check_invariants(text: &str, toks: &[Token]) -> Result<(), String> {
    let gap_ok = |lo: usize, hi: usize| {
        let gap = &text[lo..hi];
        let gap = if lo == 0 { gap.strip_prefix('\u{FEFF}').unwrap_or(gap) } else { gap };
        gap.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
    };
    let mut last = 0;
    for t in toks {
        let r = t.range.clone();
        if r.start < last {
            return Err(format!("token {r:?} {:?} starts before the end of the previous one ({last})", t.kind));
        }
        if r.start >= r.end {
            return Err(format!("empty token {r:?} {:?}", t.kind));
        }
        if r.end > text.len() {
            return Err(format!("token {r:?} past the end of the text ({})", text.len()));
        }
        if !text.is_char_boundary(r.start) || !text.is_char_boundary(r.end) {
            return Err(format!("token {r:?} {:?} not on character boundaries", t.kind));
        }
        if text[r.clone()].contains('\n') {
            return Err(format!("token {r:?} {:?} contains a line break", t.kind));
        }
        if !gap_ok(last, r.start) {
            return Err(format!("text lost before {r:?}: {:?}", &text[last..r.start]));
        }
        last = r.end;
    }
    if !gap_ok(last, text.len()) {
        return Err(format!("text lost at the end: {:?}", &text[last..]));
    }
    Ok(())
}

/// Describes the invalid tokens of a text, with their line numbers.
pub fn invalid_tokens(text: &str, toks: &[Token]) -> Vec<String> {
    toks.iter()
        .filter(|t| t.kind == Kind::Invalid)
        .map(|t| {
            let line = text[..t.range.start].matches('\n').count() + 1;
            format!("line {line}: {:?}", &text[t.range.clone()])
        })
        .collect()
}

/// A small deterministic pseudo-random generator (xorshift64*), for reproducible fuzzing.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// Fragments that exercise every corner of the tokenizer when glued at random.
pub const FRAGMENTS: &[&str] = &[
    "\"",
    "'",
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    "\\",
    "\\u{",
    "\\u{1F600}",
    "/*",
    "*/",
    "//",
    "///",
    "r\"",
    "0x",
    "0b1",
    "1.5e",
    "1_0",
    "..=",
    "..",
    ".",
    ":",
    ":>8",
    "@",
    "@test",
    "!",
    "!=",
    "?",
    "fn ",
    "let ",
    "struct ",
    "Point",
    "MAX",
    "x",
    "_",
    " ",
    "\t",
    "\n",
    "\r\n",
    "\r",
    "é",
    "中",
    "😀",
    "\u{202E}",
    "\u{2066}",
    "\u{061C}",
    "\u{FEFF}",
    "\u{7}",
    "\0",
    "#",
    "$",
    "`",
    "+%=",
    "<<=",
    "->",
    "=>",
    "self",
    "Self",
    "true",
    "if ",
    "else",
    "\"{x:02x}\"",
];

/// A random text made of [`FRAGMENTS`].
pub fn random_text(rng: &mut Rng, max_fragments: usize) -> String {
    let n = rng.below(max_fragments + 1);
    let mut s = String::new();
    for _ in 0..n {
        s.push_str(rng.pick(FRAGMENTS));
    }
    s
}

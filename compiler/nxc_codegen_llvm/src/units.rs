//! Units of the incremental build (ADR 0016): the module of the program, cut into
//! modules that LLVM compiles one by one, and that the driver caches by their text.
//!
//! The cut works on the text of the IR. Each unit gets its functions, sorted by symbol,
//! and what they use:
//! - the private constants (strings, locations, vtables), copied into each unit;
//! - the types and the debug metadata;
//! - declarations of the functions and variables defined by other units.
//!
//! The variables (`static`) are defined in the common unit only. A function or variable
//! that was `internal` becomes `hidden`, so that the other units of the program reach it
//! without any library exporting it.
//!
//! In each unit, the private constants, types and metadata are renamed in the order in
//! which the unit uses them. A unit whose functions did not change thus has the same text,
//! even when the program around it changed: its object is taken from the cache.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write;

/// The pieces of the module of a program, as `Gen` writes them.
#[derive(Clone, Debug, Default)]
pub struct Parts {
    pub triple: String,
    pub module_asm: Vec<String>,
    pub type_defs: String,
    pub globals: String,
    pub decls: String,
    pub fn_defs: String,
    pub md_tail: String,
    pub md_defs: String,
    /// Definitions get the attribute `"probe-stack"="inline-asm"` (`#0`).
    pub probe_stack: bool,
    /// The unit of each function, by its name in the IR (`@"main.f"`). The others are in
    /// the common unit.
    pub unit_of: HashMap<String, String>,
}

/// A unit: its name and its IR.
#[derive(Clone, Debug)]
pub struct Unit {
    pub name: String,
    pub ir: String,
}

/// The name of the common unit.
pub const COMMON: &str = "common";

/// 64-bit FNV-1a: stable across builds and versions of Rust.
pub fn hash64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 128 bits of hash, in hexadecimal.
pub fn hash_hex(bytes: &[u8]) -> String {
    let a = hash64(bytes);
    let mut seeded = b"nexul".to_vec();
    seeded.extend_from_slice(bytes);
    let b = hash64(&seeded);
    format!("{:016x}{:016x}", a, b)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Tok {
    /// `@name`
    Global,
    /// `%name`
    Local,
    /// `!N`
    Meta,
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'$' | b'-')
}

/// Calls `f` for each name of `s`: `@x`, `@"x"`, `%x`, `%"x"`, `!N`. Strings and
/// comments are skipped.
fn tokens(s: &str, mut f: impl FnMut(Tok, usize, usize)) {
    let b = s.as_bytes();
    let n = b.len();
    let mut i = 0;
    while i < n {
        match b[i] {
            b'"' => {
                i += 1;
                while i < n && b[i] != b'"' {
                    i += 1;
                }
                i += 1;
            }
            b';' => {
                while i < n && b[i] != b'\n' {
                    i += 1;
                }
            }
            c @ (b'@' | b'%') => {
                let st = i;
                i += 1;
                if i < n && b[i] == b'"' {
                    i += 1;
                    while i < n && b[i] != b'"' {
                        i += 1;
                    }
                    i += 1;
                } else {
                    while i < n && is_ident(b[i]) {
                        i += 1;
                    }
                }
                if i > st + 1 {
                    f(if c == b'@' { Tok::Global } else { Tok::Local }, st, i.min(n));
                }
            }
            b'!' => {
                let st = i;
                i += 1;
                if i < n && b[i].is_ascii_digit() {
                    while i < n && b[i].is_ascii_digit() {
                        i += 1;
                    }
                    f(Tok::Meta, st, i);
                } else {
                    while i < n && is_ident(b[i]) {
                        i += 1;
                    }
                }
            }
            _ => i += 1,
        }
    }
}

/// The first name of a kind in a line.
fn first(line: &str, kind: Tok) -> Option<String> {
    let mut r = None;
    tokens(line, |k, a, b| {
        if r.is_none() && k == kind {
            r = Some(line[a..b].to_string());
        }
    });
    r
}

/// New names, by kind of name.
type Names = [HashMap<String, String>; 3];

fn kind_index(k: Tok) -> usize {
    match k {
        Tok::Global => 0,
        Tok::Local => 1,
        Tok::Meta => 2,
    }
}

/// `s` with its names renamed by `map`.
fn rename(s: &str, map: &Names) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    tokens(s, |k, a, b| {
        if let Some(n) = map[kind_index(k)].get(&s[a..b]) {
            out.push_str(&s[last..a]);
            out.push_str(n);
            last = b;
        }
    });
    out.push_str(&s[last..]);
    out
}

/// The type of a global from its definition `@x = <linkage> [unnamed_addr] global T ...`,
/// and whether it is a constant.
fn global_type(line: &str) -> Option<(bool, String)> {
    let mut rest = &line[line.find(" = ")? + 3..];
    let constant = loop {
        let (word, r) = rest.split_once(' ')?;
        rest = r;
        match word {
            "global" => break false,
            "constant" => break true,
            _ => {}
        }
    };
    let b = rest.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'{' | b'[' | b'<' | b'(' => depth += 1,
            b'}' | b']' | b'>' | b')' => depth -= 1,
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += 1;
                }
            }
            b' ' | b',' if depth == 0 => break,
            _ => {}
        }
        i += 1;
    }
    Some((constant, rest[..i.min(b.len())].to_string()))
}

/// The declaration of a function defined by another unit, from its definition line.
fn declaration(header: &str) -> String {
    let mut h = header.trim_end();
    h = h.strip_suffix('{').unwrap_or(h).trim_end();
    let h = match h.find(" !dbg ") {
        Some(p) => &h[..p],
        None => h,
    };
    let mut rest = h.strip_prefix("define ").unwrap_or(h);
    let mut hidden = false;
    loop {
        if let Some(r) = rest.strip_prefix("internal ").or_else(|| rest.strip_prefix("hidden ")) {
            hidden = true;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("weak ").or_else(|| rest.strip_prefix("private ")) {
            rest = r;
        } else {
            break;
        }
    }
    format!("declare {}{}", if hidden { "hidden " } else { "" }, rest)
}

struct Func<'a> {
    name: String,
    unit: &'a str,
    text: &'a str,
}

/// What a unit uses, found in the order of use.
struct Collect<'a, 'b> {
    unit: &'a str,
    common: bool,
    defined: &'b HashMap<&'a str, &'b Func<'a>>,
    private: &'b HashMap<String, &'a str>,
    shared: &'b HashMap<&'a str, &'a str>,
    decls: &'b HashMap<String, &'a str>,
    types: &'b HashMap<String, &'a str>,
    md: &'b HashMap<String, &'a str>,
    /// The canonical name of each private constant, type and metadata.
    rn: Names,
    queue: VecDeque<(Tok, String)>,
    order: Vec<(Tok, String)>,
    seen: [HashSet<String>; 3],
    n_seen: usize,
    ext_fns: Vec<String>,
    ext_vars: Vec<String>,
    used_decls: Vec<String>,
    /// Names given so far: private constants, types, metadata.
    count: [usize; 3],
}

impl Collect<'_, '_> {
    fn visit(&mut self, text: &str) {
        let mut found = Vec::new();
        tokens(text, |k, a, b| found.push((k, a, b)));
        for (k, a, b) in found {
            let name = &text[a..b];
            let ki = kind_index(k);
            if self.seen[ki].contains(name) {
                continue;
            }
            self.seen[ki].insert(name.to_string());
            self.n_seen += 1;
            let new = match k {
                Tok::Global => {
                    if let Some(f) = self.defined.get(name) {
                        if f.unit != self.unit {
                            self.ext_fns.push(name.to_string());
                        }
                        None
                    } else if self.private.contains_key(name) {
                        Some(format!("@p.{}", self.count[0]))
                    } else {
                        if self.shared.contains_key(name) {
                            if !self.common {
                                self.ext_vars.push(name.to_string());
                            }
                        } else if self.decls.contains_key(name) {
                            self.used_decls.push(name.to_string());
                        }
                        None
                    }
                }
                Tok::Local => self.types.contains_key(name).then(|| format!("%T{}", self.count[1])),
                Tok::Meta => self.md.contains_key(name).then(|| format!("!{}", self.count[2])),
            };
            if let Some(n) = new {
                self.count[ki] += 1;
                self.rn[ki].insert(name.to_string(), n);
                self.queue.push_back((k, name.to_string()));
            }
        }
    }
}

/// Cuts the module into units: the common unit first, then the others by name.
pub fn split(p: &Parts) -> Vec<Unit> {
    // What each name is.
    let mut types: HashMap<String, &str> = HashMap::new();
    for line in p.type_defs.lines() {
        if let Some(n) = first(line, Tok::Local) {
            types.insert(n, line);
        }
    }
    let mut private: HashMap<String, &str> = HashMap::new();
    let mut shared: Vec<(String, &str)> = Vec::new();
    for line in p.globals.lines() {
        let Some(n) = first(line, Tok::Global) else { continue };
        if line[n.len()..].starts_with(" = private ") {
            private.insert(n, line);
        } else {
            shared.push((n, line));
        }
    }
    shared.sort_by(|a, b| a.0.cmp(&b.0));
    let shared_map: HashMap<&str, &str> = shared.iter().map(|(n, l)| (n.as_str(), *l)).collect();
    let mut decls: HashMap<String, &str> = HashMap::new();
    for line in p.decls.lines() {
        if let Some(n) = first(line, Tok::Global) {
            decls.insert(n, line);
        }
    }
    let mut md: HashMap<String, &str> = HashMap::new();
    for line in p.md_defs.lines() {
        if let Some(n) = first(line, Tok::Meta) {
            md.insert(n, line);
        }
    }
    let mut funcs: Vec<Func> = Vec::new();
    let mut start: Option<usize> = None;
    let mut off = 0;
    for line in p.fn_defs.split_inclusive('\n') {
        if line.starts_with("define ") {
            start = Some(off);
        }
        off += line.len();
        if line.trim_end() == "}" {
            if let Some(s) = start.take() {
                let text = &p.fn_defs[s..off];
                let name = first(text.lines().next().unwrap_or(""), Tok::Global).unwrap_or_default();
                let unit = p.unit_of.get(&name).map(|u| u.as_str()).unwrap_or(COMMON);
                funcs.push(Func { name, unit, text });
            }
        }
    }
    funcs.sort_by(|a, b| (a.unit, &a.name).cmp(&(b.unit, &b.name)));
    let defined: HashMap<&str, &Func> = funcs.iter().map(|f| (f.name.as_str(), f)).collect();
    let mut names: Vec<&str> = funcs.iter().map(|f| f.unit).collect();
    names.push(COMMON);
    names.sort();
    names.dedup();
    names.retain(|n| *n != COMMON);
    names.insert(0, COMMON);

    // The units are independent: cut them in parallel.
    let one = |unit: &str| -> Unit {
        let common = unit == COMMON;
        let mine: Vec<&Func> = funcs.iter().filter(|f| f.unit == unit).collect();
        let mut c = Collect { unit, common, defined: &defined, private: &private, shared: &shared_map, decls: &decls, types: &types, md: &md, rn: Default::default(), queue: VecDeque::new(), order: Vec::new(), seen: Default::default(), n_seen: 0, ext_fns: Vec::new(), ext_vars: Vec::new(), used_decls: Vec::new(), count: [0; 3] };
        c.visit(&p.md_tail);
        for f in &mine {
            c.visit(f.text);
        }
        if common {
            for (_, line) in &shared {
                c.visit(line);
            }
        }
        loop {
            while let Some(key) = c.queue.pop_front() {
                let line = match key.0 {
                    Tok::Global => private[&key.1],
                    Tok::Local => types[&key.1],
                    Tok::Meta => md[&key.1],
                };
                c.visit(line);
                c.order.push(key);
            }
            // Declarations use types too: only the signature of a function, not its body.
            let before = c.n_seen;
            let pending: Vec<String> = c.used_decls.iter().chain(c.ext_fns.iter()).chain(c.ext_vars.iter()).cloned().collect();
            for n in pending {
                let line = decls
                    .get(n.as_str())
                    .copied()
                    .or_else(|| defined.get(n.as_str()).map(|f| f.text.lines().next().unwrap_or("")))
                    .or_else(|| shared_map.get(n.as_str()).copied())
                    .unwrap_or("");
                c.visit(line);
            }
            if c.queue.is_empty() && c.n_seen == before {
                break;
            }
        }
        let Collect { rn, order, mut ext_fns, mut ext_vars, mut used_decls, .. } = c;
        let mut ir = String::new();
        let _ = writeln!(ir, "; Unit `{}` of a program, generated by nxc.", unit);
        let _ = writeln!(ir, "target triple = \"{}\"\n", p.triple);
        if common {
            for asm in &p.module_asm {
                for line in asm.lines() {
                    let _ = writeln!(ir, "module asm \"{}\"", line.replace('\\', "\\5C").replace('"', "\\22"));
                }
            }
        }
        let by_kind = |k: Tok| order.iter().filter(move |o| o.0 == k);
        for key in by_kind(Tok::Local) {
            let _ = writeln!(ir, "{}", rename(types[&key.1], &rn));
        }
        for key in by_kind(Tok::Global) {
            let _ = writeln!(ir, "{}", rename(private[&key.1], &rn));
        }
        if common {
            for (_, line) in &shared {
                let line = match line.find(" = internal ") {
                    Some(p) => format!("{} = hidden {}", &line[..p], &line[p + " = internal ".len()..]),
                    None => line.to_string(),
                };
                let _ = writeln!(ir, "{}", rename(&line, &rn));
            }
        } else {
            ext_vars.sort();
            for n in &ext_vars {
                let line = shared_map[n.as_str()];
                if let Some((constant, ty)) = global_type(line) {
                    let _ = writeln!(ir, "{} = external hidden {} {}", n, if constant { "constant" } else { "global" }, rename(&ty, &rn));
                }
            }
        }
        used_decls.sort();
        for n in &used_decls {
            let _ = writeln!(ir, "{}", rename(decls[n.as_str()], &rn));
        }
        ext_fns.sort();
        for n in &ext_fns {
            let header = defined[n.as_str()].text.lines().next().unwrap_or("");
            let _ = writeln!(ir, "{}", rename(&declaration(header), &rn));
        }
        ir.push('\n');
        for f in &mine {
            for line in f.text.split_inclusive('\n') {
                let line = if line.starts_with("define ") {
                    let l = match line.strip_prefix("define internal ") {
                        Some(r) => format!("define hidden {}", r),
                        None => line.to_string(),
                    };
                    if p.probe_stack {
                        let at = l.find(" !dbg ").unwrap_or_else(|| l.rfind(" {").expect("function header"));
                        format!("{} #0{}", &l[..at], &l[at..])
                    } else {
                        l
                    }
                } else {
                    line.to_string()
                };
                ir.push_str(&rename(&line, &rn));
            }
            ir.push('\n');
        }
        if p.probe_stack && !mine.is_empty() {
            ir.push_str("\nattributes #0 = { \"probe-stack\"=\"inline-asm\" }\n");
        }
        ir.push_str(&rename(&p.md_tail, &rn));
        for key in by_kind(Tok::Meta) {
            let _ = writeln!(ir, "{}", rename(md[&key.1], &rn));
        }
        Unit { name: unit.to_string(), ir }
    };
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(names.len()).max(1);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut out: Vec<Option<Unit>> = vec![None; names.len()];
    let done: Vec<Vec<(usize, Unit)>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(unit) = names.get(i) else { break };
                        mine.push((i, one(unit)));
                    }
                    mine
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("cutting a unit")).collect()
    });
    for (i, u) in done.into_iter().flatten() {
        out[i] = Some(u);
    }
    out.into_iter().map(|u| u.expect("every unit is cut")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts() -> Parts {
        let mut unit_of = HashMap::new();
        unit_of.insert("@\"m.f\"".to_string(), "m/0of1".to_string());
        unit_of.insert("@\"m.g\"".to_string(), "m/0of1".to_string());
        Parts {
            triple: "x86_64-unknown-linux-gnu".into(),
            module_asm: vec![],
            type_defs: "%\"S#4\" = type { i64, ptr }\n%\"U#9\" = type { i32 }\n".into(),
            globals: "@str.7 = private unnamed_addr constant [3 x i8] c\"hi\\00\", align 1\n@\"m.v#2\" = internal global %\"S#4\" zeroinitializer, align 8\n".into(),
            decls: "declare void @puts(ptr)\n".into(),
            fn_defs: "define internal void @\"m.f\"() {\nstart:\n  call void @puts(ptr @str.7)\n  call void @\"m.h\"(ptr @\"m.v#2\")\n  ret void\n}\n\ndefine internal void @\"m.h\"(ptr %p) {\nstart:\n  %t1 = load %\"S#4\", ptr %p\n  ret void\n}\n\ndefine i32 @main() {\nstart:\n  call void @\"m.f\"()\n  ret i32 0\n}\n".into(),
            md_tail: String::new(),
            md_defs: String::new(),
            probe_stack: false,
            unit_of,
        }
    }

    #[test]
    fn units_declare_what_others_define() {
        let u = split(&parts());
        assert_eq!(u.len(), 2);
        assert_eq!(u[0].name, COMMON);
        let c = &u[0].ir;
        assert!(c.contains("@\"m.v#2\" = hidden global %T0 zeroinitializer"), "{c}");
        assert!(c.contains("declare hidden void @\"m.f\"()"), "{c}");
        assert!(c.contains("define hidden void @\"m.h\"(ptr %p)"), "{c}");
        let m = &u[1].ir;
        assert!(m.contains("@p.0 = private unnamed_addr constant [3 x i8] c\"hi\\00\""), "{m}");
        assert!(m.contains("@\"m.v#2\" = external hidden global %T0"), "{m}");
        assert!(m.contains("declare hidden void @\"m.h\"(ptr %p)"), "{m}");
        assert!(m.contains("declare void @puts(ptr)"), "{m}");
        assert!(m.contains("call void @puts(ptr @p.0)"), "{m}");
        assert!(!m.contains("U#9"), "{m}");
    }

    #[test]
    fn a_unit_does_not_depend_on_the_numbering_of_the_program() {
        let a = parts();
        let mut b = parts();
        b.type_defs = b.type_defs.replace("S#4", "S#40");
        b.globals = format!("@str.1 = private unnamed_addr constant [2 x i8] c\"x\\00\", align 1\n{}", b.globals.replace("@str.7", "@str.70").replace("S#4", "S#40"));
        b.fn_defs = b.fn_defs.replace("@str.7", "@str.70").replace("S#4", "S#40");
        let (ua, ub) = (split(&a), split(&b));
        assert_eq!(ua[1].ir, ub[1].ir);
    }
}

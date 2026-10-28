//! `nxc migrate`: rewrites a file of NEXUL 0.2 in the syntax of edition 1 (ADR 0010).
//!
//! The file is parsed as 0.2; the rewriting is a list of small edits at the positions the
//! syntax tree records, so comments and layout are kept. Constructs whose order of parts
//! changes (`let x: T` → `let T x`, `if let P = e` → `if e is P`, function headers) are
//! rebuilt from the tree. The result is parsed again as edition 1: what could not be
//! rewritten is reported, with its line, to be finished by hand.

use nxc_syntax::ast::*;
use nxc_syntax::{lexer, parser, DiagSink, Edition, SourceMap};
use std::collections::{HashMap, HashSet};

/// The result of a migration.
pub struct Migration {
    pub text: String,
    pub edits: usize,
    /// Places to finish by hand: line and reason.
    pub manual: Vec<(usize, String)>,
    /// Names renamed because edition 1 reserves them (`repeat` → `repeat_`).
    pub renamed: Vec<String>,
}

#[derive(Clone, Debug)]
struct Edit {
    lo: usize,
    hi: usize,
    text: String,
    seq: usize,
}

struct Migrator<'a> {
    src: &'a str,
    edits: Vec<Edit>,
    seq: usize,
    manual: Vec<(usize, String)>,
    /// The function being rewritten returns `Result[..., String]`.
    string_errors: bool,
    /// Public types copied by their structure in 0.2, which declare `Copy` in edition 1.
    add_copy: HashSet<String>,
    /// The parameters of the generic structs and enums of the module, by type name.
    type_generics: HashMap<String, Vec<String>>,
    /// The method being rewritten moves to an `extend Type[P is …]` block that carries the
    /// bounds of its `where` on the parameters of the type (ADR 0010, Q1).
    bounds_moved: bool,
}

/// The functions of `std.io` whose errors became `io.IoError` (ADR 0015).
const IO_FALLIBLE: &[&str] = &["read_file", "read_bytes", "write_file", "write_bytes", "append_file"];

/// Words that edition 1 reserves; a 0.2 program that uses them as names must rename them.
const NEW_WORDS: &[&str] = &[
    "when", "then", "is", "interface", "extend", "any", "distinct", "it", "repeat", "test", "expects", "requires", "ensures", "with", "every",
    "within", "spawn", "loop",
];

/// Migrates the text of one file. `Err`: the file is not valid NEXUL 0.2 (the diagnostics).
pub fn migrate(name: &str, src: &str) -> Result<Migration, String> {
    let (renamed_src, renamed, mut unsafe_words) = rename_reserved(src);
    let src = renamed_src.as_str();
    let mut sm = SourceMap::new();
    let file = sm.add(name, src);
    let diags = DiagSink::new();
    let toks = lexer::lex(src, file, &diags);
    let module = parser::parse_module(toks, &diags);
    if diags.has_errors() {
        let list = diags.take_sorted();
        return Err(list.iter().map(|d| format!("error[{}]: {}", d.code, d.message)).collect::<Vec<_>>().join("\n"));
    }
    let type_generics = module
        .items
        .iter()
        .filter_map(|it| match &it.kind {
            ItemKind::Struct(st) => Some((st.name.name.clone(), st.generics.iter().map(|g| g.name.clone()).collect())),
            ItemKind::Enum(en) => Some((en.name.name.clone(), en.generics.iter().map(|g| g.name.clone()).collect())),
            _ => None,
        })
        .collect();
    let mut m = Migrator { src, edits: Vec::new(), seq: 0, manual: Vec::new(), string_errors: false, add_copy: copy_types(&module), type_generics, bounds_moved: false };
    for item in &module.items {
        m.item(item);
    }
    for (pos, w) in unsafe_words.drain(..) {
        m.manual(pos, format!("`{}` is a keyword in edition 1 and is also a field or method name: rename it", w));
    }
    let edits = m.edits.len();
    let text = m.render(0, src.len(), &m.edits.clone());
    // What edition 1 still refuses is left to the reader.
    let mut sm1 = SourceMap::new();
    let f1 = sm1.add_with_edition(name, text.clone(), Edition::V1);
    let d1 = DiagSink::new();
    let toks1 = lexer::lex_in(&text, f1, &d1, Edition::V1);
    let _ = parser::parse_module_in(toks1, &d1, Edition::V1);
    let mut manual = m.manual;
    for d in d1.take_sorted() {
        let line = d.primary_span().map(|s| sm1.get(f1).line_col(s.lo).0).unwrap_or(0);
        manual.push((line, format!("{} [{}]", d.message, d.code)));
    }
    manual.sort();
    manual.dedup();
    Ok(Migration { text, edits, manual, renamed })
}

impl<'a> Migrator<'a> {
    // -----------------------------------------------------------------------
    // Edits

    fn rep(&mut self, lo: usize, hi: usize, text: impl Into<String>) {
        self.seq += 1;
        self.edits.push(Edit { lo, hi, text: text.into(), seq: self.seq });
    }

    fn ins(&mut self, at: usize, text: impl Into<String>) {
        self.rep(at, at, text);
    }

    /// The edits made by `f`, kept apart (to rebuild a construct whose parts move).
    fn capture(&mut self, f: impl FnOnce(&mut Self)) -> Vec<Edit> {
        let saved = std::mem::take(&mut self.edits);
        f(self);
        std::mem::replace(&mut self.edits, saved)
    }

    /// The text of `lo..hi` with the edits that fall inside it.
    fn render(&self, lo: usize, hi: usize, edits: &[Edit]) -> String {
        let mut inside: Vec<&Edit> = edits.iter().filter(|e| e.lo >= lo && e.hi <= hi).collect();
        inside.sort_by_key(|e| (e.lo, e.seq));
        let mut out = String::new();
        let mut at = lo;
        for e in inside {
            if e.lo < at {
                continue; // overlapping: the first edit wins
            }
            out.push_str(&self.src[at..e.lo]);
            out.push_str(&e.text);
            at = e.hi;
        }
        out.push_str(&self.src[at..hi]);
        out
    }

    fn text(&self, lo: usize, hi: usize) -> &str {
        &self.src[lo..hi]
    }

    /// `lo..hi` widened to whole lines, with the comment lines just above (`///`, `//`) and
    /// the line break at the end.
    fn lines_of(&self, lo: usize, hi: usize) -> (usize, usize) {
        let mut start = self.src[..lo].rfind('\n').map_or(0, |i| i + 1);
        while start > 0 {
            let prev = self.src[..start - 1].rfind('\n').map_or(0, |i| i + 1);
            let line = self.src[prev..start - 1].trim_start();
            if line.starts_with("//") || line.starts_with('@') {
                start = prev;
            } else {
                break;
            }
        }
        // The blank line that separated it from the previous member goes with it.
        if start > 0 {
            let prev = self.src[..start - 1].rfind('\n').map_or(0, |i| i + 1);
            if self.src[prev..start - 1].trim().is_empty() {
                start = prev;
            }
        }
        let end = self.src[hi..].find('\n').map_or(self.src.len(), |i| hi + i + 1);
        (start, end)
    }

    fn line_of(&self, pos: usize) -> usize {
        self.src[..pos.min(self.src.len())].matches('\n').count() + 1
    }

    fn manual(&mut self, pos: usize, why: impl Into<String>) {
        let line = self.line_of(pos);
        self.manual.push((line, why.into()));
    }

    /// Position of `needle` in `lo..hi`, outside strings and comments as far as headers go.
    fn find(&self, lo: usize, hi: usize, needle: &str) -> Option<usize> {
        self.src.get(lo..hi)?.find(needle).map(|i| lo + i)
    }

    fn rfind(&self, lo: usize, hi: usize, needle: &str) -> Option<usize> {
        self.src.get(lo..hi)?.rfind(needle).map(|i| lo + i)
    }

    /// The first non-blank position at or after `pos`.
    fn skip_blank(&self, mut pos: usize) -> usize {
        let b = self.src.as_bytes();
        while pos < b.len() && (b[pos] == b' ' || b[pos] == b'\t') {
            pos += 1;
        }
        pos
    }

    // -----------------------------------------------------------------------
    // Printing (types, parameters, generics)

    fn ty(&self, t: &TypeExpr) -> String {
        match &t.kind {
            TypeExprKind::Path { segments, args } => {
                let mut s = segments.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".");
                if !args.is_empty() {
                    s.push('[');
                    s.push_str(&args.iter().map(|a| self.ty(a)).collect::<Vec<_>>().join(", "));
                    s.push(']');
                }
                s
            }
            TypeExprKind::SelfType => "Self".into(),
            TypeExprKind::Array(elem, len) => format!("[{}; {}]", self.ty(elem), self.text(len.span.lo as usize, len.span.hi as usize)),
            TypeExprKind::Unit => "()".into(),
            TypeExprKind::Tuple(ts) => format!("({})", ts.iter().map(|t| self.ty(t)).collect::<Vec<_>>().join(", ")),
            TypeExprKind::Fn(params, ret) => {
                let ps: Vec<String> = params
                    .iter()
                    .map(|(c, t)| match c {
                        Convention::Inout => format!("{}!", self.ty(t)),
                        Convention::Owned => format!("owned {}", self.ty(t)),
                        Convention::Borrow => self.ty(t),
                    })
                    .collect();
                match ret {
                    Some(r) => format!("fn {}({})", self.ty(r), ps.join(", ")),
                    None => format!("fn({})", ps.join(", ")),
                }
            }
            TypeExprKind::Dyn(inner) => format!("any {}", self.ty(inner)),
            TypeExprKind::Error => self.text(t.span.lo as usize, t.span.hi as usize).to_string(),
        }
    }

    fn param(&self, p: &Param) -> String {
        match &p.kind {
            ParamKind::SelfParam => match p.conv {
                Convention::Inout => "self!".into(),
                Convention::Owned => "owned self".into(),
                Convention::Borrow => "self".into(),
            },
            ParamKind::Named { name, ty } => match p.conv {
                Convention::Inout => format!("{} {}!", self.ty(ty), name.name),
                Convention::Owned => format!("owned {} {}", self.ty(ty), name.name),
                Convention::Borrow => format!("{} {}", self.ty(ty), name.name),
            },
        }
    }

    /// `[T is A and B, U]`
    fn generics(&self, names: &[Ident], bounds: &[BoundDecl]) -> String {
        if names.is_empty() {
            return String::new();
        }
        let parts: Vec<String> = names
            .iter()
            .map(|n| {
                let traits: Vec<String> = bounds.iter().filter(|b| b.param.name == n.name).flat_map(|b| b.traits.iter().map(|t| self.ty(t))).collect();
                if traits.is_empty() {
                    n.name.clone()
                } else {
                    format!("{} is {}", n.name, traits.join(" and "))
                }
            })
            .collect();
        format!("[{}]", parts.join(", "))
    }

    // -----------------------------------------------------------------------
    // Items

    fn item(&mut self, item: &Item) {
        let lo = item.span.lo as usize;
        match &item.kind {
            ItemKind::Fn(f) => {
                let test = item.attrs.iter().find(|a| a.name.name == "test" && a.args.is_empty());
                if let Some(a) = test {
                    let fn_lo = f.span.lo as usize;
                    // `@test` and the line break before `fn` disappear.
                    self.rep(a.span.lo as usize, fn_lo, "");
                    self.fn_decl(f, Some(&f.name.name));
                } else {
                    self.fn_decl(f, None);
                }
            }
            ItemKind::Struct(s) => {
                if let Some(k) = self.find(lo, s.name.span.lo as usize, "struct") {
                    self.rep(k, k + "struct".len(), "type");
                }
                // Edition 1 copies a public type only if it says so (ADR 0012).
                if self.add_copy.contains(&s.name.name) {
                    if let Some(open) = self.find(s.name.span.hi as usize, item.span.hi as usize, "{") {
                        let spaced = self.src[..open].ends_with(char::is_whitespace);
                        self.ins(open, if spaced { "is Copy " } else { " is Copy " });
                    }
                }
                for fd in &s.fields {
                    let mut t = String::new();
                    if fd.is_pub {
                        t.push_str("pub ");
                    }
                    if fd.indirect {
                        t.push_str("indirect ");
                    }
                    t.push_str(&format!("{} {}", self.ty(&fd.ty), fd.name.name));
                    self.rep(fd.span.lo as usize, fd.span.hi as usize, t);
                }
            }
            ItemKind::Enum(e) => self.enum_decl(item, e),
            ItemKind::Impl(i) => {
                let Some(k) = self.find(lo, item.span.hi as usize, "impl") else { return };
                let open = i.items.first().map(|x| x.span.lo as usize).unwrap_or(item.span.hi as usize);
                let Some(brace) = self.rfind(k, open, "{") else { return };
                let ty = match &i.ty.kind {
                    TypeExprKind::Path { segments, args } if !i.bounds.is_empty() => {
                        let names: Vec<Ident> = args
                            .iter()
                            .filter_map(|a| match &a.kind {
                                TypeExprKind::Path { segments, .. } => segments.first().cloned(),
                                _ => None,
                            })
                            .collect();
                        format!("{}{}", segments.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join("."), self.generics(&names, &i.bounds))
                    }
                    _ => self.ty(&i.ty),
                };
                let head = match &i.trait_ {
                    Some(t) => format!("extend {} is {} ", ty, self.ty(t)),
                    None => format!("extend {} ", ty),
                };
                self.rep(k, brace, head);
                // Edition 1 bounds a parameter of the type on a block, not on a method: the
                // methods with such a `where` move to `extend Type[P is …] { … }` blocks after
                // this one, one per set of bounds (ADR 0010, Q1).
                let owner = match (&i.trait_, &i.ty.kind) {
                    (None, TypeExprKind::Path { segments, args }) if segments.len() == 1 && args.is_empty() => {
                        self.type_generics.get(&segments[0].name).cloned().map(|g| (segments[0].name.clone(), g))
                    }
                    _ => None,
                };
                let mut moved: Vec<(String, String)> = Vec::new();
                for it in &i.items {
                    let owner_bounds: Vec<&BoundDecl> = match (&owner, &it.kind) {
                        (Some(_), ItemKind::Fn(f)) => f.bounds.iter().filter(|b| !f.generics.iter().any(|g| g.name == b.param.name)).collect(),
                        _ => Vec::new(),
                    };
                    if owner_bounds.is_empty() {
                        self.item(it);
                        continue;
                    }
                    let (name, generics) = owner.clone().unwrap();
                    let params: Vec<String> = generics
                        .iter()
                        .map(|g| {
                            let traits: Vec<String> = owner_bounds.iter().filter(|b| b.param.name == *g).flat_map(|b| b.traits.iter().map(|t| self.ty(t))).collect();
                            if traits.is_empty() { g.clone() } else { format!("{} is {}", g, traits.join(" and ")) }
                        })
                        .collect();
                    let header = format!("extend {}[{}] {{", name, params.join(", "));
                    // The method, with its comments and attributes, from the start of its first
                    // line to the end of its last one.
                    let (lo, hi) = self.lines_of(it.span.lo as usize, it.span.hi as usize);
                    let edits = self.capture(|m| {
                        m.bounds_moved = true;
                        m.item(it);
                        m.bounds_moved = false;
                    });
                    let text = self.render(lo, hi, &edits);
                    self.rep(lo, hi, "");
                    // In its new block, one blank line between members and none at the top.
                    let text = text.trim_start_matches('\n').to_string();
                    match moved.iter_mut().find(|(h, _)| *h == header) {
                        Some((_, body)) => {
                            body.push('\n');
                            body.push_str(&text);
                        }
                        None => moved.push((header, text)),
                    }
                }
                let end = item.span.hi as usize;
                for (header, body) in moved {
                    self.ins(end, format!("\n\n{}\n{}}}", header, body));
                }
            }
            ItemKind::Trait(t) => {
                let Some(k) = self.find(lo, t.name.span.lo as usize, "trait") else { return };
                let open = t.items.first().map(|x| x.span.lo as usize).unwrap_or(item.span.hi as usize);
                let Some(brace) = self.rfind(k, open, "{") else { return };
                let mut head = format!("interface {}", t.name.name);
                if !t.supertraits.is_empty() {
                    head.push_str(&format!(" is {}", t.supertraits.iter().map(|s| self.ty(s)).collect::<Vec<_>>().join(" and ")));
                }
                head.push(' ');
                self.rep(k, brace, head);
                for it in &t.items {
                    self.item(it);
                }
            }
            ItemKind::Const(c) => {
                if let Some(k) = self.find(lo, c.name.span.lo as usize, "const") {
                    let eq = self.find(c.ty.span.hi as usize, c.value.span.lo as usize, "=").unwrap_or(c.ty.span.hi as usize);
                    self.rep(k, eq, format!("const {} {} ", self.ty(&c.ty), c.name.name));
                }
                self.expr(&c.value);
            }
            ItemKind::Static(s) => {
                if let Some(k) = self.find(lo, s.name.span.lo as usize, "static") {
                    let eq = self.find(s.ty.span.hi as usize, s.value.span.lo as usize, "=").unwrap_or(s.ty.span.hi as usize);
                    let var = if s.mutable { "var " } else { "" };
                    self.rep(k, eq, format!("static {}{} {} ", var, self.ty(&s.ty), s.name.name));
                }
                self.expr(&s.value);
            }
            // Edition 1 only: nothing to migrate.
            ItemKind::Newtype(_) => {}
            ItemKind::TypeAlias(a) => {
                let printed = self.ty(&a.ty);
                self.rep(a.ty.span.lo as usize, a.ty.span.hi as usize, printed);
            }
            ItemKind::ExternFn(e) => {
                let Some(k) = self.find(lo, e.name.span.lo as usize, "fn") else { return };
                let ret = e.ret.as_ref().map(|r| format!("{} ", self.ty(r))).unwrap_or_default();
                let params: Vec<String> = e.params.iter().map(|(n, t)| format!("{} {}", self.ty(t), n.name)).collect();
                self.rep(k, item.span.hi as usize, format!("fn {}{}({})", ret, e.name.name, params.join(", ")));
            }
        }
    }

    /// `fn name[G](params) -> R where … {` → `fn R name[G](params) {` (or `test "name" {`).
    fn fn_decl(&mut self, f: &FnDecl, test: Option<&str>) {
        let start = f.span.lo as usize;
        let head_end = match &f.body {
            Some(b) => b.span.lo as usize,
            None => f.span.hi as usize,
        };
        let header = self.text(start, head_end);
        if header.contains("//") || header.contains("/*") {
            self.manual(start, "comment inside a function header: moved out by the rewriting, check it");
        }
        // A bound on a parameter of the type (`where K: Eq` in `impl Map`): edition 1 has
        // no place for it in the header of the method, and dropping it would drop a check.
        if let Some(b) = f.bounds.iter().find(|b| !self.bounds_moved && !f.generics.iter().any(|g| g.name == b.param.name)) {
            self.manual(
                start,
                format!("`where {}: …` bounds a parameter of the type: move `{}` to a separate `extend Type[{} is …] {{ … }}`", b.param.name, f.name.name, b.param.name),
            );
        }
        let new = match test {
            Some(name) => format!("test \"{}\" ", name),
            None => {
                let mut s = String::new();
                if f.is_unsafe {
                    s.push_str("unsafe ");
                }
                s.push_str("fn ");
                if let Some(r) = &f.ret {
                    s.push_str(&self.ty(r));
                    s.push(' ');
                }
                s.push_str(&f.name.name);
                s.push_str(&self.generics(&f.generics, &f.bounds));
                s.push('(');
                s.push_str(&f.params.iter().map(|p| self.param(p)).collect::<Vec<_>>().join(", "));
                s.push(')');
                if f.body.is_some() {
                    s.push(' ');
                }
                s
            }
        };
        self.rep(start, head_end, new);
        if let Some(b) = &f.body {
            let outer = std::mem::replace(&mut self.string_errors, f.ret.as_ref().is_some_and(is_result_of_string));
            self.block(b);
            self.string_errors = outer;
        }
    }

    /// `enum E { A, B(T) }` → `type E = .A | .B(T)`, one variant per line if long.
    fn enum_decl(&mut self, item: &Item, e: &EnumDecl) {
        let lo = item.span.lo as usize;
        let Some(k) = self.find(lo, e.name.span.lo as usize, "enum") else { return };
        let end = item.span.hi as usize;
        let body = self.text(e.name.span.hi as usize, end);
        let comments: Vec<String> = body.lines().filter_map(|l| l.find("//").map(|i| l[i..].trim().to_string())).collect();
        let variants: Vec<String> = e
            .variants
            .iter()
            .map(|v| {
                let mut s = String::new();
                if v.indirect {
                    s.push_str("indirect ");
                }
                s.push('.');
                s.push_str(&v.name.name);
                if !v.fields.is_empty() {
                    s.push_str(&format!("({})", v.fields.iter().map(|t| self.ty(t)).collect::<Vec<_>>().join(", ")));
                }
                s
            })
            .collect();
        let mut generics = if e.generics.is_empty() { String::new() } else { format!("[{}]", e.generics.iter().map(|g| g.name.as_str()).collect::<Vec<_>>().join(", ")) };
        // Edition 1 copies a public type only if it says so (ADR 0012).
        if self.add_copy.contains(&e.name.name) {
            generics.push_str(" is Copy");
        }
        let one_line = format!("type {}{} = {}", e.name.name, generics, variants.join(" | "));
        let indent: String = self.src[..k].rsplit('\n').next().unwrap_or("").chars().take_while(|c| c.is_whitespace()).collect();
        let mut text = String::new();
        for c in &comments {
            text.push_str(c);
            text.push('\n');
            text.push_str(&indent);
        }
        if one_line.len() + indent.len() <= 100 && comments.is_empty() {
            text.push_str(&one_line);
        } else {
            text.push_str(&format!("type {}{} =", e.name.name, generics));
            for v in &variants {
                text.push_str(&format!("\n{}    | {}", indent, v));
            }
        }
        self.rep(k, end, text);
    }

    // -----------------------------------------------------------------------
    // Statements

    fn block(&mut self, b: &Block) {
        for s in &b.stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::Let { pat, ty, init } => {
                if let Some(t) = ty {
                    let printed = self.ty(t);
                    self.ins(pat.span.lo as usize, format!("{} ", printed));
                    self.rep(pat.span.hi as usize, t.span.hi as usize, "");
                }
                self.pat(pat);
                self.expr(init);
            }
            StmtKind::Var { name, ty, init } => {
                if let Some(t) = ty {
                    let printed = self.ty(t);
                    self.rep(name.span.hi as usize, t.span.hi as usize, "");
                    self.ins(name.span.lo as usize, format!("{} ", printed));
                }
                if let Some(e) = init {
                    self.expr(e);
                }
            }
            StmtKind::Assign { target, value, .. } => {
                self.expr(target);
                self.expr(value);
            }
            StmtKind::Expr(e) => self.expr(e),
            StmtKind::While { label, cond, body } => {
                let lo = s.span.lo as usize;
                self.label(label);
                let kw = self.find(lo, body.span.lo as usize, "while").unwrap_or(lo);
                // `while let P = e { … }` was parsed as `while true { if let P = e { … } else { break } }`.
                if self.text(cond.span.lo as usize, cond.span.hi as usize).starts_with("while") {
                    if let [Stmt { kind: StmtKind::Expr(Expr { kind: ExprKind::If { cond: Cond::Let(p, e, mutable), then, .. }, .. }), .. }] = body.stmts.as_slice() {
                        let inner = self.capture(|m| {
                            m.pat(p);
                            m.expr(e);
                        });
                        let pt = self.render(p.span.lo as usize, p.span.hi as usize, &inner);
                        let et = self.render(e.span.lo as usize, e.span.hi as usize, &inner);
                        let bang = if *mutable { "!" } else { "" };
                        self.rep(kw, then.span.lo as usize, format!("while {}{} is {} ", et, bang, pt));
                        self.block(then);
                        return;
                    }
                }
                if matches!(cond.kind, ExprKind::Lit(Lit::Bool(true))) {
                    self.rep(kw, cond.span.hi as usize, "loop");
                } else {
                    self.expr(cond);
                }
                self.block(body);
            }
            StmtKind::For { label, pat, iter, body } => {
                self.label(label);
                self.pat(pat);
                match iter {
                    ForIter::Range { start, end, .. } => {
                        self.expr(start);
                        self.expr(end);
                    }
                    ForIter::Expr { expr, mutable } => {
                        if *mutable {
                            if let Some(amp) = self.rfind(pat.span.hi as usize, expr.span.lo as usize, "&") {
                                self.rep(amp, amp + 1, "");
                            }
                            self.ins(pat.span.hi as usize, "!");
                        }
                        self.expr(expr);
                    }
                }
                self.block(body);
            }
        }
    }

    /// `name: while` → `#name while`.
    fn label(&mut self, label: &Option<Ident>) {
        if let Some(l) = label {
            let lo = l.span.lo as usize;
            if let Some(colon) = self.find(l.span.hi as usize, self.src.len(), ":") {
                let after = self.skip_blank(colon + 1);
                self.rep(lo, after, format!("#{} ", l.name));
            }
        }
    }

    // -----------------------------------------------------------------------
    // Patterns

    fn is_variant_name(name: &str) -> bool {
        name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && name.chars().any(|c| c.is_ascii_lowercase())
    }

    fn pat(&mut self, p: &Pattern) {
        match &p.kind {
            PatKind::Ident(name) if Self::is_variant_name(name) => self.ins(p.span.lo as usize, "."),
            PatKind::Ctor { path, args } => {
                if path.len() == 1 {
                    self.ins(p.span.lo as usize, ".");
                }
                for a in args {
                    match a {
                        PatArg::Positional(q) | PatArg::Named(_, q) => self.pat(q),
                        PatArg::Rest(_) => {}
                    }
                }
            }
            PatKind::Tuple(ps) | PatKind::Or(ps) => {
                for q in ps {
                    self.pat(q);
                }
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Expressions

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Interp(parts) => {
                for p in parts {
                    if let StrPartAst::Interp { expr, .. } = p {
                        self.expr(expr);
                    }
                }
            }
            ExprKind::Try(x) => {
                // Edition 0.2 turns an `io.IoError` into its text for `?` (a transition, ADR
                // 0015); edition 1 does not.
                if self.string_errors && is_io_call(x) {
                    self.manual(
                        e.span.lo as usize,
                        "`?` turns the `io.IoError` of this call into its text only in edition 0.2: return `Result[..., io.IoError]` or `Result[..., any Error]`, or convert with `when` (ADR 0015)",
                    );
                }
                self.expr(x)
            }
            ExprKind::Paren(x) | ExprKind::Unary(_, x) | ExprKind::Field(x, _) | ExprKind::TupleField(x, _, _) | ExprKind::Move(x) => self.expr(x),
            ExprKind::Tuple(xs) | ExprKind::Array(xs) => {
                for x in xs {
                    self.expr(x);
                }
            }
            ExprKind::ArrayRepeat(a, b) | ExprKind::Binary(_, a, b) => {
                self.expr(a);
                self.expr(b);
            }
            ExprKind::Cast(x, t) => {
                self.expr(x);
                let printed = self.ty(t);
                if printed != self.text(t.span.lo as usize, t.span.hi as usize) {
                    self.rep(t.span.lo as usize, t.span.hi as usize, printed);
                }
            }
            ExprKind::Call(f, args) => {
                self.expr(f);
                for a in args {
                    if let Some(amp) = a.inout {
                        self.rep(amp.lo as usize, amp.hi as usize, "");
                        self.ins(a.expr.span.hi as usize, "!");
                    }
                    self.expr(&a.expr);
                }
            }
            ExprKind::Index(x, args) => {
                self.expr(x);
                for a in args {
                    self.expr(a);
                }
            }
            ExprKind::Type(t) => {
                let printed = self.ty(t);
                if printed != self.text(t.span.lo as usize, t.span.hi as usize) {
                    self.rep(t.span.lo as usize, t.span.hi as usize, printed);
                }
            }
            ExprKind::Return(Some(x)) => self.expr(x),
            ExprKind::Break(Some(l)) | ExprKind::Continue(Some(l)) => self.ins(l.span.lo as usize, "#"),
            ExprKind::If { cond, then, else_ } => {
                match cond {
                    Cond::Expr(c) => self.expr(c),
                    Cond::Let(p, x, mutable) => {
                        if let Some(kw) = self.find(e.span.lo as usize, p.span.lo as usize, "let") {
                            let inner = self.capture(|m| {
                                m.pat(p);
                                m.expr(x);
                            });
                            let pt = self.render(p.span.lo as usize, p.span.hi as usize, &inner);
                            let xt = self.render(x.span.lo as usize, x.span.hi as usize, &inner);
                            let bang = if *mutable { "!" } else { "" };
                            self.rep(kw, x.span.hi as usize, format!("{}{} is {}", xt, bang, pt));
                        }
                    }
                }
                self.block(then);
                if let Some(x) = else_ {
                    self.expr(x);
                }
            }
            ExprKind::Match { scrutinee, arms, mutable } => {
                let lo = e.span.lo as usize;
                self.rep(lo, lo + "match".len(), "when");
                if *mutable {
                    if let Some(amp) = self.find(lo, scrutinee.span.lo as usize, "&") {
                        self.rep(amp, amp + 1, "");
                    }
                    self.ins(scrutinee.span.hi as usize, "!");
                }
                self.expr(scrutinee);
                for (i, a) in arms.iter().enumerate() {
                    self.pat(&a.pat);
                    let before = a.guard.as_ref().map(|g| g.span.hi).unwrap_or(a.pat.span.hi) as usize;
                    if let Some(arrow) = self.find(before, a.body.span.lo as usize, "=>") {
                        self.rep(arrow, arrow + 2, "then");
                    }
                    if let Some(g) = &a.guard {
                        self.expr(g);
                    }
                    self.expr(&a.body);
                    // `A => 1, B => 2`: the arms of `when` are separated by lines or `;`.
                    let after = self.skip_blank(a.body.span.hi as usize);
                    if self.src.as_bytes().get(after) == Some(&b',') {
                        let next = self.skip_blank(after + 1);
                        let same_line = !matches!(self.src.as_bytes().get(next), Some(b'\n') | Some(b'\r') | Some(b'}') | Some(b'/') | None);
                        if same_line && i + 1 < arms.len() {
                            self.rep(after, after + 1, ";");
                        } else {
                            self.rep(after, after + 1, "");
                        }
                    }
                }
            }
            ExprKind::Block(b) | ExprKind::Unsafe(b) => self.block(b),
            ExprKind::Closure(c) => {
                let lo = e.span.lo as usize;
                let body_lo = match &c.body {
                    ClosureBody::Block(b) => b.span.lo as usize,
                    ClosureBody::Expr(x) => self.rfind(lo, x.span.lo as usize, "=>").unwrap_or(x.span.lo as usize),
                };
                let params: Vec<String> = c
                    .params
                    .iter()
                    .map(|p| {
                        let base = match &p.ty {
                            Some(t) => format!("{} {}", self.ty(t), p.name.name),
                            None => p.name.name.clone(),
                        };
                        match p.conv {
                            Convention::Inout => format!("{}!", base),
                            Convention::Owned => format!("owned {}", base),
                            Convention::Borrow => base,
                        }
                    })
                    .collect();
                let ret = c.ret.as_ref().map(|r| format!(" {}", self.ty(r))).unwrap_or_default();
                self.rep(lo, body_lo, format!("fn{}({}) ", ret, params.join(", ")));
                match &c.body {
                    ClosureBody::Block(b) => self.block(b),
                    ClosureBody::Expr(x) => self.expr(x),
                }
            }
            _ => {}
        }
    }
}

/// Renames the names that edition 1 reserves (`repeat` → `repeat_`) in the source, when the
/// name is never used after a `.` (a field or a method, which other files may use): the
/// renamed source, the names renamed, and the places where renaming is not safe.
fn rename_reserved(src: &str) -> (String, Vec<String>, Vec<(usize, String)>) {
    use nxc_syntax::token::TokenKind as K;
    let mut sm = SourceMap::new();
    let file = sm.add("rename.nx", src);
    let diags = DiagSink::new();
    // The tokens of the interpolations (`"{x}"`) count too.
    fn flatten(toks: Vec<nxc_syntax::token::Token>, out: &mut Vec<nxc_syntax::token::Token>) {
        for t in toks {
            if let K::Str(s) = &t.kind {
                for p in &s.parts {
                    if let nxc_syntax::token::StrPart::Interp { tokens, .. } = p {
                        flatten(tokens.clone(), out);
                    }
                }
            }
            out.push(t);
        }
    }
    // The methods and fields declared here: edition 1 accepts a reserved word as the name
    // of a member (ADR 0010, Q2), so they keep their names, and so do their uses after a dot
    // and as named arguments (`Sac(any: 3)`).
    let mut members: HashSet<String> = HashSet::new();
    let mut member_decls: HashSet<u32> = HashSet::new();
    {
        let d = DiagSink::new();
        let module = parser::parse_module(lexer::lex(src, file, &d), &d);
        for item in &module.items {
            let methods: &[Item] = match &item.kind {
                ItemKind::Impl(i) => &i.items,
                ItemKind::Trait(t) => &t.items,
                ItemKind::Struct(st) => {
                    for fd in &st.fields {
                        members.insert(fd.name.name.clone());
                        member_decls.insert(fd.name.span.lo);
                    }
                    &[]
                }
                _ => &[],
            };
            for m in methods {
                if let ItemKind::Fn(f) = &m.kind {
                    members.insert(f.name.name.clone());
                    member_decls.insert(f.name.span.lo);
                }
            }
        }
    }
    let mut toks = Vec::new();
    flatten(lexer::lex(src, file, &diags), &mut toks);
    toks.retain(|t| !matches!(t.kind, K::Str(_)));
    toks.sort_by_key(|t| t.span.lo);
    let is_name = |i: usize| -> bool {
        let K::Ident(n) = &toks[i].kind else { return false };
        // `@test` is the attribute, not a name.
        NEW_WORDS.contains(&n.as_str()) && !(n == "test" && i > 0 && toks[i - 1].kind == K::At)
    };
    let mut after_dot: Vec<String> = Vec::new();
    for i in 0..toks.len() {
        if is_name(i) && i > 0 && toks[i - 1].kind == K::Dot {
            if let K::Ident(n) = &toks[i].kind {
                after_dot.push(n.clone());
            }
        }
    }
    let mut out = String::with_capacity(src.len());
    let mut at = 0;
    let mut renamed: Vec<String> = Vec::new();
    let mut unsafe_words = Vec::new();
    for i in 0..toks.len() {
        if !is_name(i) {
            continue;
        }
        let K::Ident(n) = &toks[i].kind else { continue };
        let (lo, hi) = (toks[i].span.lo as usize, toks[i].span.hi as usize);
        // After a dot (`s.repeat(3)`, a method of the library) the word stays valid.
        if i > 0 && toks[i - 1].kind == K::Dot {
            continue;
        }
        // A method or a field declared here, or a named argument (`Sac(any: 3)`), keeps its
        // name (ADR 0010, Q2).
        if member_decls.contains(&(lo as u32)) || (members.contains(n) && toks.get(i + 1).is_some_and(|t| t.kind == K::Colon)) {
            continue;
        }
        // A field or a method declared elsewhere and used after a dot: renaming only the
        // declaration would break its uses.
        let declares = (i > 0 && toks[i - 1].kind == K::Fn) || toks.get(i + 1).is_some_and(|t| t.kind == K::Colon);
        if declares && after_dot.contains(n) {
            unsafe_words.push((lo, n.clone()));
            continue;
        }
        out.push_str(&src[at..hi]);
        out.push('_');
        at = hi;
        if !renamed.contains(n) {
            renamed.push(n.clone());
        }
    }
    out.push_str(&src[at..]);
    unsafe_words.dedup_by(|a, b| a.1 == b.1);
    (out, renamed, unsafe_words)
}
/// The public structs and enums of the module that edition 0.2 copies by their structure
/// and that do not declare `Copy` yet: edition 1 copies them only with `is Copy` (ADR
/// 0012). Without the types of other modules, a value of a type not declared here counts
/// as a resource (apart from `Option`, `Result` and `Ordering`): such a type stays a
/// resource, and a copy of it is reported by the compiler (E0402), never silently changed.
fn copy_types(module: &Module) -> HashSet<String> {
    struct Decl<'a> {
        is_pub: bool,
        generics: Vec<String>,
        values: Vec<&'a TypeExpr>,
        indirect: bool,
    }
    let mut decls: HashMap<String, Decl> = HashMap::new();
    let mut not_copy: HashSet<String> = HashSet::new();
    let mut declared: HashSet<String> = HashSet::new();
    for item in &module.items {
        if item.attrs.iter().any(|a| a.name.name == "derive" && a.args.iter().any(|(l, _)| matches!(l, Lit::Str(s) if s == "Copy"))) {
            if let ItemKind::Struct(StructDecl { name, .. }) | ItemKind::Enum(EnumDecl { name, .. }) = &item.kind {
                declared.insert(name.name.clone());
            }
        }
        match &item.kind {
            ItemKind::Struct(s) => {
                let d = Decl {
                    is_pub: item.is_pub,
                    generics: s.generics.iter().map(|g| g.name.clone()).collect(),
                    values: s.fields.iter().map(|f| &f.ty).collect(),
                    indirect: s.fields.iter().any(|f| f.indirect),
                };
                decls.insert(s.name.name.clone(), d);
            }
            ItemKind::Enum(e) => {
                let d = Decl {
                    is_pub: item.is_pub,
                    generics: e.generics.iter().map(|g| g.name.clone()).collect(),
                    values: e.variants.iter().flat_map(|v| v.fields.iter()).collect(),
                    indirect: e.variants.iter().any(|v| v.indirect),
                };
                decls.insert(e.name.name.clone(), d);
            }
            ItemKind::Impl(i) => {
                let TypeExprKind::Path { segments, .. } = &i.ty.kind else { continue };
                let Some(name) = segments.last().map(|s| s.name.clone()) else { continue };
                match &i.trait_ {
                    Some(t) if matches!(&t.kind, TypeExprKind::Path { segments, .. } if segments.last().is_some_and(|s| s.name == "Copy")) => {
                        declared.insert(name);
                    }
                    None if i.items.iter().any(|it| matches!(&it.kind, ItemKind::Fn(f) if f.name.name == "drop")) => {
                        not_copy.insert(name);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    // The greatest set of copyable types: remove the types with a value that is not.
    let mut copy: HashSet<String> = decls.iter().filter(|(n, d)| !d.indirect && !not_copy.contains(*n)).map(|(n, _)| n.clone()).collect();
    loop {
        let out: Vec<String> = copy.iter().filter(|n| !decls[*n].values.iter().all(|t| copyable(t, &decls[*n].generics, &copy))).cloned().collect();
        if out.is_empty() {
            break;
        }
        for n in out {
            copy.remove(&n);
        }
    }
    copy.into_iter().filter(|n| decls[n].is_pub && !declared.contains(n)).collect()
}

/// Is a value of type `t` copied in edition 0.2, given the copyable types of the module?
fn copyable(t: &TypeExpr, generics: &[String], copy: &HashSet<String>) -> bool {
    const PRIMITIVES: &[&str] = &["i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize", "f32", "f64", "bool", "char", "Ptr", "Ordering"];
    match &t.kind {
        TypeExprKind::Path { segments, args } if segments.len() == 1 => {
            let n = segments[0].name.as_str();
            let args_copy = args.iter().all(|a| copyable(a, generics, copy));
            if n == "Ptr" || (PRIMITIVES.contains(&n) && args.is_empty()) || generics.iter().any(|g| g == n) {
                true
            } else if n == "Option" || n == "Result" {
                args_copy
            } else {
                copy.contains(n) && args_copy
            }
        }
        TypeExprKind::Unit | TypeExprKind::Fn(..) => true,
        TypeExprKind::Tuple(xs) => xs.iter().all(|x| copyable(x, generics, copy)),
        TypeExprKind::Array(e, _) => copyable(e, generics, copy),
        _ => false,
    }
}

/// `Result[..., String]`.
fn is_result_of_string(t: &TypeExpr) -> bool {
    let TypeExprKind::Path { segments, args } = &t.kind else { return false };
    let is_string = |t: &TypeExpr| matches!(&t.kind, TypeExprKind::Path { segments, args } if args.is_empty() && segments.len() == 1 && segments[0].name == "String");
    segments.last().is_some_and(|s| s.name == "Result") && args.len() == 2 && is_string(&args[1])
}

/// A call of a fallible function of `std.io`: `io.read_file(...)`.
fn is_io_call(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Paren(x) => is_io_call(x),
        ExprKind::Call(f, _) => match &f.kind {
            ExprKind::Field(m, name) => matches!(&m.kind, ExprKind::Ident(m) if m == "io") && IO_FALLIBLE.contains(&name.name.as_str()),
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(src: &str) -> String {
        let r = migrate("t.nx", src).unwrap();
        assert!(r.manual.is_empty(), "{:?}\n{}", r.manual, r.text);
        r.text
    }

    #[test]
    fn declarations() {
        assert_eq!(m("fn carre(x: i32) -> i32 {\n    return x * x\n}\n"), "fn i32 carre(i32 x) {\n    return x * x\n}\n");
        assert_eq!(m("struct P {\n    pub x: f64\n}\n"), "type P {\n    pub f64 x\n}\n");
        assert_eq!(m("enum C {\n    Rouge\n    Rgb(u8, u8)\n}\n"), "type C = .Rouge | .Rgb(u8, u8)\n");
        assert_eq!(
            m("impl P {\n    fn bouge(inout self, owned n: String, inout q: P) {\n    }\n}\n"),
            "extend P {\n    fn bouge(self!, owned String n, P q!) {\n    }\n}\n"
        );
        assert_eq!(m("trait T: A + B {\n    fn f(self) -> dyn A\n}\n"), "interface T is A and B {\n    fn any A f(self)\n}\n");
        assert_eq!(m("fn g[T: Ord + Clone](v: Vec[T]) -> fn(i32) -> i32 {\n}\n"), "fn fn i32(i32) g[T is Ord and Clone](Vec[T] v) {\n}\n");
        assert_eq!(m("@test\nfn ok() {\n}\n"), "test \"ok\" {\n}\n");
        assert_eq!(m("const MAX: i32 = 3\n"), "const i32 MAX = 3\n");
    }

    #[test]
    fn statements_and_expressions() {
        let src = "fn f(c: C) {\n    let x: i32 = 1 // garde\n    var v: Vec[i32] = Vec.new()\n    doubler(&v)\n    match c {\n        Rouge => 1, Vert => 2\n        Rgb(r, _) if r > 2 => 3\n        _ => 0\n    }\n    if let Some(n) = trouve(x) {\n    }\n    while let Some(y) = v.pop() {\n    }\n    exterieur: while true {\n        break exterieur\n    }\n    for e in &v {\n    }\n    let g = fn(a: i32) -> i32 => a + 1\n}\n";
        let out = m(src);
        assert_eq!(
            out,
            "fn f(C c) {\n    let i32 x = 1 // garde\n    var Vec[i32] v = Vec.new()\n    doubler(v!)\n    when c {\n        .Rouge then 1; .Vert then 2\n        .Rgb(r, _) if r > 2 then 3\n        _ then 0\n    }\n    if trouve(x) is .Some(n) {\n    }\n    while v.pop() is .Some(y) {\n    }\n    #exterieur loop {\n        break #exterieur\n    }\n    for e! in v {\n    }\n    let g = fn i32(i32 a) => a + 1\n}\n"
        );
    }

    #[test]
    fn reports_what_is_left() {
        let r = migrate("t.nx", "fn main() {\n    let test = 1\n    println(\"{test}\")\n}\n").unwrap();
        assert_eq!(r.text, "fn main() {\n    let test_ = 1\n    println(\"{test_}\")\n}\n");
        assert_eq!(r.renamed, vec!["test"]);
        // A field declared here keeps its name, a reserved word of edition 1 (ADR 0010, Q2).
        let r = migrate("t.nx", "struct S {\n    with: i32\n}\nfn f(s: S) -> i32 {\n    return s.with\n}\n").unwrap();
        assert!(r.manual.is_empty(), "{:?}", r.manual);
        assert!(r.text.contains("    i32 with\n") && r.text.contains("s.with"), "{}", r.text);
        // A local name is renamed; a method of the library after a dot is kept.
        let r = migrate("t.nx", "fn main() {\n    let with = 1\n    x.with()\n}\n").unwrap();
        assert_eq!(r.text, "fn main() {\n    let with_ = 1\n    x.with()\n}\n");
        // `?` from `std.io` into `String` errors: converted by edition 0.2 only (ADR 0015).
        let src = "fn lire(p: String) -> Result[String, String] {\n    let t = io.read_file(p)?\n    let n = autre(p)?\n    return Ok(t)\n}\nfn ok(p: String) -> Result[String, io.IoError] {\n    return Ok(io.read_file(p)?)\n}\n";
        let r = migrate("t.nx", src).unwrap();
        assert_eq!(r.manual.len(), 1, "{:?}", r.manual);
        assert!(r.manual[0].0 == 2 && r.manual[0].1.contains("ADR 0015"), "{:?}", r.manual);
    }

    #[test]
    fn public_copyable_types_declare_copy() {
        // ADR 0012: what 0.2 copies by structure says `is Copy` in edition 1.
        assert_eq!(m("pub struct P {\n    x: i32\n}\n"), "pub type P is Copy {\n    i32 x\n}\n");
        assert_eq!(m("pub enum D {\n    N\n    S\n}\n"), "pub type D is Copy = .N | .S\n");
        assert_eq!(m("pub struct G[T] {\n    v: Option[T]\n    i: In\n}\nstruct In {\n    p: (u8, [i32; 2])\n}\n"), "pub type G[T] is Copy {\n    Option[T] v\n    In i\n}\ntype In {\n    (u8, [i32; 2]) p\n}\n");
        // Resources, private types, and types that already say it: unchanged.
        assert_eq!(m("pub struct S {\n    s: String\n}\n"), "pub type S {\n    String s\n}\n");
        assert_eq!(m("struct Q {\n    x: i32\n}\n"), "type Q {\n    i32 x\n}\n");
        assert_eq!(m("pub struct H {\n    x: i32\n}\nimpl H {\n    fn drop(inout self) {\n    }\n}\n"), "pub type H {\n    i32 x\n}\nextend H {\n    fn drop(self!) {\n    }\n}\n");
        assert_eq!(m("@derive(Copy)\npub struct C {\n    x: i32\n}\n"), "@derive(Copy)\npub type C {\n    i32 x\n}\n");
    }

    #[test]
    fn unsafe_declarations_of_task_properties() {
        // ADR 0013: `unsafe impl Transfer for T` becomes `unsafe extend T is Transfer`.
        assert_eq!(m("unsafe impl Transfer for H {\n}\n"), "unsafe extend H is Transfer {\n}\n");
    }
}

//! G0 census of the syntax of NEXUL 0.2 (`docs/adr/syntaxe/g0-audit.md`): counts each
//! syntactic form in the repository with the real lexer and parser, by family of sources
//! (standard library, examples, run tests, UI tests, other tests, NEXUL blocks of the
//! Markdown documentation). It reads files only.
//!
//! usage: cargo run -p nxc_syntax --example g0_census -- <repository root>
//!
//! The report (Markdown, in French like the documentation) goes to the standard output.

use nxc_syntax::ast::*;
use nxc_syntax::token::{StrPart, TokenKind};
use nxc_syntax::{lexer, parser, DiagSink, FileId, SourceMap, Span};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const CATS: [&str; 6] = ["lib", "examples", "tests/run", "tests/ui", "tests/autres", "docs"];

#[derive(Default)]
struct Census {
    /// key -> per category count
    counts: BTreeMap<String, [u64; CATS.len()]>,
    /// key -> a few locations
    places: BTreeMap<String, Vec<String>>,
    files: [u64; CATS.len()],
    lines: [u64; CATS.len()],
    code_lines: [u64; CATS.len()],
    tokens: [u64; CATS.len()],
    syntax_errors: [u64; CATS.len()],
    unparsed_blocks: Vec<String>,
}

struct Ctx<'a> {
    c: &'a mut Census,
    cat: usize,
    sm: &'a SourceMap,
    name: String,
    /// Line offset for blocks extracted from Markdown.
    line0: usize,
}

impl Ctx<'_> {
    fn add(&mut self, key: &str, span: Span) {
        self.c.counts.entry(key.to_string()).or_default()[self.cat] += 1;
        let places = self.c.places.entry(key.to_string()).or_default();
        if places.len() < 4 {
            let (line, _) = self.sm.get(span.file).line_col(span.lo);
            places.push(format!("{}:{}", self.name, line + self.line0));
        }
    }

    fn module(&mut self, m: &Module) {
        if let Some((_, span)) = &m.decl {
            self.add("élément: module (déclaration)", *span);
        }
        for i in &m.imports {
            let k = match &i.kind {
                ImportKind::Module { alias: None } => "import: module",
                ImportKind::Module { alias: Some(_) } => "import: module as alias",
                ImportKind::Items(list) => {
                    if list.iter().any(|(_, a)| a.is_some()) {
                        self.add("import: élément renommé (x as y)", i.span);
                    }
                    "import: liste .{a, b}"
                }
            };
            self.add(k, i.span);
        }
        for it in &m.items {
            self.item(it, None);
        }
    }

    fn item(&mut self, it: &Item, owner: Option<&str>) {
        for a in &it.attrs {
            let k = if a.args.is_empty() { format!("attribut: @{}", a.name.name) } else { format!("attribut: @{}(…)", a.name.name) };
            self.add(&k, a.span);
        }
        if it.is_pub {
            self.add(if owner.is_some() { "pub: méthode" } else { "pub: élément" }, it.span);
        }
        match &it.kind {
            ItemKind::Fn(f) => self.fn_decl(f, owner),
            ItemKind::Struct(s) => {
                self.add("élément: struct", it.span);
                if !s.generics.is_empty() {
                    self.add("générique: struct paramétrée", it.span);
                }
                for f in &s.fields {
                    self.add("struct: champ", f.span);
                    if f.is_pub {
                        self.add("struct: champ pub", f.span);
                    }
                    if f.indirect {
                        self.add("struct: champ indirect", f.span);
                    }
                    self.ty(&f.ty);
                }
            }
            ItemKind::Enum(e) => {
                self.add("élément: enum", it.span);
                if !e.generics.is_empty() {
                    self.add("générique: enum paramétrée", it.span);
                }
                for v in &e.variants {
                    self.add("enum: variante", v.span);
                    if !v.fields.is_empty() {
                        self.add("enum: variante avec données", v.span);
                    }
                    if v.indirect {
                        self.add("enum: variante indirect", v.span);
                    }
                    for t in &v.fields {
                        self.ty(t);
                    }
                }
            }
            ItemKind::Impl(i) => {
                self.add(if i.trait_.is_some() { "élément: impl Trait for Type" } else { "élément: impl Type (inhérent)" }, it.span);
                if !i.bounds.is_empty() {
                    self.add("générique: impl avec bornes sur les paramètres du type", it.span);
                }
                self.ty(&i.ty);
                if let Some(t) = &i.trait_ {
                    self.ty(t);
                }
                for m in &i.items {
                    self.item(m, Some("impl"));
                }
            }
            ItemKind::Trait(t) => {
                self.add("élément: trait", it.span);
                if !t.supertraits.is_empty() {
                    self.add("trait: avec super-traits", it.span);
                }
                for m in &t.items {
                    self.item(m, Some("trait"));
                }
            }
            ItemKind::Const(c) => {
                self.add("élément: const", it.span);
                self.ty(&c.ty);
                self.expr(&c.value);
            }
            ItemKind::Static(s) => {
                self.add(if s.mutable { "élément: static var" } else { "élément: static" }, it.span);
                self.ty(&s.ty);
                self.expr(&s.value);
            }
            ItemKind::TypeAlias(t) => {
                self.add("élément: type (alias)", it.span);
                self.ty(&t.ty);
            }
            ItemKind::ExternFn(e) => {
                self.add(&format!("élément: extern \"{}\" fn", e.abi), it.span);
                for (_, t) in &e.params {
                    self.ty(t);
                }
                if let Some(t) = &e.ret {
                    self.ty(t);
                }
            }
        }
    }

    fn fn_decl(&mut self, f: &FnDecl, owner: Option<&str>) {
        let k = match owner {
            None => "fonction: libre",
            Some("impl") => "fonction: méthode ou fonction associée (impl)",
            _ => "fonction: dans un trait",
        };
        self.add(k, f.span);
        if f.is_unsafe {
            self.add("fonction: unsafe fn", f.span);
        }
        if f.body.is_none() {
            self.add("fonction: sans corps", f.span);
        }
        if !f.generics.is_empty() {
            self.add("générique: fonction paramétrée", f.span);
        }
        if !f.bounds.is_empty() {
            self.add("générique: fonction avec bornes (inline ou where)", f.span);
        }
        if f.ret.is_some() {
            self.add("fonction: type de retour (->)", f.span);
        }
        for p in &f.params {
            let k = match (&p.kind, p.conv) {
                (ParamKind::SelfParam, Convention::Borrow) => "paramètre: self",
                (ParamKind::SelfParam, Convention::Inout) => "paramètre: inout self",
                (ParamKind::SelfParam, Convention::Owned) => "paramètre: owned self",
                (ParamKind::Named { .. }, Convention::Borrow) => "paramètre: nom: T (emprunt en lecture)",
                (ParamKind::Named { .. }, Convention::Inout) => "paramètre: nom: inout T",
                (ParamKind::Named { .. }, Convention::Owned) => "paramètre: nom: owned T",
            };
            self.add(k, p.span);
            if let ParamKind::Named { ty, .. } = &p.kind {
                self.ty(ty);
            }
        }
        if let Some(t) = &f.ret {
            self.ty(t);
        }
        if let Some(b) = &f.body {
            self.block(b);
        }
    }

    fn ty(&mut self, t: &TypeExpr) {
        match &t.kind {
            TypeExprKind::Path { segments, args } => {
                if segments.len() > 1 {
                    self.add("type: chemin qualifié (a.B)", t.span);
                }
                if !args.is_empty() {
                    self.add("type: générique appliqué T[A]", t.span);
                }
                for a in args {
                    self.ty(a);
                }
            }
            TypeExprKind::SelfType => self.add("type: Self", t.span),
            TypeExprKind::Array(e, n) => {
                self.add("type: tableau [T; N]", t.span);
                self.ty(e);
                self.expr(n);
            }
            TypeExprKind::Unit => self.add("type: ()", t.span),
            TypeExprKind::Tuple(v) => {
                self.add("type: tuple", t.span);
                for x in v {
                    self.ty(x);
                }
            }
            TypeExprKind::Fn(p, r) => {
                self.add("type: fn(…) -> T", t.span);
                for (c, x) in p {
                    if *c != Convention::Borrow {
                        self.add("type: fn avec convention inout/owned", x.span);
                    }
                    self.ty(x);
                }
                if let Some(r) = r {
                    self.ty(r);
                }
            }
            TypeExprKind::Dyn(x) => {
                self.add("type: dyn Trait", t.span);
                self.ty(x);
            }
            TypeExprKind::Error => {}
        }
    }

    fn block(&mut self, b: &Block) {
        for s in &b.stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::Let { pat, ty, init } => {
                self.add("instruction: let", s.span);
                if !matches!(pat.kind, PatKind::Ident(_) | PatKind::Wild) {
                    self.add("instruction: let avec motif déstructurant", s.span);
                }
                if let Some(t) = ty {
                    self.add("instruction: let avec type", s.span);
                    self.ty(t);
                }
                self.pat(pat);
                self.expr(init);
            }
            StmtKind::Var { ty, init, .. } => {
                self.add("instruction: var", s.span);
                if ty.is_some() {
                    self.add("instruction: var avec type", s.span);
                }
                if init.is_none() {
                    self.add("instruction: var sans valeur initiale", s.span);
                }
                if let Some(t) = ty {
                    self.ty(t);
                }
                if let Some(e) = init {
                    self.expr(e);
                }
            }
            StmtKind::Assign { op, target, value, .. } => {
                let k = match op {
                    AssignOp::Assign => "affectation: =".to_string(),
                    AssignOp::Compound(b) => format!("affectation: {}=", b.symbol()),
                };
                self.add(&k, s.span);
                self.expr(target);
                self.expr(value);
            }
            StmtKind::Expr(e) => {
                self.add("instruction: expression", s.span);
                self.expr(e);
            }
            StmtKind::While { label, cond, body } => {
                // `while let` is desugared by the parser into `while true { if let … }`.
                if label.is_some() {
                    self.add("boucle: étiquette (nom: while/for)", s.span);
                }
                self.expr(cond);
                self.block(body);
            }
            StmtKind::For { label, pat, iter, body } => {
                if label.is_some() {
                    self.add("boucle: étiquette (nom: while/for)", s.span);
                }
                if !matches!(pat.kind, PatKind::Ident(_) | PatKind::Wild) {
                    self.add("boucle: for avec motif déstructurant", s.span);
                }
                match iter {
                    ForIter::Range { start, end, inclusive } => {
                        self.add(if *inclusive { "boucle: for x in a..=b" } else { "boucle: for x in a..b" }, s.span);
                        self.expr(start);
                        self.expr(end);
                    }
                    ForIter::Expr { expr, mutable } => {
                        self.add(if *mutable { "boucle: for x in &v (modification en place)" } else { "boucle: for x in v" }, s.span);
                        self.expr(expr);
                    }
                }
                self.pat(pat);
                self.block(body);
            }
        }
    }

    fn expr(&mut self, e: &Expr) {
        let sp = e.span;
        match &e.kind {
            ExprKind::Lit(l) => {
                let k = match l {
                    Lit::Int(_) => "littéral: entier",
                    Lit::Float(_) => "littéral: flottant",
                    Lit::Char(_) => "littéral: caractère",
                    Lit::Str(_) => "littéral: chaîne simple",
                    Lit::Bool(_) => "littéral: booléen",
                };
                self.add(k, sp);
            }
            ExprKind::Interp(parts) => {
                self.add("littéral: chaîne interpolée \"{x}\"", sp);
                for p in parts {
                    if let StrPartAst::Interp { expr, spec } = p {
                        self.add("interpolation: expression", expr.span);
                        if spec.is_some() {
                            self.add("interpolation: avec format {x:…}", expr.span);
                        }
                        if !matches!(expr.kind, ExprKind::Ident(_)) {
                            self.add("interpolation: expression complexe (pas un simple nom)", expr.span);
                        }
                        self.expr(expr);
                    }
                }
            }
            ExprKind::Ident(_) => {}
            ExprKind::SelfValue => {}
            ExprKind::Variant(_) => self.add("expression: .Variante (édition 1)", sp),
            ExprKind::SelfType => self.add("expression: Self", sp),
            ExprKind::Unit => self.add("expression: ()", sp),
            ExprKind::Paren(x) => self.expr(x),
            ExprKind::Tuple(v) => {
                self.add("expression: tuple (a, b)", sp);
                v.iter().for_each(|x| self.expr(x));
            }
            ExprKind::Array(v) => {
                self.add("expression: tableau [a, b]", sp);
                v.iter().for_each(|x| self.expr(x));
            }
            ExprKind::ArrayRepeat(a, b) => {
                self.add("expression: tableau répété [x; n]", sp);
                self.expr(a);
                self.expr(b);
            }
            ExprKind::Unary(op, x) => {
                let k = match op {
                    UnOp::Neg => "opérateur: - (préfixe)",
                    UnOp::Not => "opérateur: not",
                    UnOp::BitNot => "opérateur: ~",
                };
                self.add(k, sp);
                self.expr(x);
            }
            ExprKind::Binary(op, a, b) => {
                self.add(&format!("opérateur: {}", op.symbol()), sp);
                self.expr(a);
                self.expr(b);
            }
            ExprKind::Cast(x, t) => {
                self.add("opérateur: as (conversion)", sp);
                self.expr(x);
                self.ty(t);
            }
            ExprKind::Field(x, _) => {
                self.add("accès: .champ ou module.nom", sp);
                self.expr(x);
            }
            ExprKind::TupleField(x, _, _) => {
                self.add("accès: .0 (tuple)", sp);
                self.expr(x);
            }
            ExprKind::Call(f, args) => {
                self.add(if matches!(f.kind, ExprKind::Field(..)) { "appel: x.f(…) (méthode ou module)" } else { "appel: f(…)" }, sp);
                for a in args {
                    if a.label.is_some() {
                        self.add("appel: argument nommé (nom: valeur)", a.span);
                    }
                    if a.inout.is_some() {
                        self.add("appel: argument &x (inout)", a.span);
                    }
                    self.expr(&a.expr);
                }
                self.expr(f);
            }
            ExprKind::Index(x, v) => {
                self.add("crochets: a[i] ou instanciation générique F[T]", sp);
                if v.len() > 1 {
                    self.add("crochets: plusieurs arguments a[x, y]", sp);
                }
                self.expr(x);
                v.iter().for_each(|y| self.expr(y));
            }
            ExprKind::Try(x) => {
                self.add("opérateur: ? (propagation)", sp);
                self.expr(x);
            }
            ExprKind::Fallback(x, f) => {
                self.add("repli: valeur else (édition 1)", sp);
                self.expr(x);
                self.expr(f);
            }
            ExprKind::Move(x) => {
                self.add("opérateur: move", sp);
                self.expr(x);
            }
            ExprKind::Return(x) => {
                self.add(if x.is_some() { "contrôle: return valeur" } else { "contrôle: return" }, sp);
                if let Some(x) = x {
                    self.expr(x);
                }
            }
            ExprKind::Break(l) => self.add(if l.is_some() { "contrôle: break étiquette" } else { "contrôle: break" }, sp),
            ExprKind::Continue(l) => self.add(if l.is_some() { "contrôle: continue étiquette" } else { "contrôle: continue" }, sp),
            ExprKind::If { cond, then, else_ } => {
                match cond {
                    Cond::Expr(c) => {
                        self.add("contrôle: if", sp);
                        self.expr(c);
                    }
                    Cond::Let(p, x, m) => {
                        self.add(if *m { "contrôle: if let P = &e / while let (modification)" } else { "contrôle: if let / while let" }, sp);
                        self.pat(p);
                        self.expr(x);
                    }
                }
                self.block(then);
                if let Some(x) = else_ {
                    if matches!(x.kind, ExprKind::If { .. }) {
                        self.add("contrôle: else if", x.span);
                    } else {
                        self.add("contrôle: else", x.span);
                    }
                    self.expr(x);
                }
            }
            ExprKind::Match { scrutinee, arms, mutable } => {
                self.add(if *mutable { "contrôle: match &e (modification)" } else { "contrôle: match" }, sp);
                self.expr(scrutinee);
                for a in arms {
                    self.add("match: bras", a.span);
                    if a.guard.is_some() {
                        self.add("match: garde (if)", a.span);
                    }
                    if matches!(a.body.kind, ExprKind::Block(_)) {
                        self.add("match: bras à bloc => { … }", a.span);
                    }
                    self.pat(&a.pat);
                    if let Some(g) = &a.guard {
                        self.expr(g);
                    }
                    self.expr(&a.body);
                }
            }
            ExprKind::Block(b) => self.block(b),
            ExprKind::Unsafe(b) => {
                self.add("bloc: unsafe { … }", sp);
                self.block(b);
            }
            ExprKind::Type(t) => {
                self.add("expression: type en position d'argument", sp);
                self.ty(t);
            }
            ExprKind::Closure(c) => {
                match &c.body {
                    ClosureBody::Block(b) => {
                        self.add("fonction anonyme: fn(x) { … }", sp);
                        self.block(b);
                    }
                    ClosureBody::Expr(x) => {
                        self.add("fonction anonyme: fn(x) => expr", sp);
                        self.expr(x);
                    }
                }
                for p in &c.params {
                    if p.ty.is_some() {
                        self.add("fonction anonyme: paramètre typé", sp);
                    }
                    if p.conv != Convention::Borrow {
                        self.add("fonction anonyme: paramètre inout/owned", sp);
                    }
                }
                if c.ret.is_some() {
                    self.add("fonction anonyme: type de retour", sp);
                }
            }
            ExprKind::Error => {}
        }
    }

    fn pat(&mut self, p: &Pattern) {
        let sp = p.span;
        match &p.kind {
            PatKind::Wild => self.add("motif: _", sp),
            PatKind::Ident(_) => {}
            PatKind::Lit(_) => self.add("motif: littéral", sp),
            PatKind::Range(_, _, inc) => self.add(if *inc { "motif: intervalle a..=b" } else { "motif: intervalle a..b" }, sp),
            PatKind::Tuple(v) => {
                self.add("motif: tuple (a, b)", sp);
                v.iter().for_each(|x| self.pat(x));
            }
            PatKind::Ctor { args, .. } => {
                self.add("motif: constructeur V(…)", sp);
                for a in args {
                    match a {
                        PatArg::Positional(x) => self.pat(x),
                        PatArg::Named(_, x) => {
                            self.add("motif: champ nommé (nom: motif)", x.span);
                            self.pat(x)
                        }
                        PatArg::Rest(s) => self.add("motif: reste ..", *s),
                    }
                }
            }
            PatKind::Path(v) => {
                if v.len() > 1 {
                    self.add("motif: chemin qualifié a.B", sp);
                }
            }
            PatKind::Or(v) => {
                self.add("motif: alternative a | b", sp);
                v.iter().for_each(|x| self.pat(x));
            }
            PatKind::Error => {}
        }
    }
}

fn tok_name(k: &TokenKind) -> Option<String> {
    use TokenKind::*;
    Some(match k {
        Ident(_) | Int(_) | Float(_) | Char(_) | Str(_) | Eof => return None,
        Newline => "fin de ligne significative".into(),
        Semi => ";".into(),
        other => format!("{other:?}"),
    })
}

fn count_text(c: &mut Census, cat: usize, text: &str) {
    c.lines[cat] += text.lines().count() as u64;
    let mut in_block = 0i32;
    for l in text.lines() {
        let t = l.trim();
        if in_block > 0 {
            in_block += t.matches("/*").count() as i32 - t.matches("*/").count() as i32;
            continue;
        }
        if t.is_empty() || t.starts_with("//") {
            continue;
        }
        if t.starts_with("/*") {
            in_block = t.matches("/*").count() as i32 - t.matches("*/").count() as i32;
            continue;
        }
        c.code_lines[cat] += 1;
    }
}

fn analyse(c: &mut Census, cat: usize, name: &str, text: &str, line0: usize) -> bool {
    let mut sm = SourceMap::new();
    let file: FileId = sm.add(name, text);
    let diags = DiagSink::new();
    let toks = lexer::lex(&sm.get(file).text, file, &diags);
    let module = parser::parse_module(toks.clone(), &diags);
    let errors = diags.has_errors();
    let mut ctx = Ctx { c, cat, sm: &sm, name: name.to_string(), line0 };
    // Tokens, and a few sequences the AST does not keep.
    ctx.c.tokens[cat] += toks.len() as u64;
    for (i, t) in toks.iter().enumerate() {
        if let Some(n) = tok_name(&t.kind) {
            ctx.add(&format!("jeton: {n}"), t.span);
        }
        if let TokenKind::Str(s) = &t.kind {
            if s.parts.iter().any(|p| matches!(p, StrPart::Interp { .. })) {
                ctx.add("jeton: chaîne avec interpolation", t.span);
            }
        }
        let next = toks.get(i + 1).map(|t| &t.kind);
        match (&t.kind, next) {
            (TokenKind::While, Some(TokenKind::Let)) => ctx.add("contrôle: while let (source)", t.span),
            (TokenKind::While, _) => ctx.add("contrôle: while (source)", t.span),
            (TokenKind::Where, _) => ctx.add("générique: clause where (source)", t.span),
            (TokenKind::Semi, _) => {
                // What follows on the same line: nothing (or a comment), or another statement.
                let rest = text[t.span.hi as usize..].lines().next().unwrap_or("").trim();
                let end_of_line = rest.is_empty() || rest.starts_with("//");
                ctx.add(if end_of_line { "fin d'instruction: ; en fin de ligne" } else { "fin d'instruction: ; entre deux instructions" }, t.span);
            }
            _ => {}
        }
    }
    ctx.module(&module);
    if errors {
        c.syntax_errors[cat] += 1;
    }
    errors
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, ext: &str) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if p.is_dir() {
            if name == "target" || name == ".git" || name == "node_modules" {
                continue;
            }
            walk(&p, out, ext);
        } else if p.extension().is_some_and(|e| e == ext) {
            out.push(p);
        }
    }
}

fn category(rel: &str) -> usize {
    if rel.starts_with("lib/") {
        0
    } else if rel.starts_with("examples/") {
        1
    } else if rel.starts_with("tests/run/") {
        2
    } else if rel.starts_with("tests/ui/") {
        3
    } else if rel.starts_with("tests/") {
        4
    } else {
        5
    }
}

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("repo root"));
    let mut c = Census::default();
    let rel = |p: &Path| p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
    // .nx files.
    let mut nx = Vec::new();
    walk(&root, &mut nx, "nx");
    for p in &nx {
        let r = rel(p);
        if r.starts_with("target/") {
            continue;
        }
        let cat = category(&r);
        let text = std::fs::read_to_string(p).unwrap_or_default();
        c.files[cat] += 1;
        count_text(&mut c, cat, &text);
        analyse(&mut c, cat, &r, &text, 0);
    }
    // NEXUL blocks of the Markdown files (docs, READMEs).
    let mut md = Vec::new();
    walk(&root, &mut md, "md");
    for p in &md {
        let r = rel(p);
        // The handoff and the syntax studies may show sketches that are not NEXUL 0.2.
        if r.starts_with("target/") || r.starts_with("docs/refonte/") || r.starts_with("docs/adr/syntaxe/") {
            continue;
        }
        let text = std::fs::read_to_string(p).unwrap_or_default();
        let mut lines = text.lines().enumerate();
        while let Some((i, l)) = lines.next() {
            let t = l.trim_start();
            let lang = t.strip_prefix("```").map(str::trim);
            if !matches!(lang, Some("nx") | Some("nexul")) {
                continue;
            }
            let mut block = String::new();
            for (_, l2) in lines.by_ref() {
                if l2.trim_start().starts_with("```") {
                    break;
                }
                block.push_str(l2);
                block.push('\n');
            }
            let cat = 5;
            c.files[cat] += 1;
            count_text(&mut c, cat, &block);
            let name = r.clone();
            // Fragments: tried as a file, then inside a function.
            let mut probe = Census::default();
            if !analyse(&mut probe, cat, &name, &block, i + 1) {
                analyse(&mut c, cat, &name, &block, i + 1);
            } else {
                let wrapped = format!("fn __g0() {{\n{block}}}\n");
                let mut probe2 = Census::default();
                if !analyse(&mut probe2, cat, &name, &wrapped, i) {
                    analyse(&mut c, cat, &name, &wrapped, i);
                } else {
                    analyse(&mut c, cat, &name, &block, i + 1);
                    c.unparsed_blocks.push(format!("{r}:{}", i + 1));
                }
            }
        }
    }
    // Report.
    println!("| | {} | total |", CATS.join(" | "));
    println!("|---|{}---|", "---|".repeat(CATS.len()));
    let row = |name: &str, v: &[u64; CATS.len()]| {
        let total: u64 = v.iter().sum();
        // `|` (an operator, a pattern) would end the cell.
        let name = name.replace('|', "\\|");
        println!("| {name} | {} | {total} |", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" | "));
    };
    row("fichiers ou blocs", &c.files);
    row("lignes", &c.lines);
    row("lignes de code", &c.code_lines);
    row("jetons", &c.tokens);
    row("avec erreur de syntaxe", &c.syntax_errors);
    for (k, v) in &c.counts {
        row(k, v);
    }
    println!();
    println!("## Emplacements (constructions rares, total <= 12)");
    for (k, v) in &c.counts {
        let total: u64 = v.iter().sum();
        if total <= 12 {
            println!("- {k} ({total}) : {}", c.places[k].join(", "));
        }
    }
    println!();
    println!("## Blocs de documentation non analysables : {}", c.unparsed_blocks.len());
    for b in &c.unparsed_blocks {
        println!("- {b}");
    }
}

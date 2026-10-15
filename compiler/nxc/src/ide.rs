//! Editor services (`nxc ide`, and the language server `nxc lsp`): information on
//! hover, go to definition, completion, outline, references and rename, formatting.
//!
//! Every request analyzes the program again (the front end takes a few milliseconds),
//! from the text being edited when it is not saved yet. Positions are 1-based lines and
//! columns counted in Unicode scalar values, like the diagnostics.

use crate::json_str;
use nxc_sema::defs::{AdtId, AdtKind, ConstId, FnId, FnKind, ItemRef, ModId, Program, StaticId, TraitId};
use nxc_sema::thir::{self, Block, Body, Callee, Expr, ExprKind as T, LocalId, LocalKind, Pat, PatKind, StmtKind as S};
use nxc_sema::ty::{Ty, TyKind};
use nxc_sema::ProgramSource;
use nxc_syntax::ast::Convention;
use nxc_syntax::{DiagSink, FileId, SourceMap, Span};
use std::path::{Path, PathBuf};

/// The analyzed program and where its root file is.
pub struct Analysis {
    pub sm: SourceMap,
    pub prog: Program,
    pub file: FileId,
    pub root: ModId,
    pub dir: Option<PathBuf>,
}

/// The edition given by `nxc ide --edition` (otherwise each file and its project say).
pub static EDITION: std::sync::OnceLock<nxc_syntax::Edition> = std::sync::OnceLock::new();

/// Analyzes `path`, with `text` as its content when given (unsaved changes).
pub fn analyze(path: &Path, text: Option<String>) -> Option<Analysis> {
    let text = match text {
        Some(t) => t,
        None => std::fs::read_to_string(path).ok()?,
    };
    let edition = EDITION.get().copied().unwrap_or_else(|| crate::pkg::edition_for(path, &text));
    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "main.nx".into());
    let dir = path.parent().map(|p| if p.as_os_str().is_empty() { PathBuf::from(".") } else { p.to_path_buf() });
    let mut sm = SourceMap::new();
    let diags = DiagSink::new();
    let mut src = ProgramSource::new(name, text, dir.clone());
    src.edition = edition;
    let prog = nxc_sema::analyze(src, &mut sm, &diags);
    let root = prog.root;
    let file = prog.modules.get(root.idx())?.file;
    Some(Analysis { sm, prog, file, root, dir })
}

impl Analysis {
    pub fn text(&self) -> &str {
        &self.sm.get(self.file).text
    }

    /// Byte offset of a 1-based (line, column) position in the root file.
    pub fn offset(&self, line: usize, col: usize) -> u32 {
        offset_of(self.text(), line, col)
    }

    fn pos_json(&self, span: Span) -> String {
        match self.sm.try_get(span.file) {
            Some(f) if !span.is_dummy() => {
                let (l1, c1) = f.line_col(span.lo);
                let (l2, c2) = f.line_col(span.hi.max(span.lo));
                let path = if span.file == self.file { String::new() } else { f.name.clone() };
                format!(
                    "{{\"file\":{},\"line\":{},\"col\":{},\"end_line\":{},\"end_col\":{},\"std\":{}}}",
                    json_str(&path),
                    l1,
                    c1,
                    l2,
                    c2,
                    f.name.starts_with('<')
                )
            }
            _ => "null".into(),
        }
    }

    fn user_bodies(&self) -> Vec<&Body> {
        let mut v: Vec<&Body> = self
            .prog
            .bodies
            .values()
            .filter(|b| {
                let f = self.prog.func(b.fn_id);
                !self.prog.modules[f.module.idx()].is_std
            })
            .collect();
        v.sort_by_key(|b| b.fn_id);
        v
    }
}

pub fn offset_of(text: &str, line: usize, col: usize) -> u32 {
    let mut cur_line = 1;
    let mut off = 0usize;
    for (i, ch) in text.char_indices() {
        if cur_line == line {
            off = i;
            break;
        }
        if ch == '\n' {
            cur_line += 1;
            off = i + 1;
        }
    }
    if cur_line < line {
        return text.len() as u32;
    }
    let mut c = 1;
    let mut pos = off;
    for (i, ch) in text[off..].char_indices() {
        if c == col || ch == '\n' {
            pos = off + i;
            break;
        }
        c += 1;
        pos = off + i + ch.len_utf8();
    }
    pos as u32
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The identifier around `offset`: (start, end).
fn word_at(text: &str, offset: usize) -> Option<(usize, usize)> {
    let b = text.as_bytes();
    let mut s = offset.min(b.len());
    while s > 0 && is_ident(b[s - 1] as char) {
        s -= 1;
    }
    let mut e = offset.min(b.len());
    while e < b.len() && is_ident(b[e] as char) {
        e += 1;
    }
    if s == e {
        None
    } else {
        Some((s, e))
    }
}

/// The `///` comment lines above the line of `span`, skipping attributes.
pub fn doc_comment(sm: &SourceMap, span: Span) -> String {
    let Some(f) = sm.try_get(span.file) else { return String::new() };
    if span.is_dummy() {
        return String::new();
    }
    let mut idx = f.line_index(span.lo);
    let mut lines = Vec::new();
    while idx > 0 {
        idx -= 1;
        let t = f.line_text(idx).trim();
        if t.starts_with('@') {
            continue;
        }
        match t.strip_prefix("///") {
            Some(rest) => lines.push(rest.strip_prefix(' ').unwrap_or(rest).to_string()),
            None => break,
        }
    }
    lines.reverse();
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Walking the typed tree

pub enum Node<'a> {
    Expr(&'a Expr),
    Pat(&'a Pat),
}

pub fn walk_block<'a>(b: &'a Block, f: &mut dyn FnMut(Node<'a>)) {
    for s in &b.stmts {
        match &s.kind {
            S::Let { pat, init } => {
                walk_expr(init, f);
                walk_pat(pat, f);
            }
            S::Var { init, .. } => {
                if let Some(e) = init {
                    walk_expr(e, f);
                }
            }
            S::Assign { place, value } | S::CompoundAssign { place, value, .. } => {
                walk_expr(place, f);
                walk_expr(value, f);
            }
            S::Expr(e) => walk_expr(e, f),
            S::While { cond, body, .. } => {
                walk_expr(cond, f);
                walk_block(body, f);
            }
            S::ForRange { start, end, body, .. } => {
                walk_expr(start, f);
                walk_expr(end, f);
                walk_block(body, f);
            }
            S::ForEach { pat, iter, body, .. } => {
                walk_expr(iter, f);
                walk_pat(pat, f);
                walk_block(body, f);
            }
        }
    }
    if let Some(t) = &b.tail {
        walk_expr(t, f);
    }
}

fn walk_pat<'a>(p: &'a Pat, f: &mut dyn FnMut(Node<'a>)) {
    f(Node::Pat(p));
    match &p.kind {
        PatKind::Tuple(ps) | PatKind::Or(ps) => ps.iter().for_each(|q| walk_pat(q, f)),
        PatKind::Variant { fields, .. } | PatKind::Struct { fields, .. } => fields.iter().for_each(|q| walk_pat(q, f)),
        _ => {}
    }
}

fn walk_expr<'a>(e: &'a Expr, f: &mut dyn FnMut(Node<'a>)) {
    f(Node::Expr(e));
    match &e.kind {
        T::Unary(_, x) | T::Cast(x) | T::Field(x, _) | T::Try(x, _) | T::Move(x) | T::ArrayRepeat(x, _) | T::FnToClosure(x) | T::ToDyn(x) => walk_expr(x, f),
        T::Binary(_, a, b) | T::Index(a, b) => {
            walk_expr(a, f);
            walk_expr(b, f);
        }
        T::IndexFn { base, index, .. } => {
            walk_expr(base, f);
            walk_expr(index, f);
        }
        T::Call { callee, args } => {
            if let Callee::Indirect(c) = callee {
                walk_expr(c, f);
            }
            args.iter().for_each(|a| walk_expr(&a.expr, f));
        }
        T::Adt { fields: xs, .. } | T::Tuple(xs) | T::Array(xs) => xs.iter().for_each(|x| walk_expr(x, f)),
        T::If { cond, then, else_ } => {
            walk_expr(cond, f);
            walk_block(then, f);
            if let Some(x) = else_ {
                walk_expr(x, f);
            }
        }
        T::IfLet { pat, scrutinee, then, else_, .. } => {
            walk_expr(scrutinee, f);
            walk_pat(pat, f);
            walk_block(then, f);
            if let Some(x) = else_ {
                walk_expr(x, f);
            }
        }
        T::Match { scrutinee, arms, .. } => {
            walk_expr(scrutinee, f);
            for a in arms {
                walk_pat(&a.pat, f);
                if let Some(g) = &a.guard {
                    walk_expr(g, f);
                }
                walk_expr(&a.body, f);
            }
        }
        T::Block(b) => walk_block(b, f),
        T::Return(Some(x)) => walk_expr(x, f),
        T::Interp(parts) => {
            for p in parts {
                if let thir::InterpPart::Value(x, _) = p {
                    walk_expr(x, f);
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// What is under the cursor

/// A named entity of the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// A local variable or parameter: the body's function and the local.
    Local(FnId, LocalId),
    Fn(FnId),
    Field(AdtId, u32),
    Adt(AdtId),
    Variant(AdtId, u32),
    Const(ConstId),
    Static(StaticId),
    Trait(TraitId),
    Module(ModId),
}

/// The entity at `offset` in the root file, with the span of its occurrence.
pub fn target_at(a: &Analysis, offset: u32) -> Option<(Target, Span, Option<Ty>)> {
    let text = a.text();
    let (ws, we) = word_at(text, offset as usize)?;
    let word = &text[ws..we];
    let p = &a.prog;
    let mut best: Option<(Target, Span, Option<Ty>)> = None;
    let consider = |t: Target, span: Span, ty: Option<Ty>, best: &mut Option<(Target, Span, Option<Ty>)>| {
        if span.file != a.file || span.lo > offset || span.hi < offset {
            return;
        }
        if best.as_ref().map_or(true, |(_, s, _)| span.len() < s.len()) {
            *best = Some((t, span, ty));
        }
    };
    let wspan = Span::new(a.file, ws as u32, we as u32);
    for body in a.user_bodies() {
        // Declarations of locals and parameters.
        for (i, l) in body.locals.iter().enumerate() {
            if l.span.file == a.file && l.span.lo as usize <= ws && we <= l.span.hi as usize && l.name == word {
                consider(Target::Local(body.fn_id, i as LocalId), l.span, Some(l.ty), &mut best);
            }
        }
        let mut visit = |n: Node| match n {
            Node::Expr(e) => {
                if e.span.file != a.file || e.span.lo > offset || e.span.hi < offset {
                    return;
                }
                match &e.kind {
                    T::Local(l) if body.locals[*l as usize].name == word => consider(Target::Local(body.fn_id, *l), e.span, Some(e.ty), &mut best),
                    T::Call { callee: Callee::Fn(f, _), .. } if p.func(*f).name == word => consider(Target::Fn(*f), wspan, None, &mut best),
                    T::FnRef(f) if p.func(*f).name == word => consider(Target::Fn(*f), e.span, Some(e.ty), &mut best),
                    T::Field(base, i) => {
                        if let Some((adt, _)) = p.tcx.as_adt(base.ty) {
                            if p.adt(adt).fields().get(*i as usize).map_or(false, |fd| fd.name == word) && !(base.span.lo as usize <= ws && we <= base.span.hi as usize) {
                                consider(Target::Field(adt, *i), wspan, Some(e.ty), &mut best);
                            }
                        }
                    }
                    T::Adt { adt, variant, .. } => {
                        let d = p.adt(*adt);
                        match variant {
                            Some(v) if d.variants()[*v as usize].name == word => consider(Target::Variant(*adt, *v), wspan, Some(e.ty), &mut best),
                            _ if d.name == word => consider(Target::Adt(*adt), wspan, Some(e.ty), &mut best),
                            _ => {}
                        }
                    }
                    T::Const(c) if p.consts[c.idx()].name == word => consider(Target::Const(*c), e.span, Some(e.ty), &mut best),
                    T::Static(s) if p.statics[s.idx()].name == word => consider(Target::Static(*s), e.span, Some(e.ty), &mut best),
                    _ => {}
                }
            }
            Node::Pat(pt) => {
                if let PatKind::Binding(l) = pt.kind {
                    if body.locals[l as usize].name == word {
                        consider(Target::Local(body.fn_id, l), pt.span, Some(pt.ty), &mut best);
                    }
                }
                if let PatKind::Variant { adt, variant, .. } = pt.kind {
                    if p.adt(adt).variants()[variant as usize].name == word {
                        consider(Target::Variant(adt, variant), wspan, Some(pt.ty), &mut best);
                    }
                }
            }
        };
        walk_block(&body.block, &mut visit);
    }
    if best.is_some() {
        return best;
    }
    // Declarations of items, and names written in types.
    for (i, f) in p.fns.iter().enumerate() {
        if f.span.file == a.file && f.span.lo as usize <= ws && we <= f.span.hi as usize {
            return Some((Target::Fn(FnId(i as u32)), f.span, None));
        }
    }
    for (i, d) in p.adts.iter().enumerate() {
        if d.span.file == a.file && d.span.lo as usize <= ws && we <= d.span.hi as usize {
            return Some((Target::Adt(AdtId(i as u32)), d.span, None));
        }
        for (j, fd) in d.fields().iter().enumerate() {
            if fd.span.file == a.file && fd.span.lo as usize <= ws && we <= fd.span.hi as usize {
                return Some((Target::Field(AdtId(i as u32), j as u32), fd.span, Some(fd.ty)));
            }
        }
        for (j, v) in d.variants().iter().enumerate() {
            if v.span.file == a.file && v.span.lo as usize <= ws && we <= v.span.hi as usize && v.name == word {
                return Some((Target::Variant(AdtId(i as u32), j as u32), v.span, None));
            }
        }
    }
    for (i, t) in p.traits.iter().enumerate() {
        if t.span.file == a.file && t.span.lo as usize <= ws && we <= t.span.hi as usize {
            return Some((Target::Trait(TraitId(i as u32)), t.span, None));
        }
    }
    let item = nxc_sema::collect::lookup_item(p, a.root, word)?;
    let t = match item {
        ItemRef::Fn(f) => Target::Fn(f),
        ItemRef::Adt(d) => Target::Adt(d),
        ItemRef::Variant(d, v) => Target::Variant(d, v),
        ItemRef::Const(c) => Target::Const(c),
        ItemRef::Static(s) => Target::Static(s),
        ItemRef::Trait(t) => Target::Trait(t),
        ItemRef::Module(m) => Target::Module(m),
        ItemRef::Alias(al) => match p.tcx.as_adt(p.aliases[al.idx()].ty) {
            Some((d, _)) => Target::Adt(d),
            None => return None,
        },
        ItemRef::Builtin(_) => return None,
    };
    Some((t, wspan, None))
}

/// Where an entity is declared.
pub fn definition_span(a: &Analysis, t: Target) -> Span {
    let p = &a.prog;
    match t {
        Target::Local(f, l) => p.bodies.get(&f).map(|b| b.locals[l as usize].span).unwrap_or(Span::DUMMY),
        Target::Fn(f) => p.func(f).span,
        Target::Field(d, i) => p.adt(d).fields()[i as usize].span,
        Target::Adt(d) => p.adt(d).span,
        Target::Variant(d, v) => p.adt(d).variants()[v as usize].span,
        Target::Const(c) => p.consts[c.idx()].span,
        Target::Static(s) => p.statics[s.idx()].span,
        Target::Trait(t) => p.traits[t.idx()].span,
        Target::Module(m) => Span::new(p.modules[m.idx()].file, 0, 0),
    }
}

/// A function's signature as written in NEXUL.
pub fn fn_signature(p: &Program, f: FnId) -> String {
    let fd = p.func(f);
    let names = &fd.generics;
    let own: Vec<&String> = names[fd.n_owner_generics..].iter().collect();
    let mut s = String::new();
    if fd.is_unsafe {
        s.push_str("unsafe ");
    }
    s.push_str("fn ");
    s.push_str(&fd.qualified_name(p));
    if !own.is_empty() {
        let mut gs = Vec::new();
        for (k, g) in own.iter().enumerate() {
            let idx = (fd.n_owner_generics + k) as u32;
            let bounds: Vec<String> = fd.bounds.iter().filter(|b| b.param == idx).map(|b| p.traits[b.trait_id.idx()].name.clone()).collect();
            gs.push(if bounds.is_empty() { g.to_string() } else { format!("{}: {}", g, bounds.join(" + ")) });
        }
        s.push_str(&format!("[{}]", gs.join(", ")));
    }
    let params: Vec<String> = fd
        .params
        .iter()
        .skip(fd.closure_captures())
        .map(|prm| {
            let conv = match prm.conv {
                Convention::Borrow => "",
                Convention::Inout => "inout ",
                Convention::Owned => "owned ",
            };
            if prm.name == "self" {
                format!("{}self", conv)
            } else {
                format!("{}{}: {}", conv, prm.name, p.ty_str_with(prm.ty, names))
            }
        })
        .collect();
    s.push_str(&format!("({})", params.join(", ")));
    if fd.ret != p.tcx.unit {
        s.push_str(&format!(" -> {}", p.ty_str_with(fd.ret, names)));
    }
    s
}

/// Hover text: a code line and the documentation.
pub fn describe(a: &Analysis, t: Target, ty: Option<Ty>) -> (String, String) {
    let p = &a.prog;
    let doc_of = |span: Span| doc_comment(&a.sm, span);
    match t {
        Target::Local(f, l) => {
            let Some(b) = p.bodies.get(&f) else { return (String::new(), String::new()) };
            let d = &b.locals[l as usize];
            let names = &p.func(f).generics;
            let kw = match d.kind {
                LocalKind::Param(Convention::Inout) => "inout ",
                LocalKind::Param(Convention::Owned) => "owned ",
                LocalKind::Param(_) => "",
                LocalKind::Var => "var ",
                LocalKind::RefMut => "var ",
                _ => "let ",
            };
            let what = if matches!(d.kind, LocalKind::Param(_)) { "parameter" } else { "variable" };
            (format!("{}{}: {}", kw, d.name, p.ty_str_with(ty.unwrap_or(d.ty), names)), what.to_string())
        }
        Target::Fn(f) => (fn_signature(p, f), doc_of(p.func(f).span)),
        Target::Field(d, i) => {
            let adt = p.adt(d);
            let fd = &adt.fields()[i as usize];
            (format!("{}{}: {}    (field of {})", if fd.is_pub { "pub " } else { "" }, fd.name, p.ty_str_with(fd.ty, &adt.generics), adt.name), doc_of(fd.span))
        }
        Target::Adt(d) => {
            let adt = p.adt(d);
            let gen = if adt.generics.is_empty() { String::new() } else { format!("[{}]", adt.generics.join(", ")) };
            let body = match &adt.kind {
                AdtKind::Struct { fields } => {
                    let fs: Vec<String> = fields.iter().map(|f| format!("    {}{}: {}", if f.is_pub { "pub " } else { "" }, f.name, p.ty_str_with(f.ty, &adt.generics))).collect();
                    format!("struct {}{} {{\n{}\n}}", adt.name, gen, fs.join("\n"))
                }
                AdtKind::Enum { variants } => {
                    let vs: Vec<String> = variants
                        .iter()
                        .map(|v| {
                            if v.fields.is_empty() {
                                format!("    {}", v.name)
                            } else {
                                format!("    {}({})", v.name, v.fields.iter().map(|&t| p.ty_str_with(t, &adt.generics)).collect::<Vec<_>>().join(", "))
                            }
                        })
                        .collect();
                    format!("enum {}{} {{\n{}\n}}", adt.name, gen, vs.join("\n"))
                }
            };
            (body, doc_of(adt.span))
        }
        Target::Variant(d, v) => {
            let adt = p.adt(d);
            let var = &adt.variants()[v as usize];
            let fields = if var.fields.is_empty() { String::new() } else { format!("({})", var.fields.iter().map(|&t| p.ty_str_with(t, &adt.generics)).collect::<Vec<_>>().join(", ")) };
            (format!("{}.{}{}", adt.name, var.name, fields), doc_of(var.span))
        }
        Target::Const(c) => {
            let cd = &p.consts[c.idx()];
            let v = p.const_values.get(&c).map(|v| format!(" = {}", const_text(v))).unwrap_or_default();
            (format!("const {}: {}{}", cd.name, p.ty_str(cd.ty), v), doc_of(cd.span))
        }
        Target::Static(s) => {
            let sd = &p.statics[s.idx()];
            (format!("static {}{}: {}", if sd.mutable { "var " } else { "" }, sd.name, p.ty_str(sd.ty)), doc_of(sd.span))
        }
        Target::Trait(t) => {
            let td = &p.traits[t.idx()];
            let sup = if td.supertraits.is_empty() { String::new() } else { format!(": {}", td.supertraits.iter().map(|s| p.traits[s.idx()].name.clone()).collect::<Vec<_>>().join(" + ")) };
            let ms: Vec<String> = td.methods.iter().map(|m| format!("    {}", fn_signature(p, m.decl).replacen(&format!("{}.", td.name), "", 1))).collect();
            (format!("trait {}{} {{\n{}\n}}", td.name, sup, ms.join("\n")), doc_of(td.span))
        }
        Target::Module(m) => (format!("module {}", p.modules[m.idx()].path), module_doc(a, m)),
    }
}

fn module_doc(a: &Analysis, m: ModId) -> String {
    let f = a.sm.get(a.prog.modules[m.idx()].file);
    let mut lines = Vec::new();
    for l in f.text.lines() {
        match l.trim().strip_prefix("///") {
            Some(r) => lines.push(r.strip_prefix(' ').unwrap_or(r).to_string()),
            None => break,
        }
    }
    lines.join("\n")
}

fn const_text(v: &nxc_sema::defs::ConstValue) -> String {
    use nxc_sema::defs::ConstValue as C;
    match v {
        C::Int(n) => n.to_string(),
        C::Float(f) => format!("{:?}", f),
        C::Bool(b) => b.to_string(),
        C::Char(c) => format!("{:?}", c),
        C::Str(s) => format!("{:?}", s),
        C::Unit => "()".into(),
    }
}

// ---------------------------------------------------------------------------
// Requests

pub fn hover(a: &Analysis, line: usize, col: usize) -> String {
    let off = a.offset(line, col);
    match target_at(a, off) {
        Some((t, _, ty)) => {
            let (code, doc) = describe(a, t, ty);
            format!("{{\"code\":{},\"doc\":{}}}", json_str(&code), json_str(&doc))
        }
        None => "null".into(),
    }
}

pub fn definition(a: &Analysis, line: usize, col: usize) -> String {
    let off = a.offset(line, col);
    match target_at(a, off) {
        Some((t, _, _)) => a.pos_json(definition_span(a, t)),
        None => "null".into(),
    }
}

/// Every occurrence of the entity at the position (declaration included), in the files
/// of the program (not the standard library).
pub fn references(a: &Analysis, line: usize, col: usize) -> Vec<Span> {
    let off = a.offset(line, col);
    let Some((t, _, _)) = target_at(a, off) else { return vec![] };
    let p = &a.prog;
    let mut out = vec![definition_span(a, t)];
    let name_span = |sm: &SourceMap, span: Span, name: &str| -> Option<Span> {
        // The occurrence of `name` inside `span` (calls and field accesses cover more).
        let text = sm.snippet(span);
        let bytes = text.as_bytes();
        let mut from = 0;
        let mut last = None;
        while let Some(i) = text[from..].find(name) {
            let s = from + i;
            let e = s + name.len();
            let before = s == 0 || !is_ident(bytes[s - 1] as char);
            let after = e >= bytes.len() || !is_ident(bytes[e] as char);
            if before && after {
                last = Some(Span::new(span.file, span.lo + s as u32, span.lo + e as u32));
                if !matches!(t, Target::Field(..)) {
                    return last;
                }
            }
            from = e;
        }
        last
    };
    for body in a.user_bodies() {
        let mut visit = |n: Node| match n {
            Node::Expr(e) => {
                let hit = match (&e.kind, t) {
                    (T::Local(l), Target::Local(f, m)) => body.fn_id == f && *l == m || closure_shares(p, body.fn_id, f) && *l == m,
                    (T::Call { callee: Callee::Fn(g, _), .. }, Target::Fn(f)) => *g == f,
                    (T::FnRef(g), Target::Fn(f)) => *g == f,
                    (T::Field(base, i), Target::Field(d, j)) => *i == j && p.tcx.as_adt(base.ty).map(|x| x.0) == Some(d),
                    (T::Adt { adt, variant: None, .. }, Target::Adt(d)) => *adt == d,
                    (T::Adt { adt, variant: Some(v), .. }, Target::Variant(d, w)) => *adt == d && *v == w,
                    (T::Const(c), Target::Const(d)) => *c == d,
                    (T::Static(s), Target::Static(d)) => *s == d,
                    _ => false,
                };
                if hit {
                    let name = match t {
                        Target::Local(f, l) => p.bodies.get(&f).map(|b| b.locals[l as usize].name.clone()).unwrap_or_default(),
                        Target::Fn(f) => p.func(f).name.clone(),
                        Target::Field(d, i) => p.adt(d).fields()[i as usize].name.clone(),
                        Target::Adt(d) => p.adt(d).name.clone(),
                        Target::Variant(d, v) => p.adt(d).variants()[v as usize].name.clone(),
                        Target::Const(c) => p.consts[c.idx()].name.clone(),
                        Target::Static(s) => p.statics[s.idx()].name.clone(),
                        _ => String::new(),
                    };
                    if let Some(s) = name_span(&a.sm, e.span, &name) {
                        out.push(s);
                    }
                }
            }
            Node::Pat(pt) => {
                if let (PatKind::Binding(l), Target::Local(f, m)) = (&pt.kind, t) {
                    if (body.fn_id == f || closure_shares(p, body.fn_id, f)) && *l == m {
                        out.push(pt.span);
                    }
                }
            }
        };
        walk_block(&body.block, &mut visit);
    }
    out.retain(|s| !s.is_dummy());
    out.sort();
    out.dedup();
    out
}

/// Anonymous functions share the locals of the function that contains them.
fn closure_shares(p: &Program, body_fn: FnId, f: FnId) -> bool {
    let a = p.func(body_fn);
    let b = p.func(f);
    (matches!(a.kind, FnKind::Closure { .. }) || matches!(b.kind, FnKind::Closure { .. })) && a.span.file == b.span.file && {
        // The closure is inside the function (or the reverse).
        let (outer, inner) = if matches!(a.kind, FnKind::Closure { .. }) { (f, body_fn) } else { (body_fn, f) };
        match (p.bodies.get(&outer), p.bodies.get(&inner)) {
            (Some(o), Some(i)) => o.span.lo <= i.span.lo && i.span.hi <= o.span.hi,
            _ => false,
        }
    }
}

pub fn references_json(a: &Analysis, refs: &[Span]) -> String {
    let items: Vec<String> = refs.iter().filter(|s| !a.sm.get(s.file).name.starts_with('<')).map(|s| a.pos_json(*s)).collect();
    format!("[{}]", items.join(","))
}

/// The edits renaming the entity at the position.
pub fn rename(a: &Analysis, line: usize, col: usize, new_name: &str) -> Result<String, String> {
    if new_name.is_empty() || !new_name.chars().all(is_ident) || new_name.chars().next().map_or(true, |c| c.is_ascii_digit()) {
        return Err(format!("`{}` is not a valid name", new_name));
    }
    let refs = references(a, line, col);
    if refs.is_empty() {
        return Err("nothing to rename here".into());
    }
    if refs.iter().any(|s| a.sm.get(s.file).name.starts_with('<')) {
        return Err("names of the standard library cannot be renamed".into());
    }
    Ok(references_json(a, &refs))
}

// ---------------------------------------------------------------------------
// Outline

pub fn symbols(a: &Analysis) -> String {
    let p = &a.prog;
    let mut items: Vec<(u32, String)> = Vec::new();
    let pos = |s: Span| {
        let (l, c) = a.sm.get(s.file).line_col(s.lo);
        (l, c)
    };
    let sym = |name: &str, kind: &str, detail: &str, s: Span, children: String| {
        let (l, c) = pos(s);
        format!("{{\"name\":{},\"kind\":\"{}\",\"detail\":{},\"line\":{},\"col\":{},\"children\":{}}}", json_str(name), kind, json_str(detail), l, c, children)
    };
    for (i, f) in p.fns.iter().enumerate() {
        if f.span.file != a.file || f.owner.is_some() || f.self_ty.is_some() || matches!(f.kind, FnKind::Closure { .. }) {
            continue;
        }
        let kind = if f.is_test { "test" } else { "function" };
        items.push((f.span.lo, sym(&f.name, kind, &fn_signature(p, FnId(i as u32)), f.span, "[]".into())));
    }
    for (i, d) in p.adts.iter().enumerate() {
        if d.span.file != a.file {
            continue;
        }
        let mut kids: Vec<(u32, String)> = Vec::new();
        for f in d.fields() {
            kids.push((f.span.lo, sym(&f.name, "field", &p.ty_str_with(f.ty, &d.generics), f.span, "[]".into())));
        }
        for v in d.variants() {
            kids.push((v.span.lo, sym(&v.name, "variant", "", v.span, "[]".into())));
        }
        for (j, f) in p.fns.iter().enumerate() {
            if f.owner == Some(AdtId(i as u32)) && f.span.file == a.file && !matches!(f.kind, FnKind::Closure { .. }) {
                kids.push((f.span.lo, sym(&f.name, "method", &fn_signature(p, FnId(j as u32)), f.span, "[]".into())));
            }
        }
        kids.sort_by_key(|k| k.0);
        let children = format!("[{}]", kids.into_iter().map(|k| k.1).collect::<Vec<_>>().join(","));
        items.push((d.span.lo, sym(&d.name, if d.is_enum() { "enum" } else { "struct" }, "", d.span, children)));
    }
    for t in &p.traits {
        if t.span.file != a.file {
            continue;
        }
        let kids: Vec<String> = t.methods.iter().map(|m| sym(&m.name, "method", &fn_signature(p, m.decl), p.func(m.decl).span, "[]".into())).collect();
        items.push((t.span.lo, sym(&t.name, "trait", "", t.span, format!("[{}]", kids.join(",")))));
    }
    for c in &p.consts {
        if c.span.file == a.file {
            items.push((c.span.lo, sym(&c.name, "constant", &p.ty_str(c.ty), c.span, "[]".into())));
        }
    }
    for s in &p.statics {
        if s.span.file == a.file {
            items.push((s.span.lo, sym(&s.name, "static", &p.ty_str(s.ty), s.span, "[]".into())));
        }
    }
    items.sort_by_key(|i| i.0);
    format!("[{}]", items.into_iter().map(|i| i.1).collect::<Vec<_>>().join(","))
}

/// The `@test` functions of the file: name and line.
pub fn tests(a: &Analysis) -> String {
    let mut v: Vec<(u32, String)> = Vec::new();
    for f in &a.prog.fns {
        if f.is_test && f.span.file == a.file {
            let (l, _) = a.sm.get(a.file).line_col(f.span.lo);
            v.push((f.span.lo, format!("{{\"name\":{},\"line\":{}}}", json_str(&f.name), l)));
        }
    }
    v.sort_by_key(|x| x.0);
    format!("[{}]", v.into_iter().map(|x| x.1).collect::<Vec<_>>().join(","))
}

// ---------------------------------------------------------------------------
// Completion

const KEYWORDS: &[&str] = &[
    "fn", "let", "var", "if", "else", "while", "for", "in", "match", "return", "break", "continue", "struct", "enum", "trait", "impl", "pub", "import",
    "const", "static", "type", "extern", "unsafe", "inout", "owned", "move", "and", "or", "not", "true", "false", "self", "Self", "dyn", "where", "indirect",
];

const PRIMS: &[&str] = &["i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize", "f32", "f64", "bool", "char", "Never"];

const INT_METHODS: &[(&str, &str)] = &[
    ("abs", "fn abs(self) -> Self"),
    ("min", "fn min(self, other: Self) -> Self"),
    ("max", "fn max(self, other: Self) -> Self"),
    ("pow", "fn pow(self, exp: u32) -> Self"),
    ("leading_zeros", "fn leading_zeros(self) -> u32"),
    ("trailing_zeros", "fn trailing_zeros(self) -> u32"),
    ("count_ones", "fn count_ones(self) -> u32"),
    ("rotate_left", "fn rotate_left(self, n: u32) -> Self"),
    ("rotate_right", "fn rotate_right(self, n: u32) -> Self"),
    ("swap_bytes", "fn swap_bytes(self) -> Self"),
    ("to_string", "fn to_string(self) -> String"),
];

const FLOAT_METHODS: &[&str] = &[
    "sqrt", "floor", "ceil", "round", "trunc", "sin", "cos", "tan", "asin", "acos", "atan", "exp", "ln", "log10", "log2", "abs", "powf", "atan2", "min", "max",
    "hypot", "copysign", "to_bits", "is_nan", "is_infinite", "is_finite", "to_string",
];

const CHAR_METHODS: &[&str] = &[
    "is_ascii_digit", "is_ascii_alphabetic", "is_ascii_alphanumeric", "is_whitespace", "is_ascii_uppercase", "is_ascii_lowercase", "is_ascii",
    "to_ascii_uppercase", "to_ascii_lowercase", "len_utf8", "to_string",
];

struct Item {
    label: String,
    kind: &'static str,
    detail: String,
    doc: String,
}

fn items_json(items: Vec<Item>) -> String {
    let mut seen = std::collections::HashSet::new();
    let v: Vec<String> = items
        .into_iter()
        .filter(|i| seen.insert((i.label.clone(), i.kind)))
        .map(|i| format!("{{\"label\":{},\"kind\":\"{}\",\"detail\":{},\"doc\":{}}}", json_str(&i.label), i.kind, json_str(&i.detail), json_str(&i.doc)))
        .collect();
    format!("[{}]", v.join(","))
}

/// The start of the postfix expression ending at `end` (`a.b(c)[d]`).
fn receiver_start(text: &str, end: usize) -> usize {
    let b = text.as_bytes();
    let mut i = end;
    loop {
        if i == 0 {
            return 0;
        }
        let c = b[i - 1] as char;
        if is_ident(c) || c == '.' {
            i -= 1;
        } else if c == ')' || c == ']' {
            let (open, close) = if c == ')' { (b'(', b')') } else { (b'[', b']') };
            let mut depth = 0i32;
            let mut j = i;
            while j > 0 {
                j -= 1;
                if b[j] == close {
                    depth += 1;
                } else if b[j] == open {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
            i = j;
        } else {
            return i;
        }
    }
}

/// Completion at a position of `text` (the file `path`).
pub fn completion(path: &Path, text: &str, line: usize, col: usize) -> String {
    let off = offset_of(text, line, col) as usize;
    let b = text.as_bytes();
    let mut ps = off.min(b.len());
    while ps > 0 && is_ident(b[ps - 1] as char) {
        ps -= 1;
    }
    if ps > 0 && b[ps - 1] == b'.' && !(ps > 1 && b[ps - 2] == b'.') {
        let dot = ps - 1;
        let rs = receiver_start(text, dot);
        let receiver = text[rs..dot].to_string();
        // Analyze the text without the member being written (`.name(args)`): the receiver
        // becomes a complete expression.
        let mut tail = off.min(b.len());
        while tail < b.len() && is_ident(b[tail] as char) {
            tail += 1;
        }
        if tail < b.len() && b[tail] == b'(' {
            let mut depth = 0;
            while tail < b.len() {
                match b[tail] {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            tail += 1;
                            break;
                        }
                    }
                    b'\n' => break,
                    _ => {}
                }
                tail += 1;
            }
        }
        let edited = format!("{}{}", &text[..dot], &text[tail..]);
        let Some(a) = analyze(path, Some(edited)) else { return "[]".into() };
        return items_json(member_items(&a, rs as u32, dot as u32, &receiver));
    }
    let Some(a) = analyze(path, Some(text.to_string())) else { return "[]".into() };
    items_json(scope_items(&a, off as u32))
}

fn member_items(a: &Analysis, start: u32, end: u32, receiver: &str) -> Vec<Item> {
    let p = &a.prog;
    // The receiver as an expression of the program.
    let mut found: Option<Ty> = None;
    let mut best_len = u32::MAX;
    for body in a.user_bodies() {
        let mut visit = |n: Node| {
            if let Node::Expr(e) = n {
                if e.span.file == a.file && e.span.hi == end && e.span.lo >= start && e.span.len() <= best_len && e.span.lo == start {
                    best_len = e.span.len();
                    found = Some(e.ty);
                }
            }
        };
        walk_block(&body.block, &mut visit);
    }
    if found.is_none() {
        // Locals are found above; here, `module.`, `Type.` or `Trait.`.
        if receiver.chars().all(is_ident) {
            if let Some(it) = nxc_sema::collect::lookup_item(p, a.root, receiver) {
                return path_items(a, it);
            }
            if let Some(t) = nxc_sema::collect::primitive_type(p, receiver) {
                return type_assoc_items(a, t);
            }
        }
        return vec![];
    }
    type_members(a, found.unwrap())
}

fn path_items(a: &Analysis, it: ItemRef) -> Vec<Item> {
    let p = &a.prog;
    match it {
        ItemRef::Module(m) => {
            let mut v = Vec::new();
            for (name, (item, span, is_pub)) in &p.modules[m.idx()].items {
                if !*is_pub && m != a.root {
                    continue;
                }
                let (kind, detail) = match item {
                    ItemRef::Fn(f) => ("function", fn_signature(p, *f)),
                    ItemRef::Adt(d) => (if p.adt(*d).is_enum() { "enum" } else { "struct" }, String::new()),
                    ItemRef::Const(c) => ("constant", p.ty_str(p.consts[c.idx()].ty)),
                    ItemRef::Static(s) => ("static", p.ty_str(p.statics[s.idx()].ty)),
                    ItemRef::Trait(_) => ("trait", String::new()),
                    ItemRef::Alias(_) => ("type", String::new()),
                    _ => ("module", String::new()),
                };
                v.push(Item { label: name.clone(), kind, detail, doc: doc_comment(&a.sm, *span) });
            }
            v.sort_by(|x, y| x.label.cmp(&y.label));
            v
        }
        ItemRef::Adt(d) => {
            let adt = p.adt(d);
            let ty = p.tcx.adt(d, (0..adt.generics.len() as u32).map(|i| p.tcx.param(i)).collect());
            let mut v = Vec::new();
            for var in adt.variants() {
                v.push(Item { label: var.name.clone(), kind: "variant", detail: String::new(), doc: doc_comment(&a.sm, var.span) });
            }
            v.extend(type_assoc_items(a, ty));
            v
        }
        ItemRef::Trait(t) => p.traits[t.idx()]
            .methods
            .iter()
            .filter(|m| !p.func(m.decl).has_self)
            .map(|m| Item { label: m.name.clone(), kind: "function", detail: fn_signature(p, m.decl), doc: doc_comment(&a.sm, p.func(m.decl).span) })
            .collect(),
        ItemRef::Alias(al) => type_assoc_items(a, p.aliases[al.idx()].ty),
        _ => vec![],
    }
}

/// Associated functions (without `self`) of a type: `Vec.new()`, `T.default()`.
fn type_assoc_items(a: &Analysis, ty: Ty) -> Vec<Item> {
    let p = &a.prog;
    let mut v = Vec::new();
    if let Some((d, _)) = p.tcx.as_adt(ty) {
        let mut ms: Vec<(&String, &FnId)> = p.adt(d).methods.iter().collect();
        ms.sort();
        for (name, &f) in ms {
            let fd = p.func(f);
            if !fd.has_self && (fd.is_pub || fd.module == a.root) {
                v.push(Item { label: name.clone(), kind: "function", detail: fn_signature(p, f), doc: doc_comment(&a.sm, fd.span) });
            }
        }
    }
    for (_, f) in trait_methods(p, ty) {
        let fd = p.func(f);
        if !fd.has_self {
            v.push(Item { label: fd.name.clone(), kind: "function", detail: fn_signature(p, f), doc: doc_comment(&a.sm, fd.span) });
        }
    }
    v
}

/// The trait methods available on a type (implementations of concrete types).
fn trait_methods(p: &Program, ty: Ty) -> Vec<(TraitId, FnId)> {
    let mut traits: Vec<TraitId> = Vec::new();
    match &*p.tcx.kind(ty) {
        TyKind::Dyn(t) => traits = p.trait_closure(*t),
        _ => {
            for imp in &p.impls {
                if !traits.contains(&imp.trait_id) && p.find_impl(imp.trait_id, ty).is_some() {
                    traits.push(imp.trait_id);
                }
            }
        }
    }
    let mut out = Vec::new();
    for t in traits {
        for m in &p.traits[t.idx()].methods {
            out.push((t, m.decl));
        }
    }
    out
}

fn type_members(a: &Analysis, ty: Ty) -> Vec<Item> {
    let p = &a.prog;
    let mut v = Vec::new();
    let kind = p.tcx.kind(ty);
    match &*kind {
        TyKind::Adt(d, args) => {
            let adt = p.adt(*d);
            for f in adt.fields() {
                if f.is_pub || adt.module == a.root {
                    v.push(Item { label: f.name.clone(), kind: "field", detail: p.ty_str(p.tcx.subst(f.ty, args)), doc: doc_comment(&a.sm, f.span) });
                }
            }
            let mut ms: Vec<(&String, &FnId)> = adt.methods.iter().collect();
            ms.sort();
            for (name, &f) in ms {
                let fd = p.func(f);
                if fd.has_self && (fd.is_pub || fd.module == a.root) && Some(f) != adt.drop_fn && Some(f) != adt.index_fn {
                    v.push(Item { label: name.clone(), kind: "method", detail: fn_signature(p, f), doc: doc_comment(&a.sm, fd.span) });
                }
            }
        }
        TyKind::Int(_) => {
            for (m, sig) in INT_METHODS {
                v.push(Item { label: m.to_string(), kind: "method", detail: sig.to_string(), doc: String::new() });
            }
        }
        TyKind::Float(_) => {
            for m in FLOAT_METHODS {
                v.push(Item { label: m.to_string(), kind: "method", detail: String::new(), doc: String::new() });
            }
        }
        TyKind::Char => {
            for m in CHAR_METHODS {
                v.push(Item { label: m.to_string(), kind: "method", detail: String::new(), doc: String::new() });
            }
        }
        TyKind::Ptr(_) => {
            for m in ["is_null", "addr", "offset", "sub", "read", "write", "cast"] {
                v.push(Item { label: m.to_string(), kind: "method", detail: String::new(), doc: String::new() });
            }
        }
        TyKind::Tuple(ts) => {
            for (i, t) in ts.iter().enumerate() {
                v.push(Item { label: i.to_string(), kind: "field", detail: p.ty_str(*t), doc: String::new() });
            }
        }
        _ => {}
    }
    for (_, f) in trait_methods(p, ty) {
        let fd = p.func(f);
        if fd.has_self {
            v.push(Item { label: fd.name.clone(), kind: "method", detail: fn_signature(p, f), doc: doc_comment(&a.sm, fd.span) });
        }
    }
    v
}

fn scope_items(a: &Analysis, off: u32) -> Vec<Item> {
    let p = &a.prog;
    let mut v = Vec::new();
    // Locals of the innermost function containing the position, declared before it.
    let mut inner: Option<&Body> = None;
    for b in a.user_bodies() {
        if b.span.file == a.file && b.span.lo <= off && off <= b.span.hi && inner.map_or(true, |i| b.span.len() < i.span.len()) {
            inner = Some(b);
        }
    }
    if let Some(b) = inner {
        let names = &p.func(b.fn_id).generics;
        for l in b.locals.iter().rev() {
            if l.span.file == a.file && l.span.lo < off && !l.name.starts_with('$') && l.kind != LocalKind::Hidden {
                v.push(Item { label: l.name.clone(), kind: "variable", detail: p.ty_str_with(l.ty, names), doc: String::new() });
            }
        }
    }
    let m = &p.modules[a.root.idx()];
    for (name, (item, span, _)) in &m.items {
        v.push(item_completion(a, name, *item, *span));
    }
    for (name, e) in &m.imports {
        v.push(item_completion(a, name, e.item, Span::DUMMY));
    }
    for (name, item) in &p.prelude {
        v.push(item_completion(a, name, *item, Span::DUMMY));
    }
    for k in KEYWORDS {
        v.push(Item { label: k.to_string(), kind: "keyword", detail: String::new(), doc: String::new() });
    }
    for t in PRIMS {
        v.push(Item { label: t.to_string(), kind: "type", detail: String::new(), doc: String::new() });
    }
    v
}

fn item_completion(a: &Analysis, name: &str, item: ItemRef, span: Span) -> Item {
    let p = &a.prog;
    let (kind, detail, dspan) = match item {
        ItemRef::Fn(f) => ("function", fn_signature(p, f), p.func(f).span),
        ItemRef::Adt(d) => (if p.adt(d).is_enum() { "enum" } else { "struct" }, String::new(), p.adt(d).span),
        ItemRef::Variant(d, _) => ("variant", p.adt(d).name.clone(), span),
        ItemRef::Const(c) => ("constant", p.ty_str(p.consts[c.idx()].ty), p.consts[c.idx()].span),
        ItemRef::Static(s) => ("static", p.ty_str(p.statics[s.idx()].ty), p.statics[s.idx()].span),
        ItemRef::Trait(t) => ("trait", String::new(), p.traits[t.idx()].span),
        ItemRef::Module(m) => ("module", p.modules[m.idx()].path.clone(), span),
        ItemRef::Alias(al) => ("type", p.ty_str(p.aliases[al.idx()].ty), p.aliases[al.idx()].span),
        ItemRef::Builtin(b) => ("function", format!("{}(...)", b.name()), span),
    };
    Item { label: name.to_string(), kind, detail, doc: doc_comment(&a.sm, dspan) }
}

// ---------------------------------------------------------------------------
// Formatting

/// Reformats NEXUL source: indentation by nesting (4 spaces), no trailing spaces, at most
/// one blank line in a row, a space after commas, a single final newline. Strings and
/// comments are kept as written.
pub fn format_source(text: &str) -> String {
    let crlf = text.contains("\r\n");
    let mut out = String::new();
    // Each level of indentation holds the brackets opened on one line: `f(x, fn(i) {`
    // indents its body once.
    let mut levels: Vec<u32> = Vec::new();
    let mut block_comment = 0u32;
    let mut blank = 0;
    let mut continuation = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            blank += 1;
            continue;
        }
        if blank > 0 && !out.is_empty() {
            out.push('\n');
        }
        blank = 0;
        if block_comment > 0 {
            // Inside a block comment: kept as written.
            out.push_str(raw.trim_end());
            out.push('\n');
            let (_, bc, _) = scan(line, block_comment);
            block_comment = bc;
            continue;
        }
        let normalized = normalize_line(line);
        let (_, bc, code) = scan(&normalized, 0);
        let brackets: Vec<char> = code.chars().filter(|c| matches!(c, '{' | '(' | '[' | '}' | ')' | ']')).collect();
        let leading = normalized.chars().take_while(|c| matches!(c, '}' | ')' | ']')).count();
        fn close(levels: &mut Vec<u32>) {
            if let Some(top) = levels.last_mut() {
                *top -= 1;
                if *top == 0 {
                    levels.pop();
                }
            }
        }
        for _ in 0..leading {
            close(&mut levels);
        }
        let starts_dot = normalized.starts_with('.') && !normalized.starts_with("..");
        let indent = levels.len() + if continuation || starts_dot { 1 } else { 0 };
        for _ in 0..indent {
            out.push_str("    ");
        }
        out.push_str(&normalized);
        out.push('\n');
        let mut opened = 0u32;
        for c in brackets.into_iter().skip(leading) {
            if matches!(c, '{' | '(' | '[') {
                opened += 1;
            } else if opened > 0 {
                opened -= 1;
            } else {
                close(&mut levels);
            }
        }
        if opened > 0 {
            levels.push(opened);
        }
        block_comment = bc;
        continuation = depth_neutral_continuation(code.trim_end());
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    if out.is_empty() {
        return out;
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    if crlf {
        out = out.replace('\n', "\r\n");
    }
    out
}

/// A line ending with a binary operator continues on the next line.
fn depth_neutral_continuation(code: &str) -> bool {
    const OPS: &[&str] = &["+", "-", "*", "/", "%", "=", "==", "!=", "<", ">", "<=", ">=", "and", "or", "&", "|", "^", "<<", ">>", "->", "=>"];
    let t = code.trim_end();
    if t.ends_with("=>") || t.ends_with("->") {
        return false;
    }
    OPS.iter().any(|op| {
        t.ends_with(op) && {
            let before = &t[..t.len() - op.len()];
            let word_op = op.chars().all(|c| c.is_ascii_alphabetic());
            !word_op || before.ends_with(' ')
        }
    })
}

/// Change of nesting depth over a line, the block-comment depth after it, and the code
/// with strings and comments blanked out.
fn scan(line: &str, mut block: u32) -> (i32, u32, String) {
    let mut delta = 0;
    let mut code = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut in_str = false;
    let mut in_char = false;
    let mut interp = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if block > 0 {
            if c == '*' && next == Some('/') {
                block -= 1;
                i += 2;
                continue;
            }
            if c == '/' && next == Some('*') {
                block += 1;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_str {
            if c == '\\' {
                i += 2;
                continue;
            }
            if c == '{' {
                interp += 1;
            } else if c == '}' && interp > 0 {
                interp -= 1;
            } else if c == '"' && interp == 0 {
                in_str = false;
            }
            code.push(' ');
            i += 1;
            continue;
        }
        if in_char {
            if c == '\\' {
                i += 2;
                continue;
            }
            if c == '\'' {
                in_char = false;
            }
            i += 1;
            continue;
        }
        match c {
            '/' if next == Some('/') => break,
            '/' if next == Some('*') => {
                block += 1;
                i += 2;
                continue;
            }
            '"' => in_str = true,
            '\'' if chars.get(i + 2) == Some(&'\'') || next == Some('\\') => in_char = true,
            '{' | '(' | '[' => delta += 1,
            '}' | ')' | ']' => delta -= 1,
            _ => {}
        }
        code.push(c);
        i += 1;
    }
    (delta, block, code)
}

/// Spacing inside a line, outside strings and comments: one space after a comma, none
/// before it, a space before `{` after `)` or a name.
fn normalize_line(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut in_str = false;
    let mut in_char = false;
    let mut interp = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if in_str {
            out.push(c);
            if c == '\\' {
                if let Some(n) = next {
                    out.push(n);
                }
                i += 2;
                continue;
            }
            if c == '{' {
                interp += 1;
            } else if c == '}' && interp > 0 {
                interp -= 1;
            } else if c == '"' && interp == 0 {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if in_char {
            out.push(c);
            if c == '\\' {
                if let Some(n) = next {
                    out.push(n);
                }
                i += 2;
                continue;
            }
            if c == '\'' {
                in_char = false;
            }
            i += 1;
            continue;
        }
        if c == '/' && (next == Some('/') || next == Some('*')) {
            // The rest of the line is a comment: one space before it.
            let rest: String = chars[i..].iter().collect();
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push_str(rest.trim_end());
            return out.trim_end().to_string();
        }
        match c {
            '"' => {
                in_str = true;
                out.push(c);
            }
            '\'' if chars.get(i + 2) == Some(&'\'') || next == Some('\\') => {
                in_char = true;
                out.push(c);
            }
            ',' => {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push(',');
                if let Some(n) = next {
                    if n != ' ' && n != ')' && n != ']' {
                        out.push(' ');
                    }
                }
            }
            '{' => {
                if let Some(prev) = out.chars().last() {
                    if prev == ')' || is_ident(prev) || prev == ']' {
                        out.push(' ');
                    }
                }
                out.push('{');
            }
            ' ' => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out.trim_end().to_string()
}

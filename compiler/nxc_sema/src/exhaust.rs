//! Exhaustiveness and reachability of patterns (spec §4.7), using the classic
//! "usefulness" algorithm on pattern matrices.
//!
//! Integers, characters and strings are treated as infinite domains: a `match` on them
//! needs a wildcard or a binding to be exhaustive.

use crate::defs::{AdtKind, ConstValue, Program};
use crate::thir::{self, Block, Expr, ExprKind as T, Pat, PatKind, StmtKind as S};
use crate::ty::{Ty, TyKind};
use nxc_syntax::{DiagSink, Diagnostic};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Ctor {
    Single,
    Variant(u32),
    Bool(bool),
    Lit(LitKey),
    Range(i128, i128, bool),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum LitKey {
    Int(i128),
    Char(char),
    Str(String),
    Other,
}

#[derive(Clone, Debug)]
enum DPat {
    Wild,
    Ctor(Ctor, Vec<DPat>),
    Or(Vec<DPat>),
}

#[derive(Clone, Debug)]
enum WPat {
    Wild,
    Ctor(Ctor, Vec<WPat>),
}

fn lower(p: &Pat) -> DPat {
    match &p.kind {
        PatKind::Wild | PatKind::Binding(_) | PatKind::Error => DPat::Wild,
        PatKind::Lit(ConstValue::Bool(b)) => DPat::Ctor(Ctor::Bool(*b), vec![]),
        PatKind::Lit(v) => DPat::Ctor(
            Ctor::Lit(match v {
                ConstValue::Int(n) => LitKey::Int(*n),
                ConstValue::Char(c) => LitKey::Char(*c),
                ConstValue::Str(s) => LitKey::Str(s.clone()),
                _ => LitKey::Other,
            }),
            vec![],
        ),
        PatKind::Range(a, b, inc) => {
            let (lo, hi) = match (a, b) {
                (ConstValue::Int(x), ConstValue::Int(y)) => (*x, *y),
                (ConstValue::Char(x), ConstValue::Char(y)) => (*x as i128, *y as i128),
                _ => (0, 0),
            };
            DPat::Ctor(Ctor::Range(lo, hi, *inc), vec![])
        }
        PatKind::Tuple(ps) => DPat::Ctor(Ctor::Single, ps.iter().map(lower).collect()),
        PatKind::Struct { fields, .. } => DPat::Ctor(Ctor::Single, fields.iter().map(lower).collect()),
        PatKind::Variant { variant, fields, .. } => DPat::Ctor(Ctor::Variant(*variant), fields.iter().map(lower).collect()),
        PatKind::Or(ps) => DPat::Or(ps.iter().map(lower).collect()),
    }
}

/// The constructors of a pattern that is a wildcard (empty list) or one or more
/// alternatives of constructors without sub-patterns other than wildcards (`3`, `true`,
/// `Rouge`, `Some(_)`). `None` for any other pattern (ranges, nested patterns).
fn flat_ctors(p: &DPat) -> Option<Vec<Ctor>> {
    match p {
        DPat::Wild => Some(vec![]),
        DPat::Ctor(Ctor::Range(..), _) => None,
        DPat::Ctor(c, args) if args.iter().all(|a| matches!(a, DPat::Wild)) => Some(vec![c.clone()]),
        DPat::Or(alts) => {
            let mut out = Vec::new();
            for a in alts {
                match flat_ctors(a)? {
                    v if v.is_empty() => return None,
                    v => out.extend(v),
                }
            }
            Some(out)
        }
        _ => None,
    }
}

/// The rows of a `match` while they are all flat (`flat_ctors`): the usefulness of a
/// flat pattern then reduces to set membership, so that a `match` of thousands of
/// literal or variant arms is checked in linear time instead of quadratic time.
#[derive(Default)]
struct FlatRows {
    wild: bool,
    ctors: HashSet<Ctor>,
    /// A row that is not flat was added: use the general algorithm.
    general: bool,
}

struct Cx<'a> {
    p: &'a Program,
}

impl<'a> Cx<'a> {
    /// The complete list of constructors of a type, or `None` for infinite domains.
    fn ctors(&self, ty: Ty) -> Option<Vec<Ctor>> {
        let tcx = &self.p.tcx;
        match &*tcx.kind(ty) {
            TyKind::Bool => Some(vec![Ctor::Bool(false), Ctor::Bool(true)]),
            TyKind::Never => Some(vec![]),
            TyKind::Unit | TyKind::Tuple(_) => Some(vec![Ctor::Single]),
            TyKind::Adt(id, _) => {
                let adt = self.p.adt(*id);
                match &adt.kind {
                    AdtKind::Struct { .. } => Some(vec![Ctor::Single]),
                    AdtKind::Enum { variants } => Some((0..variants.len() as u32).map(Ctor::Variant).collect()),
                }
            }
            _ => None,
        }
    }

    fn sub_tys(&self, ty: Ty, c: &Ctor) -> Vec<Ty> {
        let tcx = &self.p.tcx;
        match (&*tcx.kind(ty), c) {
            (TyKind::Tuple(ts), Ctor::Single) => ts.clone(),
            (TyKind::Adt(id, args), Ctor::Single) => self.p.adt(*id).fields().iter().map(|f| tcx.subst(f.ty, args)).collect(),
            (TyKind::Adt(id, args), Ctor::Variant(v)) => {
                self.p.adt(*id).variants()[*v as usize].fields.iter().map(|&f| tcx.subst(f, args)).collect()
            }
            _ => vec![],
        }
    }

    fn arity(&self, ty: Ty, c: &Ctor) -> usize {
        self.sub_tys(ty, c).len()
    }

    /// Rows whose head is an or-pattern become one row per alternative.
    fn expand(rows: &[Vec<DPat>]) -> Vec<Vec<DPat>> {
        let mut out = Vec::new();
        for r in rows {
            match r.first() {
                Some(DPat::Or(alts)) => {
                    let mut sub = Vec::new();
                    for a in alts {
                        let mut nr = vec![a.clone()];
                        nr.extend_from_slice(&r[1..]);
                        sub.push(nr);
                    }
                    out.extend(Self::expand(&sub));
                }
                _ => out.push(r.clone()),
            }
        }
        out
    }

    fn specialize(rows: &[Vec<DPat>], c: &Ctor, arity: usize) -> Vec<Vec<DPat>> {
        let mut out = Vec::new();
        for r in rows {
            match &r[0] {
                DPat::Wild => {
                    let mut nr = vec![DPat::Wild; arity];
                    nr.extend_from_slice(&r[1..]);
                    out.push(nr);
                }
                DPat::Ctor(c2, args) if c2 == c => {
                    let mut nr = args.clone();
                    nr.resize(arity, DPat::Wild);
                    nr.extend_from_slice(&r[1..]);
                    out.push(nr);
                }
                _ => {}
            }
        }
        out
    }

    /// Is `v` useful with respect to `rows`? Returns a witness of values matched by `v`
    /// and by no row.
    fn useful(&self, rows: &[Vec<DPat>], v: &[DPat], tys: &[Ty]) -> Option<Vec<WPat>> {
        if v.is_empty() {
            return if rows.is_empty() { Some(vec![]) } else { None };
        }
        let rows = Self::expand(rows);
        let ty = tys[0];
        match &v[0] {
            DPat::Or(alts) => {
                for a in alts {
                    let mut nv = vec![a.clone()];
                    nv.extend_from_slice(&v[1..]);
                    if let Some(w) = self.useful(&rows, &nv, tys) {
                        return Some(w);
                    }
                }
                None
            }
            DPat::Ctor(c, args) => {
                let arity = args.len().max(self.arity(ty, c));
                let spec = Self::specialize(&rows, c, arity);
                let mut nv = args.clone();
                nv.resize(arity, DPat::Wild);
                nv.extend_from_slice(&v[1..]);
                let mut ntys = self.sub_tys(ty, c);
                ntys.resize(arity, self.p.tcx.error);
                ntys.extend_from_slice(&tys[1..]);
                let w = self.useful(&spec, &nv, &ntys)?;
                let (sub, rest) = w.split_at(arity);
                let mut out = vec![WPat::Ctor(c.clone(), sub.to_vec())];
                out.extend_from_slice(rest);
                Some(out)
            }
            DPat::Wild => {
                let used: Vec<Ctor> = rows
                    .iter()
                    .filter_map(|r| match &r[0] {
                        DPat::Ctor(c, _) => Some(c.clone()),
                        _ => None,
                    })
                    .collect();
                match self.ctors(ty) {
                    Some(all) if all.iter().all(|c| used.contains(c)) => {
                        // Every constructor appears: check each one.
                        for c in &all {
                            let arity = self.arity(ty, c);
                            let spec = Self::specialize(&rows, c, arity);
                            let mut nv = vec![DPat::Wild; arity];
                            nv.extend_from_slice(&v[1..]);
                            let mut ntys = self.sub_tys(ty, c);
                            ntys.extend_from_slice(&tys[1..]);
                            if let Some(w) = self.useful(&spec, &nv, &ntys) {
                                let (sub, rest) = w.split_at(arity);
                                let mut out = vec![WPat::Ctor(c.clone(), sub.to_vec())];
                                out.extend_from_slice(rest);
                                return Some(out);
                            }
                        }
                        None
                    }
                    all => {
                        // Some constructor is missing: only rows starting with a wildcard
                        // can match it.
                        let default: Vec<Vec<DPat>> = rows.iter().filter(|r| matches!(r[0], DPat::Wild)).map(|r| r[1..].to_vec()).collect();
                        let w = self.useful(&default, &v[1..], &tys[1..])?;
                        let head = match all {
                            Some(all) => match all.into_iter().find(|c| !used.contains(c)) {
                                Some(c) => {
                                    let arity = self.arity(ty, &c);
                                    WPat::Ctor(c, vec![WPat::Wild; arity])
                                }
                                None => WPat::Wild,
                            },
                            None => WPat::Wild,
                        };
                        let mut out = vec![head];
                        out.extend(w);
                        Some(out)
                    }
                }
            }
        }
    }

    fn show(&self, w: &WPat, ty: Ty) -> String {
        let tcx = &self.p.tcx;
        match w {
            WPat::Wild => "_".into(),
            WPat::Ctor(c, subs) => {
                let tys = self.sub_tys(ty, c);
                let parts: Vec<String> = subs.iter().zip(tys.iter().chain(std::iter::repeat(&tcx.error))).map(|(s, &t)| self.show(s, t)).collect();
                match (c, &*tcx.kind(ty)) {
                    (Ctor::Bool(b), _) => b.to_string(),
                    (Ctor::Variant(v), TyKind::Adt(id, _)) => {
                        let name = &self.p.adt(*id).variants()[*v as usize].name;
                        if parts.is_empty() {
                            name.clone()
                        } else {
                            format!("{}({})", name, parts.join(", "))
                        }
                    }
                    (Ctor::Single, TyKind::Tuple(_)) => format!("({})", parts.join(", ")),
                    (Ctor::Single, TyKind::Unit) => "()".into(),
                    (Ctor::Single, TyKind::Adt(id, _)) => {
                        let adt = self.p.adt(*id);
                        if parts.iter().all(|p| p == "_") {
                            format!("{}(..)", adt.name)
                        } else {
                            let fs: Vec<String> = adt.fields().iter().zip(parts.iter()).map(|(f, p)| format!("{}: {}", f.name, p)).collect();
                            format!("{}({})", adt.name, fs.join(", "))
                        }
                    }
                    _ => "_".into(),
                }
            }
        }
    }
}

pub fn check_program(prog: &Program, diags: &DiagSink) {
    let cx = Cx { p: prog };
    let mut ids: Vec<_> = prog.bodies.keys().copied().collect();
    ids.sort();
    for id in ids {
        let body = &prog.bodies[&id];
        cx.block(&body.block, diags);
    }
}

impl<'a> Cx<'a> {
    /// A value not matched by flat rows (same witness as `useful` would give).
    fn flat_witness(&self, flat: &FlatRows, ty: Ty) -> Option<Vec<WPat>> {
        if flat.wild {
            return None;
        }
        Some(vec![match self.ctors(ty) {
            Some(all) => {
                let c = all.into_iter().find(|c| !flat.ctors.contains(c))?;
                let arity = self.arity(ty, &c);
                WPat::Ctor(c, vec![WPat::Wild; arity])
            }
            None => WPat::Wild,
        }])
    }

    fn irrefutable(&self, p: &Pat, what: &str, diags: &DiagSink) {
        let row = vec![lower(p)];
        if let Some(w) = self.useful(&[row], &[DPat::Wild], &[p.ty]) {
            let missing = self.show(&w[0], p.ty);
            diags.emit(
                Diagnostic::error("E0343", format!("the pattern of {} must always match", what))
                    .primary(p.span, format!("`{}` is not covered", missing))
                    .help("use `if let ... = value { ... }` or `match` to handle the other cases"),
            );
        }
    }

    fn block(&self, b: &Block, diags: &DiagSink) {
        for s in &b.stmts {
            match &s.kind {
                S::Let { pat, init } => {
                    self.expr(init, diags);
                    self.irrefutable(pat, "a `let`", diags);
                }
                S::Var { init, .. } => {
                    if let Some(e) = init {
                        self.expr(e, diags);
                    }
                }
                S::Assign { place, value } | S::CompoundAssign { place, value, .. } => {
                    self.expr(place, diags);
                    self.expr(value, diags);
                }
                S::Expr(e) => self.expr(e, diags),
                S::While { cond, body, .. } => {
                    self.expr(cond, diags);
                    self.block(body, diags);
                }
                S::ForRange { start, end, body, .. } => {
                    self.expr(start, diags);
                    self.expr(end, diags);
                    self.block(body, diags);
                }
                S::ForEach { pat, iter, body, .. } => {
                    self.expr(iter, diags);
                    self.irrefutable(pat, "a `for` loop", diags);
                    self.block(body, diags);
                }
            }
        }
        if let Some(t) = &b.tail {
            self.expr(t, diags);
        }
    }

    fn expr(&self, e: &Expr, diags: &DiagSink) {
        match &e.kind {
            T::Match { scrutinee, arms, .. } => {
                self.expr(scrutinee, diags);
                let ty = scrutinee.ty;
                let mut rows: Vec<Vec<DPat>> = Vec::new();
                let mut flat = FlatRows::default();
                for arm in arms {
                    let lp = lower(&arm.pat);
                    let unreachable = !self.p.tcx.has_error(ty)
                        && match (&flat, flat_ctors(&lp)) {
                            (FlatRows { general: false, .. }, Some(cs)) if !cs.is_empty() => flat.wild || cs.iter().all(|c| flat.ctors.contains(c)),
                            _ => self.useful(&rows, &[lp.clone()], &[ty]).is_none(),
                        };
                    if unreachable {
                        diags.emit(
                            Diagnostic::warning("W0002", "unreachable pattern")
                                .primary(arm.pat.span, "earlier arms already match every value of this pattern"),
                        );
                    }
                    if arm.guard.is_none() {
                        match flat_ctors(&lp) {
                            Some(cs) if cs.is_empty() => flat.wild = true,
                            Some(cs) => flat.ctors.extend(cs),
                            None => flat.general = true,
                        }
                        rows.push(vec![lp]);
                    }
                    if let Some(g) = &arm.guard {
                        self.expr(g, diags);
                    }
                    self.expr(&arm.body, diags);
                }
                if !self.p.tcx.has_error(ty) {
                    let missing = if flat.general { self.useful(&rows, &[DPat::Wild], &[ty]) } else { self.flat_witness(&flat, ty) };
                    if let Some(w) = missing {
                        let missing = self.show(&w[0], ty);
                        let mut d = Diagnostic::error("E0340", format!("non-exhaustive match: `{}` is not covered", missing))
                            .primary(scrutinee.span, format!("the value may be `{}`", missing));
                        if arms.iter().any(|a| a.guard.is_some()) {
                            d = d.note("arms with an `if` guard do not count towards exhaustiveness");
                        }
                        d = d.help(if missing == "_" {
                            "add an arm `_ => ...` to handle the remaining values".to_string()
                        } else {
                            format!("add an arm for `{}`, or `_ => ...`", missing)
                        });
                        diags.emit(d);
                    }
                }
            }
            T::IfLet { pat, scrutinee, then, else_, .. } => {
                self.expr(scrutinee, diags);
                let _ = pat;
                self.block(then, diags);
                if let Some(x) = else_ {
                    self.expr(x, diags);
                }
            }
            T::If { cond, then, else_ } => {
                self.expr(cond, diags);
                self.block(then, diags);
                if let Some(x) = else_ {
                    self.expr(x, diags);
                }
            }
            T::Block(b) => self.block(b, diags),
            T::Unary(_, x) | T::Field(x, _) | T::Move(x) | T::Try(x, _) | T::ArrayRepeat(x, _) | T::Cast(x) => self.expr(x, diags),
            T::Binary(_, a, b) | T::Index(a, b) => {
                self.expr(a, diags);
                self.expr(b, diags);
            }
            T::IndexFn { base, index, .. } => {
                self.expr(base, diags);
                self.expr(index, diags);
            }
            T::Call { callee, args } => {
                if let thir::Callee::Indirect(f) = callee {
                    self.expr(f, diags);
                }
                for a in args {
                    self.expr(&a.expr, diags);
                }
            }
            T::Adt { fields: xs, .. } | T::Tuple(xs) | T::Array(xs) => xs.iter().for_each(|x| self.expr(x, diags)),
            T::Return(Some(x)) => self.expr(x, diags),
            T::Interp(parts) => {
                for p in parts {
                    if let thir::InterpPart::Value(x, _) = p {
                        self.expr(x, diags);
                    }
                }
            }
            _ => {}
        }
    }
}

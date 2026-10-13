//! End of type checking for one body: default and substitute inference variables,
//! run the checks that needed complete types, verify literal ranges, and report
//! unused variables.

use super::{Deferred, FnCtxt, VarKind};
use crate::defs::ConstValue;
use crate::thir::{self, Block, Expr, ExprKind as T, InterpPart, LocalKind, Pat, PatKind, StmtKind as S};
use crate::ty::{FloatTy, Ty, TyKind};
use nxc_syntax::ast::{BinOp, Convention, UnOp};
use nxc_syntax::token::FormatKind;
use nxc_syntax::{Diagnostic, Span};

impl<'a> FnCtxt<'a> {
    /// Gives every unbound integer/float variable its default type, and reports
    /// variables that remain unknown.
    fn apply_defaults(&mut self) {
        let tcx = self.tcx();
        let mut reported = false;
        for i in 0..self.vars.len() {
            let root = self.shallow(tcx.mk(match self.vars[i].kind {
                VarKind::General => TyKind::Infer(i as u32),
                VarKind::Int => TyKind::IntVar(i as u32),
                VarKind::Float => TyKind::FloatVar(i as u32),
            }));
            match &*tcx.kind(root) {
                TyKind::IntVar(n) => {
                    let n = *n as usize;
                    self.vars[n].binding = Some(tcx.i32);
                }
                TyKind::FloatVar(n) => {
                    let n = *n as usize;
                    self.vars[n].binding = Some(tcx.f64);
                }
                _ => {}
            }
        }
        // Remaining general variables: report the earliest one.
        for i in 0..self.vars.len() {
            if self.vars[i].kind != VarKind::General {
                continue;
            }
            let t = self.shallow(tcx.mk(TyKind::Infer(i as u32)));
            if let TyKind::Infer(n) = &*tcx.kind(t) {
                let n = *n as usize;
                if !reported {
                    reported = true;
                    let span = self.vars[n].span;
                    self.err(
                        Diagnostic::error("E0320", "cannot infer a type here")
                            .primary(span, "type unknown")
                            .help("add a type annotation, for example `let v: Vec[i32] = Vec.new()`"),
                    );
                }
                self.vars[n].binding = Some(tcx.error);
            }
        }
    }

    pub(crate) fn finish(&mut self, body: &mut thir::Body) {
        self.apply_defaults();
        let resolved: Vec<Ty> = self.locals.iter().map(|l| self.resolve(l.ty)).collect();
        for (l, t) in self.locals.iter_mut().zip(resolved) {
            l.ty = t;
        }
        self.fin_block(&mut body.block);
        let deferred = std::mem::take(&mut self.deferred);
        for d in deferred {
            self.run_deferred(d);
        }
        self.report_unused();
        body.locals = std::mem::take(&mut self.locals);
    }

    fn report_unused(&self) {
        for (i, l) in self.locals.iter().enumerate() {
            let u = self.uses[i];
            if l.name.starts_with('_') || l.name == "self" || l.span.is_dummy() {
                continue;
            }
            let is_std = self.p.modules[self.module.idx()].is_std;
            if is_std {
                continue;
            }
            if !u.read {
                let what = match l.kind {
                    LocalKind::Param(_) => "parameter",
                    _ => "variable",
                };
                self.err(
                    Diagnostic::warning("W0001", format!("unused {} `{}`", what, l.name))
                        .primary(l.span, "never read")
                        .help(format!("if this is intentional, name it `_{}`", l.name)),
                );
            } else if l.kind == LocalKind::Var && !u.mutated {
                self.err(
                    Diagnostic::warning("W0003", format!("variable `{}` is never modified", l.name))
                        .primary(l.span, "declared with `var`")
                        .help("declare it with `let`"),
                );
            }
        }
    }

    fn fin_ty(&self, t: &mut Ty) {
        *t = self.resolve(*t);
    }

    fn fin_block(&mut self, b: &mut Block) {
        self.fin_ty(&mut b.ty);
        for s in b.stmts.iter_mut() {
            match &mut s.kind {
                S::Let { pat, init } => {
                    self.fin_expr(init);
                    self.fin_pat(pat);
                }
                S::Var { init, .. } => {
                    if let Some(e) = init {
                        self.fin_expr(e);
                    }
                }
                S::Assign { place, value } => {
                    self.fin_expr(place);
                    self.fin_expr(value);
                }
                S::CompoundAssign { place, value, .. } => {
                    self.fin_expr(place);
                    self.fin_expr(value);
                }
                S::Expr(e) => self.fin_expr(e),
                S::While { cond, body, .. } => {
                    self.fin_expr(cond);
                    self.fin_block(body);
                }
                S::ForRange { start, end, body, .. } => {
                    self.fin_expr(start);
                    self.fin_expr(end);
                    self.fin_block(body);
                }
                S::ForEach { pat, iter, elem_ty, body, kind, .. } => {
                    self.fin_expr(iter);
                    self.fin_pat(pat);
                    self.fin_ty(elem_ty);
                    if let thir::IterKind::Indexed { type_args, .. } = kind {
                        for t in type_args.iter_mut() {
                            self.fin_ty(t);
                        }
                    }
                    self.fin_block(body);
                }
            }
        }
        if let Some(t) = &mut b.tail {
            self.fin_expr(t);
        }
    }

    fn fin_pat(&mut self, p: &mut Pat) {
        self.fin_ty(&mut p.ty);
        match &mut p.kind {
            PatKind::Tuple(ps) | PatKind::Or(ps) => ps.iter_mut().for_each(|p| self.fin_pat(p)),
            PatKind::Variant { type_args, fields, .. } | PatKind::Struct { type_args, fields, .. } => {
                for t in type_args.iter_mut() {
                    self.fin_ty(t);
                }
                fields.iter_mut().for_each(|p| self.fin_pat(p));
            }
            PatKind::Lit(v) => {
                if let ConstValue::Int(n) = v {
                    self.check_int_range(*n, p.ty, p.span);
                }
            }
            PatKind::Range(a, b, _) => {
                for v in [a, b] {
                    if let ConstValue::Int(n) = v {
                        self.check_int_range(*n, p.ty, p.span);
                    }
                }
            }
            _ => {}
        }
    }

    fn check_int_range(&self, n: i128, ty: Ty, span: Span) -> bool {
        if let Some(i) = self.tcx().as_int(ty) {
            if n < i.min() || n > i.max() {
                let mut d = Diagnostic::error("E0301", format!("literal out of range for `{}`", i.name()))
                    .primary(span, format!("the range of `{}` is {}..={}", i.name(), i.min(), i.max()));
                if n >= 0 && i.signed() && i.bits() < 64 {
                    d = d.help("use a wider type, such as `i64`");
                } else if n >= 0 && i.signed() && n <= u64::MAX as i128 {
                    d = d.help("use `u64` for values above `i64.MAX`");
                } else if n < 0 && !i.signed() {
                    d = d.note(format!("`{}` cannot hold negative values", i.name()));
                }
                self.err(d);
                return false;
            }
        }
        true
    }

    fn fin_expr(&mut self, e: &mut Expr) {
        self.fin_ty(&mut e.ty);
        let tcx = self.tcx();
        match &mut e.kind {
            T::Lit(ConstValue::Int(n)) => {
                let n = *n;
                if let Some(f) = tcx.as_float(e.ty) {
                    // Integer literal in a float context: it must be exact.
                    let x = n as f64;
                    let exact = (x as i128) == n && (f == FloatTy::F64 || (x as f32) as f64 == x);
                    if !exact {
                        self.err(Diagnostic::error("E0301", format!("{} cannot be represented exactly as `{}`", n, f.name())).primary(e.span, "inexact literal"));
                    }
                    e.kind = T::Lit(ConstValue::Float(x));
                } else {
                    self.check_int_range(n, e.ty, e.span);
                }
            }
            T::Lit(_) | T::Local(_) | T::Const(_) | T::FnRef(_) | T::Error | T::Break(_) | T::Continue(_) => {}
            T::Unary(_, x) | T::Field(x, _) | T::Move(x) | T::Try(x, _) | T::ArrayRepeat(x, _) => self.fin_expr(x),
            T::Cast(x) => {
                self.fin_expr(x);
                // Compile-time check of literal conversions (spec §3.6).
                if let (T::Lit(ConstValue::Int(n)), Some(_)) = (&x.kind, tcx.as_int(e.ty)) {
                    let n = *n;
                    self.check_int_range(n, e.ty, e.span);
                }
            }
            T::Binary(_, a, b) | T::Index(a, b) => {
                self.fin_expr(a);
                self.fin_expr(b);
            }
            T::IndexFn { base, index, type_args, .. } => {
                self.fin_expr(base);
                self.fin_expr(index);
                for t in type_args.iter_mut() {
                    self.fin_ty(t);
                }
            }
            T::Call { callee, args } => {
                match callee {
                    thir::Callee::Fn(_, targs) => {
                        for t in targs.iter_mut() {
                            self.fin_ty(t);
                        }
                    }
                    thir::Callee::Indirect(f) => self.fin_expr(f),
                    _ => {}
                }
                for a in args.iter_mut() {
                    self.fin_expr(&mut a.expr);
                }
            }
            T::Adt { type_args, fields, .. } => {
                for t in type_args.iter_mut() {
                    self.fin_ty(t);
                }
                fields.iter_mut().for_each(|f| self.fin_expr(f));
            }
            T::Tuple(xs) | T::Array(xs) => xs.iter_mut().for_each(|x| self.fin_expr(x)),
            T::If { cond, then, else_ } => {
                self.fin_expr(cond);
                self.fin_block(then);
                if let Some(x) = else_ {
                    self.fin_expr(x);
                }
            }
            T::IfLet { pat, scrutinee, then, else_, .. } => {
                self.fin_expr(scrutinee);
                self.fin_pat(pat);
                self.fin_block(then);
                if let Some(x) = else_ {
                    self.fin_expr(x);
                }
            }
            T::Match { scrutinee, arms, .. } => {
                self.fin_expr(scrutinee);
                for a in arms.iter_mut() {
                    self.fin_pat(&mut a.pat);
                    if let Some(g) = &mut a.guard {
                        self.fin_expr(g);
                    }
                    self.fin_expr(&mut a.body);
                }
            }
            T::Block(b) => self.fin_block(b),
            T::Return(x) => {
                if let Some(x) = x {
                    self.fin_expr(x);
                }
            }
            T::Interp(parts) => {
                for p in parts.iter_mut() {
                    if let InterpPart::Value(x, _) = p {
                        self.fin_expr(x);
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Deferred checks

    fn is_formattable(&self, t: Ty) -> bool {
        matches!(&*self.tcx().kind(t), TyKind::Int(_) | TyKind::Float(_) | TyKind::Bool | TyKind::Char | TyKind::Error) || self.p.is_string(t)
    }

    fn is_fieldless_enum(&self, t: Ty) -> bool {
        match self.tcx().as_adt(t) {
            Some((id, _)) => {
                let adt = self.p.adt(id);
                adt.is_enum() && adt.variants().iter().all(|v| v.fields.is_empty())
            }
            None => false,
        }
    }

    fn run_deferred(&mut self, d: Deferred) {
        let tcx = self.tcx();
        match d {
            Deferred::Format { ty, spec, span } => {
                let t = self.resolve(ty);
                if tcx.has_error(t) {
                    return;
                }
                if !self.is_formattable(t) {
                    let mut d = Diagnostic::error("E0350", format!("a value of type `{}` cannot be formatted", self.ty_str(t)))
                        .primary(span, "not formattable")
                        .note("NEXUL 0.1 formats integers, floats, `bool`, `char` and `String`");
                    if self.tcx().as_adt(t).is_some() {
                        d = d.help("format its fields individually, or write a method that returns a `String`");
                    }
                    self.err(d);
                    return;
                }
                if let Some(spec) = spec {
                    let k = tcx.kind(t);
                    let is_int = matches!(&*k, TyKind::Int(_));
                    let is_float = matches!(&*k, TyKind::Float(_));
                    let bad = |what: &str| Diagnostic::error("E0351", format!("invalid format for `{}`: {}", self.ty_str(t), what)).primary(spec.span, "incompatible specification");
                    if spec.precision.is_some() && !is_float && !self.p.is_string(t) {
                        self.err(bad("precision applies to floats and strings"));
                    }
                    match spec.kind {
                        Some(FormatKind::Exponent) if !is_float => self.err(bad("`e` applies to floats")),
                        Some(FormatKind::LowerHex | FormatKind::UpperHex | FormatKind::Octal | FormatKind::Binary) if !is_int => {
                            self.err(bad("`x`, `X`, `o` and `b` apply to integers"))
                        }
                        _ => {}
                    }
                    if (spec.plus || spec.zero) && !is_int && !is_float {
                        self.err(bad("`+` and `0` apply to numbers"));
                    }
                }
            }
            Deferred::Binary { op, lhs, rhs, span } => {
                let (a, b) = (self.resolve(lhs), self.resolve(rhs));
                if tcx.has_error(a) || tcx.has_error(b) || tcx.is_never(a) || tcx.is_never(b) {
                    return;
                }
                let k = tcx.kind(a);
                let is_int = matches!(&*k, TyKind::Int(_));
                let is_float = matches!(&*k, TyKind::Float(_));
                let is_str = self.p.is_string(a);
                let ok = match op {
                    BinOp::Add => is_int || is_float || is_str,
                    BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => is_int || is_float,
                    BinOp::AddWrap | BinOp::SubWrap | BinOp::MulWrap => is_int,
                    BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => is_int,
                    BinOp::Eq | BinOp::Ne => {
                        is_int || is_float || is_str || matches!(&*k, TyKind::Bool | TyKind::Char | TyKind::Ptr(_) | TyKind::Unit) || self.is_fieldless_enum(a)
                    }
                    BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => is_int || is_float || is_str || matches!(&*k, TyKind::Char),
                    BinOp::Shl | BinOp::Shr | BinOp::And | BinOp::Or => true,
                };
                if !ok {
                    let ts = self.ty_str(a);
                    let mut d = Diagnostic::error("E0303", format!("operator `{}` cannot be applied to `{}`", op.symbol(), ts)).primary(span, "unsupported operation");
                    if matches!(op, BinOp::AddWrap | BinOp::SubWrap | BinOp::MulWrap) && is_float {
                        d = d.note("wrapping arithmetic only exists for integers");
                    } else if matches!(&*k, TyKind::Param(_)) {
                        d = d.note("operations on generic types will come with traits (NEXUL 0.2)");
                    } else if op.is_comparison() && tcx.as_adt(a).is_some() {
                        d = d.help("compare the fields, or use `match`");
                    } else if matches!(&*k, TyKind::Bool) && matches!(op, BinOp::BitAnd | BinOp::BitOr) {
                        d = d.help(if op == BinOp::BitAnd { "use `and` for booleans" } else { "use `or` for booleans" });
                    }
                    self.err(d);
                }
            }
            Deferred::Unary { op, ty, span } => {
                let t = self.resolve(ty);
                if tcx.has_error(t) {
                    return;
                }
                let k = tcx.kind(t);
                let ok = match op {
                    UnOp::Neg => matches!(&*k, TyKind::Float(_)) || matches!(&*k, TyKind::Int(i) if i.signed()),
                    UnOp::BitNot => matches!(&*k, TyKind::Int(_)),
                    UnOp::Not => true,
                };
                if !ok {
                    let sym = if op == UnOp::Neg { "-" } else { "~" };
                    let mut d = Diagnostic::error("E0303", format!("unary `{}` cannot be applied to `{}`", sym, self.ty_str(t))).primary(span, "unsupported operation");
                    if op == UnOp::Neg && matches!(&*k, TyKind::Int(_)) {
                        d = d.note("unsigned integers cannot be negated");
                    } else if op == UnOp::BitNot && matches!(&*k, TyKind::Bool) {
                        d = d.help("use `not` for booleans");
                    }
                    self.err(d);
                }
            }
            Deferred::Cast { from, to, span } => {
                let (f, t) = (self.resolve(from), self.resolve(to));
                if tcx.has_error(f) || tcx.has_error(t) || tcx.is_never(f) {
                    return;
                }
                let (kf, kt) = (tcx.kind(f), tcx.kind(t));
                use TyKind::*;
                let ok = match (&*kf, &*kt) {
                    (Int(_), Int(_) | Float(_) | Char) => true,
                    (Float(_), Int(_) | Float(_)) => true,
                    (Bool, Int(_)) => true,
                    (Char, Int(_)) => true,
                    (Ptr(_), Ptr(_)) => true,
                    _ => f == t,
                };
                if !ok {
                    let mut d = Diagnostic::error("E0322", format!("cannot convert `{}` to `{}` with `as`", self.ty_str(f), self.ty_str(t))).primary(span, "invalid conversion");
                    if self.p.is_string(t) {
                        d = d.help("to build a string, use `value.to_string()` or interpolation `\"{value}\"`");
                    } else if self.p.is_string(f) {
                        d = d.help("to read a number from a string, use `s.parse_int()` or `s.parse_float()`");
                    } else if matches!(&*kt, Bool) {
                        d = d.help("compare instead: `x != 0`");
                    }
                    self.err(d);
                } else if f == t && !matches!(&*kf, Error) {
                    self.err(Diagnostic::warning("W0008", format!("useless conversion: the value is already `{}`", self.ty_str(t))).primary(span, "remove `as`"));
                }
            }
            Deferred::Integer { ty, span, what } => {
                let t = self.resolve(ty);
                if tcx.has_error(t) || tcx.is_never(t) {
                    return;
                }
                if tcx.as_int(t).is_none() {
                    self.err(Diagnostic::error("E0302", format!("{} must be an integer", what)).primary(span, format!("found `{}`", self.ty_str(t))));
                }
            }
            Deferred::SignedInt { ty, span } => {
                let t = self.resolve(ty);
                if let Some(i) = tcx.as_int(t) {
                    if !i.signed() {
                        self.err(Diagnostic::error("E0308", format!("`abs` does not exist for unsigned type `{}`", i.name())).primary(span, "already non-negative"));
                    }
                }
            }
            Deferred::UnusedResult { ty, span } => {
                let t = self.resolve(ty);
                if let Some((id, _)) = tcx.as_adt(t) {
                    if Some(id) == self.p.lang.result {
                        self.err(
                            Diagnostic::warning("W0005", "this `Result` is ignored: it may contain an error")
                                .primary(span, "unused `Result`")
                                .help("handle it with `match`, `?` or `unwrap()`, or ignore it explicitly with `let _ = ...`"),
                        );
                    }
                }
            }
            Deferred::Copyable { ty, span, what } => {
                let t = self.resolve(ty);
                if !tcx.has_error(t) && !self.p.is_copy(t) {
                    self.err(
                        Diagnostic::error("E0313", format!("{} must be copyable", what))
                            .primary(span, format!("`{}` owns resources and cannot be duplicated", self.ty_str(t)))
                            .help("build the array element by element, or use a `Vec`"),
                    );
                }
            }
        }
    }
}

#[allow(dead_code)]
fn conv_is_ref(c: Convention) -> bool {
    c != Convention::Owned
}

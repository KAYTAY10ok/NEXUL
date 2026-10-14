//! Expression checking.

use super::{Deferred, Expect, FnCtxt, VarKind};
use crate::check::call::{PathLookup, PathRes};
use crate::defs::{AdtKind, ConstValue};
use crate::thir::{self, ExprKind as T, InterpPart, LocalKind, TryKind};
use crate::ty::{Ty, TyKind};
use crate::util::best_match;
use nxc_syntax::ast::{self, BinOp, Cond, ExprKind as A, Lit, UnOp};
use nxc_syntax::{Diagnostic, Span};

fn is_float_literal(e: &ast::Expr) -> bool {
    match &e.kind {
        A::Lit(Lit::Float(_)) => true,
        A::Unary(UnOp::Neg, inner) | A::Paren(inner) => is_float_literal(inner),
        _ => false,
    }
}

fn is_literal(e: &ast::Expr) -> bool {
    match &e.kind {
        A::Lit(Lit::Int(_)) | A::Lit(Lit::Float(_)) => true,
        A::Unary(UnOp::Neg, inner) | A::Paren(inner) => is_literal(inner),
        _ => false,
    }
}

impl<'a> FnCtxt<'a> {
    pub(crate) fn mk(&self, kind: T, ty: Ty, span: Span) -> thir::Expr {
        thir::Expr { kind, ty, span }
    }

    pub(crate) fn error_expr(&self, span: Span) -> thir::Expr {
        self.mk(T::Error, self.tcx().error, span)
    }

    /// Checks `e` and coerces it to `ty`.
    pub(crate) fn check_expr_coerce(&mut self, e: &ast::Expr, ty: Ty) -> thir::Expr {
        let mut x = self.check_expr(e, Expect::Has(ty));
        self.coerce(&mut x, ty);
        x
    }

    pub(crate) fn check_expr(&mut self, e: &ast::Expr, expect: Expect) -> thir::Expr {
        let tcx = self.tcx();
        let span = e.span;
        match &e.kind {
            A::Error => self.error_expr(span),
            A::Lit(lit) => self.check_lit(lit, expect, span, false),
            A::Interp(parts) => {
                let mut out = Vec::new();
                for part in parts {
                    match part {
                        ast::StrPartAst::Text(t) => out.push(InterpPart::Text(t.clone())),
                        ast::StrPartAst::Interp { expr, spec } => {
                            let x = self.check_expr(expr, Expect::None);
                            self.deferred.push(Deferred::Format { ty: x.ty, spec: spec.clone(), span: x.span });
                            out.push(InterpPart::Value(x, spec.clone()));
                        }
                    }
                }
                self.mk(T::Interp(out), self.p.string_ty(), span)
            }
            A::Unit => self.mk(T::Lit(ConstValue::Unit), tcx.unit, span),
            A::Paren(inner) => self.check_expr(inner, expect),
            A::Ident(_) | A::SelfValue | A::SelfType | A::Field(..) => self.check_path_expr(e, expect),
            A::TupleField(base, idx, idx_span) => {
                let b = self.check_expr(base, Expect::None);
                let bt = self.shallow(b.ty);
                match &*tcx.kind(bt) {
                    TyKind::Tuple(elems) => match elems.get(*idx as usize) {
                        Some(&t) => self.mk(T::Field(Box::new(b), *idx), t, span),
                        None => {
                            self.err(
                                Diagnostic::error("E0309", format!("tuple `{}` has no field `{}`", self.ty_str(bt), idx))
                                    .primary(*idx_span, format!("the tuple has {} element{}", elems.len(), if elems.len() == 1 { "" } else { "s" })),
                            );
                            self.error_expr(span)
                        }
                    },
                    TyKind::Error => self.error_expr(span),
                    TyKind::Infer(_) => {
                        self.need_type(b.span);
                        self.error_expr(span)
                    }
                    _ => {
                        self.err(Diagnostic::error("E0309", format!("`{}` is not a tuple", self.ty_str(bt))).primary(*idx_span, "numbered fields only exist on tuples"));
                        self.error_expr(span)
                    }
                }
            }
            A::Tuple(elems) => {
                let hints: Vec<Option<Ty>> = match expect.ty().map(|t| self.shallow(t)).map(|t| tcx.kind(t)) {
                    Some(k) => match &*k {
                        TyKind::Tuple(ts) if ts.len() == elems.len() => ts.iter().map(|&t| Some(t)).collect(),
                        _ => vec![None; elems.len()],
                    },
                    None => vec![None; elems.len()],
                };
                let xs: Vec<thir::Expr> = elems
                    .iter()
                    .zip(hints)
                    .map(|(el, h)| match h {
                        Some(t) => self.check_expr(el, Expect::Has(t)),
                        None => self.check_expr(el, Expect::None),
                    })
                    .collect();
                let ty = tcx.tuple(xs.iter().map(|x| x.ty).collect());
                self.mk(T::Tuple(xs), ty, span)
            }
            A::Array(elems) => {
                let hint = expect.ty().map(|t| self.shallow(t)).and_then(|t| match &*tcx.kind(t) {
                    TyKind::Array(e, _) => Some(*e),
                    _ => None,
                });
                let elem_ty = match hint {
                    Some(t) => t,
                    None => self.new_var(span),
                };
                let mut xs = Vec::new();
                for el in elems {
                    let mut x = self.check_expr(el, Expect::Has(elem_ty));
                    self.coerce(&mut x, elem_ty);
                    xs.push(x);
                }
                let ty = tcx.mk(TyKind::Array(elem_ty, xs.len() as u64));
                self.mk(T::Array(xs), ty, span)
            }
            A::ArrayRepeat(el, count) => {
                let hint = expect.ty().map(|t| self.shallow(t)).and_then(|t| match &*tcx.kind(t) {
                    TyKind::Array(e, _) => Some(*e),
                    _ => None,
                });
                let x = match hint {
                    Some(t) => self.check_expr_coerce(el, t),
                    None => self.check_expr(el, Expect::None),
                };
                let n = crate::consts::eval_array_len(self.p, self.module, count, self.diags).unwrap_or(0);
                self.deferred.push(Deferred::Copyable { ty: x.ty, span: x.span, what: "the element of an array repeat expression `[x; n]`" });
                let ty = tcx.mk(TyKind::Array(x.ty, n));
                self.mk(T::ArrayRepeat(Box::new(x), n), ty, span)
            }
            A::Unary(op, inner) => self.check_unary(*op, inner, expect, span),
            A::Binary(op, l, r) => self.check_binary(*op, l, r, expect, span),
            A::Cast(inner, texpr) => {
                let to = self.resolve_type(texpr);
                if let TyKind::Dyn(_) = &*tcx.kind(to) {
                    // `value as dyn Trait`
                    let mut x = self.check_expr(inner, Expect::None);
                    self.coerce(&mut x, to);
                    return x;
                }
                let x = self.check_expr(inner, Expect::None);
                if matches!(&*tcx.kind(self.shallow(x.ty)), TyKind::Ptr(_)) || matches!(&*tcx.kind(to), TyKind::Ptr(_)) {
                    self.require_unsafe(span, "a pointer conversion");
                }
                self.deferred.push(Deferred::Cast { from: x.ty, to, span });
                self.mk(T::Cast(Box::new(x)), to, span)
            }
            A::Call(callee, args) => self.check_call(callee, args, expect, span),
            A::Index(base, args) => self.check_index(e, base, args, expect, span),
            A::Try(inner) => self.check_try(inner, span),
            A::Move(inner) => {
                let x = self.check_expr(inner, expect);
                let ok = match &x.kind {
                    T::Local(l) => {
                        let k = self.locals[*l as usize].kind;
                        matches!(k, LocalKind::Let | LocalKind::Var | LocalKind::Hidden | LocalKind::Param(ast::Convention::Owned))
                    }
                    T::Error => true,
                    _ => false,
                };
                if !ok {
                    let mut d = Diagnostic::error("E0406", "`move` can only take a whole variable that owns its value").primary(x.span, "not an owned variable");
                    if let T::Local(_) = &x.kind {
                        d = d.note("parameters without `owned`, `for` elements and pattern bindings are borrowed");
                    } else {
                        d = d.note("moving a part of a value would leave the rest half-initialized");
                    }
                    self.err(d);
                }
                let ty = x.ty;
                self.mk(T::Move(Box::new(x)), ty, span)
            }
            A::If { cond, then, else_ } => self.check_if(cond, then, else_.as_deref(), expect, span),
            A::Match { scrutinee, arms, mutable } => self.check_match(scrutinee, arms, *mutable, expect, span),
            A::Block(b) => {
                let blk = self.check_block(b, expect);
                let ty = blk.ty;
                self.mk(T::Block(blk), ty, span)
            }
            A::Unsafe(b) => {
                self.unsafe_depth += 1;
                let blk = self.check_block(b, expect);
                self.unsafe_depth -= 1;
                let ty = blk.ty;
                self.mk(T::Block(blk), ty, span)
            }
            A::Return(value) => {
                let ret = self.ret_ty;
                let v = match value {
                    Some(v) => {
                        if ret == tcx.unit {
                            let x = self.check_expr(v, Expect::None);
                            if !self.unify(x.ty, tcx.unit) && !tcx.is_never(self.shallow(x.ty)) {
                                self.err(
                                    Diagnostic::error("E0302", "this function does not return a value")
                                        .primary(x.span, format!("unexpected value of type `{}`", self.ty_str(x.ty)))
                                        .help("declare the return type with `-> Type`"),
                                );
                            }
                            Some(Box::new(x))
                        } else {
                            let mut x = self.check_expr(v, Expect::Has(ret));
                            self.coerce(&mut x, ret);
                            Some(Box::new(x))
                        }
                    }
                    None => {
                        if ret != tcx.unit && !tcx.is_error(ret) {
                            self.err(
                                Diagnostic::error("E0302", format!("`return` needs a value of type `{}`", self.ty_str(ret)))
                                    .primary(span, "missing value"),
                            );
                        }
                        None
                    }
                };
                self.mk(T::Return(v), tcx.never, span)
            }
            A::Break(label) | A::Continue(label) => {
                let is_break = matches!(&e.kind, A::Break(_));
                let kw = if is_break { "break" } else { "continue" };
                let idx = match label {
                    Some(l) => match self.loops.iter().rposition(|lc| lc.label.as_deref() == Some(l.name.as_str())) {
                        Some(i) => Some(i),
                        None => {
                            let labels: Vec<String> = self.loops.iter().filter_map(|l| l.label.clone()).collect();
                            let mut d = Diagnostic::error("E0207", format!("no enclosing loop labeled `{}`", l.name)).primary(l.span, "unknown label");
                            if let Some(s) = best_match(&l.name, labels.iter().map(|s| s.as_str())) {
                                d = d.help(format!("did you mean `{}`?", s));
                            }
                            self.err(d);
                            return self.mk(T::Error, tcx.never, span);
                        }
                    },
                    None => {
                        if self.loops.is_empty() {
                            self.err(Diagnostic::error("E0207", format!("`{}` outside of a loop", kw)).primary(span, format!("`{}` can only be used inside `while` or `for`", kw)));
                            return self.mk(T::Error, tcx.never, span);
                        }
                        Some(self.loops.len() - 1)
                    }
                };
                let i = idx.unwrap();
                if is_break {
                    self.loops[i].has_break = true;
                }
                let id = self.loops[i].id;
                self.mk(if is_break { T::Break(id) } else { T::Continue(id) }, tcx.never, span)
            }
            A::Type(_) => {
                self.err(Diagnostic::error("E0314", "expected a value, found a type").primary(span, "a type cannot be used as a value"));
                self.error_expr(span)
            }
            A::Closure(_) => self.misplaced_closure(span),
        }
    }

    pub(crate) fn need_type(&self, span: Span) {
        self.err(
            Diagnostic::error("E0320", "the type of this value must be known at this point")
                .primary(span, "type not yet known")
                .help("add a type annotation, for example `let v: Vec[i32] = Vec.new()`"),
        );
    }

    pub(crate) fn check_lit(&mut self, lit: &Lit, expect: Expect, span: Span, negated: bool) -> thir::Expr {
        let tcx = self.tcx();
        match lit {
            Lit::Int(v) => {
                let v = if negated { -(*v as i128) } else { *v as i128 };
                let hint = expect.ty().map(|t| self.shallow(t));
                let hint_kind = hint.map(|t| tcx.kind(t));
                match hint_kind.as_deref() {
                    Some(TyKind::Float(_)) | Some(TyKind::FloatVar(_)) => {
                        // An integer literal in a float context is a float (spec §3.5); the
                        // conversion and its exactness check happen at finalization.
                        self.mk(T::Lit(ConstValue::Int(v)), hint.unwrap(), span)
                    }
                    Some(TyKind::Int(_)) | Some(TyKind::IntVar(_)) => self.mk(T::Lit(ConstValue::Int(v)), hint.unwrap(), span),
                    _ => {
                        let t = self.new_var_of(VarKind::Int, span);
                        self.mk(T::Lit(ConstValue::Int(v)), t, span)
                    }
                }
            }
            Lit::Float(f) => {
                let f = if negated { -*f } else { *f };
                let hint = expect.ty().map(|t| self.shallow(t));
                let ty = match hint.map(|t| tcx.kind(t)).as_deref() {
                    Some(TyKind::Float(_)) | Some(TyKind::FloatVar(_)) => hint.unwrap(),
                    _ => self.new_var_of(VarKind::Float, span),
                };
                self.mk(T::Lit(ConstValue::Float(f)), ty, span)
            }
            Lit::Bool(b) => self.mk(T::Lit(ConstValue::Bool(*b)), tcx.bool, span),
            Lit::Char(c) => self.mk(T::Lit(ConstValue::Char(*c)), tcx.char, span),
            Lit::Str(s) => self.mk(T::Lit(ConstValue::Str(s.clone())), self.p.string_ty(), span),
        }
    }

    fn check_unary(&mut self, op: UnOp, inner: &ast::Expr, expect: Expect, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        match op {
            UnOp::Neg => {
                // `-literal` is a single (possibly minimal) constant: `-128` is a valid `i8`.
                let mut lit_inner = inner;
                while let A::Paren(x) = &lit_inner.kind {
                    lit_inner = x;
                }
                if let A::Lit(l @ (Lit::Int(_) | Lit::Float(_))) = &lit_inner.kind {
                    return self.check_lit(l, expect, span, true);
                }
                let x = match expect.ty() {
                    Some(t) => self.check_expr(inner, Expect::Has(t)),
                    None => self.check_expr(inner, Expect::None),
                };
                self.deferred.push(Deferred::Unary { op, ty: x.ty, span });
                let ty = x.ty;
                self.mk(T::Unary(op, Box::new(x)), ty, span)
            }
            UnOp::Not => {
                let x = self.check_expr_coerce(inner, tcx.bool);
                self.mk(T::Unary(op, Box::new(x)), tcx.bool, span)
            }
            UnOp::BitNot => {
                let x = match expect.ty() {
                    Some(t) => self.check_expr(inner, Expect::Has(t)),
                    None => self.check_expr(inner, Expect::None),
                };
                self.deferred.push(Deferred::Unary { op, ty: x.ty, span });
                let ty = x.ty;
                self.mk(T::Unary(op, Box::new(x)), ty, span)
            }
        }
    }

    fn check_binary(&mut self, op: BinOp, l: &ast::Expr, r: &ast::Expr, expect: Expect, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        if op.is_logical() {
            let a = self.check_expr_coerce(l, tcx.bool);
            let b = self.check_expr_coerce(r, tcx.bool);
            return self.mk(T::Binary(op, Box::new(a), Box::new(b)), tcx.bool, span);
        }
        if op.is_shift() {
            let hint = if op.is_comparison() { None } else { expect.ty() };
            let a = match hint {
                Some(t) => self.check_expr(l, Expect::Has(t)),
                None => self.check_expr(l, Expect::None),
            };
            let b = self.check_expr(r, Expect::None);
            self.deferred.push(Deferred::Integer { ty: a.ty, span: a.span, what: "the left operand of a shift" });
            self.deferred.push(Deferred::Integer { ty: b.ty, span: b.span, what: "a shift amount" });
            let ty = a.ty;
            return self.mk(T::Binary(op, Box::new(a), Box::new(b)), ty, span);
        }
        // Arithmetic, bitwise and comparison: both operands have the same type.
        // A literal operand takes its type from the other operand (`2 * pi`).
        let hint = if op.is_comparison() { None } else { expect.ty() };
        // Between two literals, the float one decides (`2 * 3.0` is a float product).
        let right_first = (is_literal(l) && !is_literal(r)) || (is_literal(l) && is_float_literal(r) && !is_float_literal(l));
        let (a, b) = if right_first {
            let b = match hint {
                Some(t) => self.check_expr(r, Expect::Has(t)),
                None => self.check_expr(r, Expect::None),
            };
            let a = self.check_expr(l, Expect::Has(b.ty));
            (a, b)
        } else {
            let a = match hint {
                Some(t) => self.check_expr(l, Expect::Has(t)),
                None => self.check_expr(l, Expect::None),
            };
            let b = self.check_expr(r, Expect::Has(a.ty));
            (a, b)
        };
        let never_a = tcx.is_never(self.shallow(a.ty));
        let never_b = tcx.is_never(self.shallow(b.ty));
        if !never_a && !never_b && !self.unify(a.ty, b.ty) {
            let (sa, sb) = (self.ty_str(a.ty), self.ty_str(b.ty));
            let mut d = Diagnostic::error("E0302", format!("mismatched operand types for `{}`", op.symbol()))
                .primary(span, format!("`{}` {} `{}`", sa, op.symbol(), sb))
                .secondary(a.span, format!("`{}`", sa))
                .secondary(b.span, format!("`{}`", sb));
            let numeric = |t: Ty| matches!(&*tcx.kind(self.resolve(t)), TyKind::Int(_) | TyKind::Float(_) | TyKind::IntVar(_) | TyKind::FloatVar(_));
            if numeric(a.ty) && numeric(b.ty) {
                d = d.help(format!("convert one operand explicitly, for example `... as {}`", sa));
            } else if self.p.is_string(self.resolve(a.ty)) && op == BinOp::Add {
                d = d.help("to append a value to a string, use interpolation: `\"{text}{value}\"`");
            }
            self.err(d);
        }
        let operand_ty = if never_a { b.ty } else { a.ty };
        self.deferred.push(Deferred::Binary { op, lhs: a.ty, rhs: b.ty, span });
        let ty = if op.is_comparison() { tcx.bool } else { operand_ty };
        self.mk(T::Binary(op, Box::new(a), Box::new(b)), ty, span)
    }

    fn check_index(&mut self, whole: &ast::Expr, base: &ast::Expr, args: &[ast::Expr], _expect: Expect, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        // Generic instantiation used as a value (`size_of[i32]` without a call).
        match self.resolve_path(whole) {
            PathLookup::NotPath => {}
            PathLookup::Err => return self.error_expr(span),
            PathLookup::Res(PathRes::Local(_)) => {}
            PathLookup::Res(_) => {
                self.err(
                    Diagnostic::error("E0316", "a generic item with explicit arguments must be called or used as a namespace")
                        .primary(span, "add `(...)` or `.member`"),
                );
                return self.error_expr(span);
            }
        }
        let b = self.check_expr(base, Expect::None);
        if args.len() != 1 {
            self.err(Diagnostic::error("E0317", "indexing takes exactly one index").primary(span, format!("{} indices given", args.len())));
            return self.error_expr(span);
        }
        let idx = self.check_expr(&args[0], Expect::None);
        self.deferred.push(Deferred::Integer { ty: idx.ty, span: idx.span, what: "an index" });
        let bt = self.shallow(b.ty);
        match &*tcx.kind(bt) {
            TyKind::Array(elem, _) => {
                let elem = *elem;
                self.mk(T::Index(Box::new(b), Box::new(idx)), elem, span)
            }
            TyKind::Adt(id, targs) => {
                let adt = self.p.adt(*id);
                if let Some(f) = adt.index_fn {
                    let fd = self.p.func(f);
                    // The @index function returns `Ptr[E]`: the element type is `E`.
                    let ret = tcx.subst(fd.ret, targs);
                    let elem = match &*tcx.kind(ret) {
                        TyKind::Ptr(e) => *e,
                        _ => tcx.error,
                    };
                    let targs = targs.clone();
                    return self.mk(T::IndexFn { base: Box::new(b), index: Box::new(idx), index_fn: f, type_args: targs }, elem, span);
                }
                let mut d = Diagnostic::error("E0317", format!("cannot index into a value of type `{}`", self.ty_str(bt))).primary(span, "not indexable");
                if self.p.is_string(bt) {
                    d = d
                        .note("strings are UTF-8: a character may span several bytes")
                        .help("use `s.byte_at(i)` for a byte, or `for c in s` / `s.chars()` for characters");
                }
                self.err(d);
                self.error_expr(span)
            }
            TyKind::Error => self.error_expr(span),
            TyKind::Infer(_) => {
                self.need_type(b.span);
                self.error_expr(span)
            }
            _ => {
                self.err(Diagnostic::error("E0317", format!("cannot index into a value of type `{}`", self.ty_str(bt))).primary(span, "not indexable"));
                self.error_expr(span)
            }
        }
    }

    fn check_try(&mut self, inner: &ast::Expr, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let x = self.check_expr(inner, Expect::None);
        let xt = self.shallow(x.ty);
        let ret = self.resolve(self.ret_ty);
        let (lang_opt, lang_res) = (self.p.lang.option, self.p.lang.result);
        let (xid, xargs) = match tcx.as_adt(xt) {
            Some(v) => v,
            None => {
                if !tcx.is_error(xt) {
                    if matches!(&*tcx.kind(xt), TyKind::Infer(_)) {
                        self.need_type(x.span);
                    } else {
                        self.err(
                            Diagnostic::error("E0360", format!("the `?` operator applies to `Option` and `Result`, not `{}`", self.ty_str(xt)))
                                .primary(span, "cannot use `?` here"),
                        );
                    }
                }
                return self.error_expr(span);
            }
        };
        let kind = if Some(xid) == lang_opt {
            TryKind::Option
        } else if Some(xid) == lang_res {
            TryKind::Result
        } else {
            self.err(
                Diagnostic::error("E0360", format!("the `?` operator applies to `Option` and `Result`, not `{}`", self.ty_str(xt)))
                    .primary(span, "cannot use `?` here"),
            );
            return self.error_expr(span);
        };
        let ret_adt = tcx.as_adt(ret);
        let ok_ret = match (&ret_adt, kind) {
            (Some((r, _)), TryKind::Option) => Some(*r) == lang_opt,
            (Some((r, _)), TryKind::Result) => Some(*r) == lang_res,
            _ => false,
        };
        if !ok_ret {
            if !tcx.is_error(ret) {
                let want = if kind == TryKind::Option { "Option[...]" } else { "Result[..., E]" };
                self.err(
                    Diagnostic::error("E0361", format!("`?` can only be used in a function that returns `{}`", want))
                        .primary(span, "cannot propagate from here")
                        .note(format!("this function returns `{}`", self.ty_str(ret)))
                        .help("handle the value with `match` or `if let` instead"),
                );
            }
            return self.mk(T::Try(Box::new(x), kind), xargs[0], span);
        }
        if kind == TryKind::Result {
            let ret_err = ret_adt.unwrap().1[1];
            if !self.unify(xargs[1], ret_err) {
                self.err(
                    Diagnostic::error("E0360", "the error type of `?` does not match the function's error type")
                        .primary(span, format!("error type `{}`", self.ty_str(xargs[1])))
                        .note(format!("the function returns errors of type `{}`", self.ty_str(ret_err)))
                        .note("NEXUL 0.1 does not convert error types automatically"),
                );
            }
        }
        self.mk(T::Try(Box::new(x), kind), xargs[0], span)
    }

    // -----------------------------------------------------------------------
    // Blocks and control flow

    /// The type of a branch for joining: `Never` branches adopt the other type.
    fn join(&mut self, acc: Option<Ty>, branch: &thir::Expr, first_span: Span) -> Option<Ty> {
        let tcx = self.tcx();
        let bt = self.shallow(branch.ty);
        match acc {
            None => Some(bt),
            Some(a) => {
                let a = self.shallow(a);
                if tcx.is_never(bt) {
                    Some(a)
                } else if tcx.is_never(a) {
                    Some(bt)
                } else {
                    if !self.unify(a, bt) {
                        self.err(
                            Diagnostic::error("E0302", "branches have different types")
                                .primary(branch.span, format!("this branch is `{}`", self.ty_str(bt)))
                                .secondary(first_span, format!("the first branch is `{}`", self.ty_str(a))),
                        );
                    }
                    Some(a)
                }
            }
        }
    }

    pub(crate) fn check_block(&mut self, b: &ast::Block, expect: Expect) -> thir::Block {
        let tcx = self.tcx();
        self.push_scope();
        let mut stmts = Vec::new();
        let mut tail = None;
        let mut diverges = false;
        let mut warned = false;
        let n = b.stmts.len();
        for (i, s) in b.stmts.iter().enumerate() {
            if diverges && !warned {
                warned = true;
                self.err(Diagnostic::warning("W0006", "unreachable code").primary(s.span, "this code is never executed"));
            }
            let last = i + 1 == n;
            if last && expect.wants_value() {
                if let ast::StmtKind::Expr(e) = &s.kind {
                    let x = self.check_expr(e, expect);
                    if tcx.is_never(self.shallow(x.ty)) {
                        diverges = true;
                    }
                    tail = Some(Box::new(x));
                    continue;
                }
            }
            let (st, div) = self.check_stmt(s);
            diverges |= div;
            stmts.push(st);
        }
        self.pop_scope();
        let ty = if diverges {
            tcx.never
        } else if let Some(t) = &tail {
            t.ty
        } else {
            tcx.unit
        };
        thir::Block { stmts, tail, ty, span: b.span }
    }

    /// A branch body (`{...}` block or `else if`) used with `expect`.
    fn check_branch(&mut self, e: &ast::Expr, expect: Expect) -> thir::Expr {
        self.check_expr(e, expect)
    }

    fn check_if(&mut self, cond: &Cond, then: &ast::Block, else_: Option<&ast::Expr>, expect: Expect, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let branch_expect = match expect {
            Expect::Discard => Expect::Discard,
            other => other,
        };
        let (kind_builder, then_b): (Box<dyn FnOnce(thir::Block, Option<Box<thir::Expr>>) -> T>, thir::Block) = match cond {
            Cond::Expr(c) => {
                let c = self.check_expr_coerce(c, tcx.bool);
                let then_b = self.check_block(then, branch_expect);
                (Box::new(move |t, e| T::If { cond: Box::new(c), then: t, else_: e }), then_b)
            }
            Cond::Let(pat, scrutinee, mutable) => {
                let s = self.check_expr(scrutinee, Expect::None);
                let mode = self.scrutinee_mode(&s, *mutable);
                self.push_scope();
                let p = self.check_pat(pat, s.ty, mode);
                let then_b = self.check_block(then, branch_expect);
                self.pop_scope();
                let mutable = *mutable;
                (Box::new(move |t, e| T::IfLet { pat: p, scrutinee: Box::new(s), then: t, else_: e, mutable }), then_b)
            }
        };
        let else_x = else_.map(|e| self.check_branch(e, branch_expect));
        let ty = if !expect.wants_value() {
            match &else_x {
                Some(x) if tcx.is_never(then_b.ty) && tcx.is_never(self.shallow(x.ty)) => tcx.never,
                _ => tcx.unit,
            }
        } else {
            match &else_x {
                None => {
                    let then_ty = self.shallow(then_b.ty);
                    if !tcx.is_never(then_ty) && !self.unify(then_ty, tcx.unit) {
                        self.err(
                            Diagnostic::error("E0330", "`if` without `else` cannot produce a value")
                                .primary(span, format!("this `if` gives a value of type `{}` only when the condition is true", self.ty_str(then_ty)))
                                .help("add an `else` branch"),
                        );
                        tcx.error
                    } else if let Some(t) = expect.ty().filter(|&t| !tcx.is_unit_like(self.shallow(t)) && !matches!(&*tcx.kind(self.shallow(t)), TyKind::Infer(_))) {
                        self.err(
                            Diagnostic::error("E0330", "`if` without `else` cannot produce a value")
                                .primary(span, format!("a value of type `{}` is expected here", self.ty_str(t)))
                                .help("add an `else` branch"),
                        );
                        tcx.error
                    } else {
                        tcx.unit
                    }
                }
                Some(x) => {
                    let then_expr_span = then_b.tail.as_ref().map(|t| t.span).unwrap_or(then_b.span);
                    let first = thir::Expr { kind: T::Error, ty: then_b.ty, span: then_expr_span };
                    let acc = self.join(None, &first, then_expr_span);
                    let branch_span = match &x.kind {
                        T::Block(b) => b.tail.as_ref().map(|t| t.span).unwrap_or(x.span),
                        _ => x.span,
                    };
                    let probe = thir::Expr { kind: T::Error, ty: x.ty, span: branch_span };
                    self.join(acc, &probe, then_expr_span).unwrap()
                }
            }
        };
        let kind = kind_builder(then_b, else_x.map(Box::new));
        self.mk(kind, ty, span)
    }

    /// How the bindings of a pattern hold the matched value (spec §5.6).
    pub(crate) fn scrutinee_mode(&mut self, s: &thir::Expr, mutable: bool) -> super::pat::BindMode {
        if mutable {
            if !s.is_place() && !matches!(s.kind, T::Error) {
                self.err(
                    Diagnostic::error("E0410", "only a variable, field or element can be matched with `&`")
                        .primary(s.span, "temporary value")
                        .note("`&` gives the bindings write access to an existing place"),
                );
            }
            self.mark_mutated(s);
            super::pat::BindMode::RefMut
        } else if s.is_place() {
            super::pat::BindMode::Ref
        } else {
            super::pat::BindMode::Owned
        }
    }

    fn check_match(&mut self, scrutinee: &ast::Expr, arms: &[ast::Arm], mutable: bool, expect: Expect, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let s = self.check_expr(scrutinee, Expect::None);
        let mode = self.scrutinee_mode(&s, mutable);
        let mut out = Vec::new();
        let mut acc: Option<Ty> = None;
        let mut all_never = true;
        let mut first_span = span;
        for (i, arm) in arms.iter().enumerate() {
            self.push_scope();
            let p = self.check_pat(&arm.pat, s.ty, mode);
            let guard = arm.guard.as_ref().map(|g| self.check_expr_coerce(g, tcx.bool));
            let body = self.check_branch(&arm.body, expect);
            self.pop_scope();
            all_never &= tcx.is_never(self.shallow(body.ty));
            if expect.wants_value() {
                let body_span = match &body.kind {
                    T::Block(b) => b.tail.as_ref().map(|t| t.span).unwrap_or(body.span),
                    _ => body.span,
                };
                if i == 0 || acc.map_or(false, |a| tcx.is_never(self.shallow(a))) {
                    first_span = body_span;
                }
                let probe = thir::Expr { kind: T::Error, ty: body.ty, span: body_span };
                acc = self.join(acc, &probe, first_span);
            }
            out.push(thir::Arm { pat: p, guard, body, span: arm.span });
        }
        let ty = if !expect.wants_value() {
            if all_never {
                tcx.never
            } else {
                tcx.unit
            }
        } else {
            acc.unwrap_or(tcx.never)
        };
        self.mk(T::Match { scrutinee: Box::new(s), arms: out, mutable }, ty, span)
    }

    /// Field access on a value (`p.x`).
    pub(crate) fn check_field_access(&mut self, base: thir::Expr, name: &ast::Ident, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let bt = self.shallow(base.ty);
        match &*tcx.kind(bt) {
            TyKind::Adt(id, targs) => {
                let adt = self.p.adt(*id);
                if let AdtKind::Struct { fields } = &adt.kind {
                    if let Some(i) = fields.iter().position(|f| f.name == name.name) {
                        let f = &fields[i];
                        if !f.is_pub && adt.module != self.module {
                            self.err(
                                Diagnostic::error("E0226", format!("field `{}` of `{}` is private", name.name, adt.name))
                                    .primary(name.span, "private field")
                                    .secondary(f.span, "declared here without `pub`"),
                            );
                        }
                        let ty = tcx.subst(f.ty, targs);
                        return self.mk(T::Field(Box::new(base), i as u32), ty, span);
                    }
                }
                if adt.methods.contains_key(&name.name) {
                    self.err(
                        Diagnostic::error("E0309", format!("`{}` is a method of `{}`, not a field", name.name, adt.name))
                            .primary(name.span, "method, not a field")
                            .help(format!("add parentheses to call it: `.{}(...)`", name.name)),
                    );
                    return self.error_expr(span);
                }
                let mut d = Diagnostic::error("E0309", format!("`{}` has no field named `{}`", self.ty_str(bt), name.name)).primary(name.span, "unknown field");
                let names: Vec<&str> = adt.fields().iter().map(|f| f.name.as_str()).collect();
                if let Some(s) = best_match(&name.name, names.iter().copied()) {
                    d = d.help(format!("did you mean `{}`?", s));
                } else if !names.is_empty() {
                    d = d.note(format!("available fields: {}", names.join(", ")));
                }
                self.err(d);
                self.error_expr(span)
            }
            TyKind::Error => self.error_expr(span),
            TyKind::Infer(_) => {
                self.need_type(base.span);
                self.error_expr(span)
            }
            TyKind::Tuple(_) => {
                self.err(Diagnostic::error("E0309", "tuple fields are numbered").primary(name.span, "use `.0`, `.1`, ...").note(format!("the type is `{}`", self.ty_str(bt))));
                self.error_expr(span)
            }
            _ => {
                self.err(Diagnostic::error("E0309", format!("`{}` has no fields", self.ty_str(bt))).primary(name.span, "unknown field"));
                self.error_expr(span)
            }
        }
    }
}

impl crate::ty::TyCtx {
    pub fn is_unit_like(&self, t: Ty) -> bool {
        t == self.unit || t == self.error || t == self.never
    }
}

//! Statement checking.

use super::pat::BindMode;
use super::{Deferred, Expect, FnCtxt, LoopCtx};
use crate::defs::{AdtKind, ConstValue};
use crate::thir::{self, ExprKind as T, IterKind, LocalKind, StmtKind as S};
use crate::ty::TyKind;
use nxc_syntax::ast::{self, AssignOp, ExprKind as A, ForIter, PatKind, StmtKind};
use nxc_syntax::Diagnostic;

impl<'a> FnCtxt<'a> {
    /// Checks a statement; returns it and whether it always diverges.
    pub(crate) fn check_stmt(&mut self, s: &ast::Stmt) -> (thir::Stmt, bool) {
        let tcx = self.tcx();
        let span = s.span;
        let never = |cx: &Self, e: &thir::Expr| tcx.is_never(cx.shallow(e.ty));
        let (kind, div) = match &s.kind {
            StmtKind::Let { pat, ty, init } => {
                let declared = ty.as_ref().map(|t| self.resolve_type(t));
                let x = match declared {
                    Some(t) => self.check_expr_coerce(init, t),
                    None => self.check_expr(init, Expect::None),
                };
                let div = never(self, &x);
                let pty = declared.unwrap_or(x.ty);
                // Refutable patterns are reported by the exhaustiveness pass (E0343).
                let p = self.check_pat(pat, pty, BindMode::Owned);
                (S::Let { pat: p, init: x }, div)
            }
            StmtKind::Var { name, ty, init } => {
                let declared = ty.as_ref().map(|t| self.resolve_type(t));
                let x = match (declared, init) {
                    (Some(t), Some(e)) => Some(self.check_expr_coerce(e, t)),
                    (None, Some(e)) => Some(self.check_expr(e, Expect::None)),
                    (_, None) => None,
                };
                if declared.is_none() && init.is_none() {
                    self.err(
                        Diagnostic::error("E0321", format!("variable `{}` needs a type or an initial value", name.name))
                            .primary(name.span, "type unknown")
                            .help(format!("write `var {}: Type` or `var {} = value`", name.name, name.name)),
                    );
                }
                let ty = declared.or(x.as_ref().map(|e| e.ty)).unwrap_or(tcx.error);
                let div = x.as_ref().map_or(false, |e| never(self, e));
                let l = self.declare_local(&name.name, ty, LocalKind::Var, name.span);
                (S::Var { local: l, init: x }, div)
            }
            StmtKind::Assign { op, op_span, target, value } => {
                let was_read = match &target.kind {
                    A::Ident(n) => self.lookup_local(n).map(|l| self.uses[l as usize].read),
                    _ => None,
                };
                let place = self.check_expr(target, Expect::None);
                if let (AssignOp::Assign, Some(prev), thir::ExprKind::Local(l)) = (op, was_read, &place.kind) {
                    // A plain assignment does not read the variable.
                    self.uses[*l as usize].read = prev;
                }
                if !place.is_place() && !matches!(place.kind, T::Error) {
                    self.err(
                        Diagnostic::error("E0407", "cannot assign to this expression")
                            .primary(target.span, "not a variable, field or element")
                            .note("only places (variables, fields, elements) can be assigned"),
                    );
                }
                self.mark_mutated(&place);
                match op {
                    AssignOp::Assign => {
                        let v = self.check_expr_coerce(value, place.ty);
                        let div = never(self, &v);
                        (S::Assign { place, value: v }, div)
                    }
                    AssignOp::Compound(bop) => {
                        let v = if bop.is_shift() {
                            let v = self.check_expr(value, Expect::None);
                            self.deferred.push(Deferred::Integer { ty: v.ty, span: v.span, what: "a shift amount" });
                            self.deferred.push(Deferred::Integer { ty: place.ty, span: place.span, what: "the left operand of a shift" });
                            v
                        } else {
                            let v = self.check_expr_coerce(value, place.ty);
                            self.deferred.push(Deferred::Binary { op: *bop, lhs: place.ty, rhs: v.ty, span: *op_span });
                            v
                        };
                        let div = never(self, &v);
                        (S::CompoundAssign { op: *bop, place, value: v }, div)
                    }
                }
            }
            StmtKind::Expr(e) => {
                let x = self.check_expr(e, Expect::Discard);
                let div = never(self, &x);
                if !div && !matches!(x.kind, T::Error) {
                    self.deferred.push(Deferred::UnusedResult { ty: x.ty, span: x.span });
                }
                (S::Expr(x), div)
            }
            StmtKind::While { label, cond, body } => {
                let c = self.check_expr_coerce(cond, tcx.bool);
                let infinite = matches!(c.kind, T::Lit(ConstValue::Bool(true)));
                let id = self.enter_loop(label);
                let b = self.check_block(body, Expect::Discard);
                let lc = self.loops.pop().unwrap();
                let div = (infinite && !lc.has_break) || never(self, &c);
                (S::While { id, cond: c, body: b }, div)
            }
            StmtKind::For { label, pat, iter, body } => match iter {
                ForIter::Range { start, end, inclusive } => {
                    let a = self.check_expr(start, Expect::None);
                    let b = self.check_expr(end, Expect::Has(a.ty));
                    if !self.unify(a.ty, b.ty) {
                        self.err(
                            Diagnostic::error("E0302", "the bounds of a range must have the same type")
                                .primary(b.span, format!("`{}`", self.ty_str(b.ty)))
                                .secondary(a.span, format!("`{}`", self.ty_str(a.ty))),
                        );
                    }
                    self.deferred.push(Deferred::Integer { ty: a.ty, span: a.span.to(b.span), what: "a range bound" });
                    self.push_scope();
                    let var = match &pat.kind {
                        PatKind::Ident(n) => Some(self.declare_local(n, a.ty, LocalKind::Let, pat.span)),
                        PatKind::Wild => None,
                        _ => {
                            self.err(Diagnostic::error("E0347", "a range loop binds a single variable").primary(pat.span, "expected a name or `_`"));
                            None
                        }
                    };
                    let id = self.enter_loop(label);
                    let body_b = self.check_block(body, Expect::Discard);
                    self.loops.pop();
                    self.pop_scope();
                    (S::ForRange { id, var, start: a, end: b, inclusive: *inclusive, body: body_b }, false)
                }
                ForIter::Expr { expr: e, mutable } => {
                    let mutable = *mutable;
                    let it = self.check_expr(e, Expect::None);
                    let it_ty = self.shallow(it.ty);
                    let (kind, elem_ty) = match &*tcx.kind(it_ty) {
                        TyKind::Array(elem, n) => (Some(IterKind::Array(*n)), *elem),
                        TyKind::Adt(id, targs) if self.p.is_string(it_ty) => {
                            let _ = (id, targs);
                            (Some(IterKind::Str), tcx.char)
                        }
                        TyKind::Adt(id, targs) => {
                            let adt = self.p.adt(*id);
                            let len_field = match &adt.kind {
                                AdtKind::Struct { fields } => fields.iter().position(|f| f.name == "len"),
                                _ => None,
                            };
                            match (adt.index_fn, len_field) {
                                (Some(f), Some(lf)) => {
                                    let ret = tcx.subst(self.p.func(f).ret, targs);
                                    let elem = match &*tcx.kind(ret) {
                                        TyKind::Ptr(e) => *e,
                                        _ => tcx.error,
                                    };
                                    (Some(IterKind::Indexed { len_field: lf as u32, index_fn: f, type_args: targs.clone() }), elem)
                                }
                                _ => (None, tcx.error),
                            }
                        }
                        TyKind::Error => (None, tcx.error),
                        TyKind::Infer(_) => {
                            self.need_type(it.span);
                            (None, tcx.error)
                        }
                        _ => (None, tcx.error),
                    };
                    if kind.is_none() && !tcx.is_error(it_ty) && !matches!(&*tcx.kind(it_ty), TyKind::Infer(_)) {
                        let mut d = Diagnostic::error("E0318", format!("cannot iterate over a value of type `{}`", self.ty_str(it_ty))).primary(it.span, "not iterable");
                        if tcx.as_int(it_ty).is_some() {
                            d = d.help("to count, use a range: `for i in 0..n`");
                        }
                        self.err(d);
                    }
                    let str_mut = mutable && matches!(kind, Some(IterKind::Str));
                    if str_mut {
                        self.err(
                            Diagnostic::error("E0318", "the characters of a `String` cannot be modified in place")
                                .primary(it.span, "iterated with `&`")
                                .help("build a new string, for example with `to_uppercase()` or a loop pushing characters"),
                        );
                    }
                    let mode = if mutable && !str_mut { self.scrutinee_mode(&it, true) } else { BindMode::Ref };
                    self.push_scope();
                    let p = self.check_pat(pat, elem_ty, mode);
                    let id = self.enter_loop(label);
                    let body_b = self.check_block(body, Expect::Discard);
                    self.loops.pop();
                    self.pop_scope();
                    match kind {
                        Some(kind) => (S::ForEach { id, pat: p, iter: it, kind, elem_ty, body: body_b, mutable }, false),
                        None => (S::Expr(self.error_expr(span)), false),
                    }
                }
            },
        };
        (thir::Stmt { kind, span }, div)
    }

    fn enter_loop(&mut self, label: &Option<ast::Ident>) -> u32 {
        let id = self.next_loop;
        self.next_loop += 1;
        if let Some(l) = label {
            if self.loops.iter().any(|lc| lc.label.as_deref() == Some(l.name.as_str())) {
                self.err(Diagnostic::error("E0207", format!("label `{}` is already used by an enclosing loop", l.name)).primary(l.span, "duplicate label"));
            }
        }
        self.loops.push(LoopCtx { id, label: label.as_ref().map(|l| l.name.clone()), has_break: false });
        id
    }
}

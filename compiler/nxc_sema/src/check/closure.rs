//! Anonymous functions (spec §4.12): `fn(x) { ... }` and `fn(x) => expr`, written as an
//! argument of a call. They use the variables around them in place (through a pointer
//! to each), which is safe because they never outlive the call that receives them.

use super::{ClosureFrame, Expect, FnCtxt, PendingClosure};
use crate::defs::FnId;
use crate::thir::{self, ExprKind as T, LocalId, LocalKind, StmtKind as S};
use crate::ty::{Ty, TyKind};
use nxc_syntax::ast::{self, ClosureBody, Convention};
use nxc_syntax::{Diagnostic, Span};

impl<'a> FnCtxt<'a> {
    pub(crate) fn check_closure(&mut self, c: &ast::ClosureExpr, expected: Ty, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let exp = self.shallow(expected);
        let (hint_params, hint_ret) = match &*tcx.kind(exp) {
            TyKind::Closure(ps, r) | TyKind::Fn(ps, r) => (Some(ps.clone()), Some(*r)),
            _ => (None, None),
        };
        if let Some(hp) = &hint_params {
            if hp.len() != c.params.len() {
                self.err(
                    Diagnostic::error("E0304", format!("this anonymous function takes {} parameter(s), but {} are expected", c.params.len(), hp.len()))
                        .primary(span, format!("expected `{}`", self.ty_str(exp))),
                );
            }
        }
        let mut sig: Vec<(Convention, Ty)> = Vec::new();
        for (i, cp) in c.params.iter().enumerate() {
            let hint = hint_params.as_ref().and_then(|h| h.get(i)).copied();
            let conv = match (cp.conv, hint) {
                (Convention::Borrow, Some((hc, _))) => hc,
                (c, Some((hc, _))) if c != hc => {
                    self.err(
                        Diagnostic::error("E0302", format!("parameter `{}` must be declared `{}`", cp.name.name, if hc == Convention::Borrow { "without convention".to_string() } else { hc.keyword().to_string() }))
                            .primary(cp.name.span, "convention differs from the expected function type"),
                    );
                    hc
                }
                (c, _) => c,
            };
            let ty = match (&cp.ty, hint) {
                (Some(t), h) => {
                    let ty = self.resolve_type(t);
                    if let Some((_, ht)) = h {
                        if !self.unify(ty, ht) {
                            self.mismatch(t.span, ht, ty);
                        }
                    }
                    ty
                }
                (None, Some((_, ht))) => ht,
                (None, None) => self.new_var(cp.name.span),
            };
            sig.push((conv, ty));
        }
        let ret = match (&c.ret, hint_ret) {
            (Some(t), h) => {
                let r = self.resolve_type(t);
                if let Some(hr) = h {
                    if !self.unify(r, hr) {
                        self.mismatch(t.span, hr, r);
                    }
                }
                r
            }
            (None, Some(hr)) => hr,
            (None, None) => self.new_var(span),
        };
        // The body is checked in this function's context: its own `return` target, no
        // enclosing loops, and a frame recording the captured variables.
        let saved_ret = std::mem::replace(&mut self.ret_ty, ret);
        let saved_loops = std::mem::take(&mut self.loops);
        let scope = std::mem::take(&mut self.next_closure_scope);
        self.closure_frames.push(ClosureFrame { first_local: self.locals.len() as LocalId, captures: Vec::new(), scope_param: None });
        self.push_scope();
        let mut params = Vec::new();
        for (cp, &(conv, ty)) in c.params.iter().zip(sig.iter()) {
            let l = self.declare_local(&cp.name.name, ty, LocalKind::Param(conv), cp.name.span);
            params.push((l, conv));
        }
        if scope {
            self.closure_frames.last_mut().unwrap().scope_param = params.first().map(|p| p.0);
        }
        let block = match &c.body {
            ClosureBody::Block(b) => {
                let blk = self.check_block(b, Expect::Discard);
                if !tcx.is_never(blk.ty) {
                    let r = self.shallow(ret);
                    if matches!(&*tcx.kind(r), TyKind::Infer(_)) || r == tcx.unit {
                        self.unify(r, tcx.unit);
                    } else if !tcx.is_error(r) {
                        let end = Span::new(b.span.file, b.span.hi.saturating_sub(1), b.span.hi);
                        self.err(
                            Diagnostic::error("E0331", "this anonymous function may finish without returning a value")
                                .primary(end, format!("execution can reach the end, but `{}` is expected", self.ty_str(r)))
                                .help("end every path with `return value`, or write `fn(x) => expression`"),
                        );
                    }
                }
                blk
            }
            ClosureBody::Expr(e) => {
                let mut x = self.check_expr(e, Expect::Has(ret));
                self.coerce(&mut x, ret);
                let sp = x.span;
                let r = self.mk(T::Return(Some(Box::new(x))), tcx.never, sp);
                thir::Block { stmts: vec![thir::Stmt { kind: S::Expr(r), span: sp }], tail: None, ty: tcx.never, span: sp }
            }
        };
        self.pop_scope();
        let frame = self.closure_frames.pop().unwrap();
        self.loops = saved_loops;
        self.ret_ty = saved_ret;
        let f = FnId(self.next_fn_id);
        self.next_fn_id += 1;
        self.closures.push(PendingClosure { fn_id: f, params, captures: frame.captures.clone(), block, ret_ty: ret, span });
        let ty = tcx.mk(TyKind::Closure(sig, ret));
        self.mk(T::Closure { f, captures: frame.captures }, ty, span)
    }

    /// An anonymous function anywhere else than directly as an argument.
    pub(crate) fn misplaced_closure(&mut self, span: Span) -> thir::Expr {
        self.err(
            Diagnostic::error("E0248", "an anonymous function can only be written as an argument of a call")
                .primary(span, "not passed directly to a function")
                .note("an anonymous function uses the variables around it: it cannot outlive the call that receives it")
                .help("pass it directly, for example `v.map(fn(x) => x * 2)`, or declare a named function"),
        );
        self.error_expr(span)
    }
}

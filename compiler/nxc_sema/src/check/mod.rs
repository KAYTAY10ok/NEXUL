//! The type checker: turns function bodies (AST) into THIR, inferring types by
//! unification (spec §3.5).

mod call;
mod expr;
mod finalize;
mod pat;
mod stmt;

use crate::collect::TypeScope;
use crate::defs::{FnId, FnKind, ModId, Program};
use crate::thir::{self, LocalDecl, LocalId, LocalKind};
use crate::ty::{Ty, TyKind};
use nxc_syntax::ast::{self, BinOp, UnOp};
use nxc_syntax::token::FormatSpec;
use nxc_syntax::{DiagSink, Diagnostic, Span};

/// What the context expects from an expression.
#[derive(Clone, Copy, Debug)]
pub enum Expect {
    /// Any value; no type hint.
    None,
    /// A value, ideally of this type (the caller performs the final coercion).
    Has(Ty),
    /// The value is discarded (expression statement): `if`/`match` need no common type.
    Discard,
}

impl Expect {
    pub fn ty(self) -> Option<Ty> {
        match self {
            Expect::Has(t) => Some(t),
            _ => None,
        }
    }
    pub fn wants_value(self) -> bool {
        !matches!(self, Expect::Discard)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum VarKind {
    General,
    Int,
    Float,
}

#[derive(Clone, Debug)]
pub(crate) struct InferVar {
    kind: VarKind,
    binding: Option<Ty>,
    span: Span,
}

/// Checks that are only possible once every type is known.
#[derive(Clone, Debug)]
pub(crate) enum Deferred {
    Format { ty: Ty, spec: Option<FormatSpec>, span: Span },
    Binary { op: BinOp, lhs: Ty, rhs: Ty, span: Span },
    Unary { op: UnOp, ty: Ty, span: Span },
    Cast { from: Ty, to: Ty, span: Span },
    Integer { ty: Ty, span: Span, what: &'static str },
    SignedInt { ty: Ty, span: Span },
    UnusedResult { ty: Ty, span: Span },
    Copyable { ty: Ty, span: Span, what: &'static str },
}

pub(crate) struct LoopCtx {
    pub id: u32,
    pub label: Option<String>,
    pub has_break: bool,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct LocalUse {
    pub read: bool,
    pub mutated: bool,
}

pub struct FnCtxt<'a> {
    pub(crate) p: &'a Program,
    pub(crate) diags: &'a DiagSink,
    pub(crate) module: ModId,
    pub(crate) generics: Vec<String>,
    pub(crate) self_ty: Option<Ty>,
    pub(crate) ret_ty: Ty,
    pub(crate) unsafe_depth: u32,
    pub(crate) locals: Vec<LocalDecl>,
    pub(crate) uses: Vec<LocalUse>,
    pub(crate) scopes: Vec<Vec<(String, LocalId)>>,
    pub(crate) loops: Vec<LoopCtx>,
    pub(crate) next_loop: u32,
    pub(crate) vars: Vec<InferVar>,
    pub(crate) deferred: Vec<Deferred>,
}

impl<'a> FnCtxt<'a> {
    pub fn new(p: &'a Program, diags: &'a DiagSink, module: ModId) -> FnCtxt<'a> {
        FnCtxt {
            p,
            diags,
            module,
            generics: Vec::new(),
            self_ty: None,
            ret_ty: p.tcx.unit,
            unsafe_depth: 0,
            locals: Vec::new(),
            uses: Vec::new(),
            scopes: vec![Vec::new()],
            loops: Vec::new(),
            next_loop: 0,
            vars: Vec::new(),
            deferred: Vec::new(),
        }
    }

    /// The type context; the returned reference does not borrow `self`.
    pub(crate) fn tcx(&self) -> &'a crate::ty::TyCtx {
        &self.p.tcx
    }

    pub(crate) fn err(&self, d: Diagnostic) {
        self.diags.emit(d);
    }

    // -----------------------------------------------------------------------
    // Inference variables

    pub(crate) fn new_var_of(&mut self, kind: VarKind, span: Span) -> Ty {
        let n = self.vars.len() as u32;
        self.vars.push(InferVar { kind, binding: None, span });
        self.tcx().mk(match kind {
            VarKind::General => TyKind::Infer(n),
            VarKind::Int => TyKind::IntVar(n),
            VarKind::Float => TyKind::FloatVar(n),
        })
    }

    pub(crate) fn new_var(&mut self, span: Span) -> Ty {
        self.new_var_of(VarKind::General, span)
    }

    pub(crate) fn fresh_args(&mut self, n: usize, span: Span) -> Vec<Ty> {
        (0..n).map(|_| self.new_var(span)).collect()
    }

    fn var_index(&self, t: Ty) -> Option<u32> {
        match *self.tcx().kind(t) {
            TyKind::Infer(n) | TyKind::IntVar(n) | TyKind::FloatVar(n) => Some(n),
            _ => None,
        }
    }

    /// Follows variable bindings at the top level only.
    pub(crate) fn shallow(&self, mut t: Ty) -> Ty {
        while let Some(n) = self.var_index(t) {
            match self.vars[n as usize].binding {
                Some(b) => t = b,
                None => break,
            }
        }
        t
    }

    /// Substitutes every bound variable, recursively.
    pub(crate) fn resolve(&self, t: Ty) -> Ty {
        let t = self.shallow(t);
        if !self.tcx().has_infer(t) {
            return t;
        }
        self.tcx().map(t, &mut |k| match k {
            TyKind::Infer(n) | TyKind::IntVar(n) | TyKind::FloatVar(n) => {
                let b = self.vars[*n as usize].binding?;
                Some(self.resolve(b))
            }
            _ => None,
        })
    }

    fn occurs(&self, n: u32, t: Ty) -> bool {
        let t = self.shallow(t);
        match &*self.tcx().kind(t) {
            TyKind::Infer(m) | TyKind::IntVar(m) | TyKind::FloatVar(m) => *m == n,
            TyKind::Adt(_, a) | TyKind::Tuple(a) => a.iter().any(|&x| self.occurs(n, x)),
            TyKind::Array(e, _) | TyKind::Ptr(e) => self.occurs(n, *e),
            TyKind::Fn(ps, r) => ps.iter().any(|&(_, x)| self.occurs(n, x)) || self.occurs(n, *r),
            _ => false,
        }
    }

    /// Unifies two types; on failure the inference state is left unchanged.
    pub(crate) fn unify(&mut self, a: Ty, b: Ty) -> bool {
        let snapshot: Vec<Option<Ty>> = self.vars.iter().map(|v| v.binding).collect();
        if self.unify_inner(a, b) {
            true
        } else {
            for (v, s) in self.vars.iter_mut().zip(snapshot) {
                v.binding = s;
            }
            false
        }
    }

    fn bind(&mut self, n: u32, t: Ty) {
        self.vars[n as usize].binding = Some(t);
    }

    fn unify_inner(&mut self, a: Ty, b: Ty) -> bool {
        let a = self.shallow(a);
        let b = self.shallow(b);
        if a == b {
            return true;
        }
        let tcx = self.tcx();
        let ka = tcx.kind(a);
        let kb = tcx.kind(b);
        use TyKind::*;
        match (&*ka, &*kb) {
            // An erroneous type absorbs variables, so that one error causes no other.
            (Infer(n) | IntVar(n) | FloatVar(n), Error) | (Error, Infer(n) | IntVar(n) | FloatVar(n)) => {
                self.bind(*n, tcx.error);
                true
            }
            (Error, _) => {
                self.absorb_error(b);
                true
            }
            (_, Error) => {
                self.absorb_error(a);
                true
            }
            (Infer(n), _) => {
                if self.occurs(*n, b) {
                    return false;
                }
                self.bind(*n, b);
                true
            }
            (_, Infer(n)) => {
                if self.occurs(*n, a) {
                    return false;
                }
                self.bind(*n, a);
                true
            }
            (IntVar(n), Int(_)) | (IntVar(n), IntVar(_)) => {
                self.bind(*n, b);
                true
            }
            (Int(_), IntVar(n)) => {
                self.bind(*n, a);
                true
            }
            (FloatVar(n), Float(_)) | (FloatVar(n), FloatVar(_)) => {
                self.bind(*n, b);
                true
            }
            (Float(_), FloatVar(n)) => {
                self.bind(*n, a);
                true
            }
            (Adt(x, xa), Adt(y, ya)) => x == y && xa.len() == ya.len() && xa.clone().iter().zip(ya.clone().iter()).all(|(&p, &q)| self.unify_inner(p, q)),
            (Tuple(xa), Tuple(ya)) => xa.len() == ya.len() && xa.clone().iter().zip(ya.clone().iter()).all(|(&p, &q)| self.unify_inner(p, q)),
            (Array(x, n), Array(y, m)) => n == m && self.unify_inner(*x, *y),
            (Ptr(x), Ptr(y)) => self.unify_inner(*x, *y),
            (Fn(xp, xr), Fn(yp, yr)) => {
                xp.len() == yp.len()
                    && xp.iter().zip(yp.iter()).all(|(p, q)| p.0 == q.0)
                    && xp.clone().iter().zip(yp.clone().iter()).all(|(p, q)| self.unify_inner(p.1, q.1))
                    && self.unify_inner(*xr, *yr)
            }
            _ => false,
        }
    }

    /// Binds every unbound variable inside `t` to the error type.
    fn absorb_error(&mut self, t: Ty) {
        let t = self.shallow(t);
        let tcx = self.tcx();
        match &*tcx.kind(t) {
            TyKind::Infer(n) | TyKind::IntVar(n) | TyKind::FloatVar(n) => self.bind(*n, tcx.error),
            TyKind::Adt(_, a) | TyKind::Tuple(a) => {
                for &x in a.iter() {
                    self.absorb_error(x);
                }
            }
            TyKind::Array(e, _) | TyKind::Ptr(e) => self.absorb_error(*e),
            TyKind::Fn(ps, r) => {
                for &(_, x) in ps.iter() {
                    self.absorb_error(x);
                }
                self.absorb_error(*r);
            }
            _ => {}
        }
    }

    pub(crate) fn ty_str(&self, t: Ty) -> String {
        self.p.ty_str_with(self.resolve(t), &self.generics)
    }

    /// Final coercion of an expression to an expected type (spec §3.6): only `Never`
    /// converts implicitly.
    pub(crate) fn coerce(&mut self, e: &thir::Expr, expected: Ty) {
        if self.tcx().is_never(self.shallow(e.ty)) {
            return;
        }
        if !self.unify(e.ty, expected) {
            self.mismatch(e.span, expected, e.ty);
        }
    }

    pub(crate) fn mismatch(&self, span: Span, expected: Ty, found: Ty) {
        let (es, fs) = (self.ty_str(expected), self.ty_str(found));
        let mut d = Diagnostic::error("E0302", "mismatched types").primary(span, format!("expected `{}`, found `{}`", es, fs));
        let tcx = self.tcx();
        let (re, rf) = (self.resolve(expected), self.resolve(found));
        let numeric = |t: Ty| matches!(&*tcx.kind(t), TyKind::Int(_) | TyKind::Float(_) | TyKind::IntVar(_) | TyKind::FloatVar(_));
        if numeric(re) && numeric(rf) {
            if !matches!(&*tcx.kind(re), TyKind::IntVar(_) | TyKind::FloatVar(_)) {
                d = d.help(format!("NEXUL never converts numbers implicitly; write `... as {}`", es));
            }
        } else if self.p.is_string(re) && matches!(&*tcx.kind(rf), TyKind::Char) {
            d = d.help("a `char` is not a `String`; use `c.to_string()` or double quotes");
        } else if let (Some((oa, oargs)), true) = (tcx.as_adt(re), true) {
            if Some(oa) == self.p.lang.option && oargs.len() == 1 && self.unify_probe(oargs[0], rf) {
                d = d.help("wrap the value in `Some(...)`");
            }
        }
        self.err(d);
    }

    /// Would `a` and `b` unify? (no side effects)
    pub(crate) fn unify_probe(&self, a: Ty, b: Ty) -> bool {
        let mut copy = FnCtxt {
            p: self.p,
            diags: self.diags,
            module: self.module,
            generics: Vec::new(),
            self_ty: None,
            ret_ty: self.ret_ty,
            unsafe_depth: 0,
            locals: Vec::new(),
            uses: Vec::new(),
            scopes: Vec::new(),
            loops: Vec::new(),
            next_loop: 0,
            vars: self.vars.clone(),
            deferred: Vec::new(),
        };
        copy.unify(a, b)
    }

    // -----------------------------------------------------------------------
    // Scopes and locals

    pub(crate) fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    pub(crate) fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub(crate) fn declare_local(&mut self, name: &str, ty: Ty, kind: LocalKind, span: Span) -> LocalId {
        let id = self.locals.len() as LocalId;
        self.locals.push(LocalDecl { name: name.to_string(), ty, kind, span });
        self.uses.push(LocalUse::default());
        if name != "_" {
            let scope = self.scopes.last_mut().unwrap();
            if let Some(&(_, prev)) = scope.iter().find(|(n, _)| n == name) {
                let prev_span = self.locals[prev as usize].span;
                self.diags.emit(
                    Diagnostic::error("E0201", format!("`{}` is already declared in this scope", name))
                        .primary(span, "redeclared here")
                        .secondary(prev_span, "first declaration")
                        .help("choose another name, or assign to the existing variable"),
                );
            }
            self.scopes.last_mut().unwrap().push((name.to_string(), id));
        }
        id
    }

    pub(crate) fn lookup_local(&self, name: &str) -> Option<LocalId> {
        for scope in self.scopes.iter().rev() {
            if let Some(&(_, id)) = scope.iter().rev().find(|(n, _)| n == name) {
                return Some(id);
            }
        }
        None
    }

    pub(crate) fn mark_read(&mut self, l: LocalId) {
        self.uses[l as usize].read = true;
    }

    /// Records that the place rooted at `e` is modified.
    pub(crate) fn mark_mutated(&mut self, e: &thir::Expr) {
        if let Some(r) = e.place_root() {
            self.uses[r as usize].mutated = true;
        }
    }

    pub(crate) fn type_scope(&self) -> TypeScope<'_> {
        TypeScope { module: self.module, generics: &self.generics, self_ty: self.self_ty }
    }

    pub(crate) fn resolve_type(&self, t: &ast::TypeExpr) -> Ty {
        crate::collect::resolve_type(self.p, &self.type_scope(), t, self.diags)
    }

    pub(crate) fn in_unsafe(&self) -> bool {
        self.unsafe_depth > 0
    }

    pub(crate) fn require_unsafe(&self, span: Span, what: &str) {
        if !self.in_unsafe() {
            self.err(
                Diagnostic::error("E0501", format!("{} requires an `unsafe` block", what))
                    .primary(span, "unsafe operation")
                    .note("the compiler cannot check this operation for memory safety")
                    .help("wrap it in `unsafe { ... }` after checking that it is correct"),
            );
        }
    }
}

/// Type-checks every function body and constant of the program.
pub fn check_program(prog: &mut Program, diags: &DiagSink) {
    let mut bodies = Vec::new();
    for i in 0..prog.fns.len() {
        let id = FnId(i as u32);
        let f = &prog.fns[i];
        let FnKind::User(decl) = &f.kind else { continue };
        let Some(block) = &decl.body else { continue };
        let decl = decl.clone();
        let body = check_fn(prog, id, &decl, block, diags);
        bodies.push((id, body));
    }
    for (id, b) in bodies {
        prog.bodies.insert(id, b);
    }
}

fn check_fn(prog: &Program, id: FnId, decl: &ast::FnDecl, block: &ast::Block, diags: &DiagSink) -> thir::Body {
    let f = prog.func(id);
    let mut cx = FnCtxt::new(prog, diags, f.module);
    cx.generics = f.generics.clone();
    cx.self_ty = f.owner.map(|a| prog.tcx.adt(a, (0..f.n_owner_generics as u32).map(|i| prog.tcx.param(i)).collect()));
    cx.ret_ty = f.ret;
    if f.is_unsafe {
        cx.unsafe_depth = 1;
    }
    let mut params = Vec::new();
    for p in &f.params {
        // Duplicate parameter names were reported with the signature: declare quietly.
        let id = cx.locals.len() as LocalId;
        cx.locals.push(LocalDecl { name: p.name.clone(), ty: p.ty, kind: LocalKind::Param(p.conv), span: p.span });
        cx.uses.push(LocalUse::default());
        cx.scopes.last_mut().unwrap().push((p.name.clone(), id));
        params.push(id);
    }
    // Parameters live in their own scope: the body may not redeclare them.
    let body_block = cx.check_block(block, Expect::Discard);
    let ret_ty = f.ret;
    if ret_ty != prog.tcx.unit && !prog.tcx.is_error(ret_ty) && !prog.tcx.is_never(body_block.ty) {
        let end = Span::new(block.span.file, block.span.hi.saturating_sub(1), block.span.hi);
        cx.err(
            Diagnostic::error("E0331", format!("function `{}` may finish without returning a value", decl.name.name))
                .primary(end, format!("execution can reach the end of the function, which returns `{}`", cx.ty_str(ret_ty)))
                .secondary(decl.ret.as_ref().map(|r| r.span).unwrap_or(decl.name.span), "return type declared here")
                .note("NEXUL has no implicit return: end every path with `return value`"),
        );
    }
    if ret_ty == prog.tcx.never && !prog.tcx.is_never(body_block.ty) {
        let end = Span::new(block.span.file, block.span.hi.saturating_sub(1), block.span.hi);
        cx.err(Diagnostic::error("E0331", format!("function `{}` returns `Never` but can finish normally", decl.name.name)).primary(end, "reachable end of function"));
    }
    let mut body = thir::Body { fn_id: id, locals: Vec::new(), params, block: body_block, ret_ty, span: decl.span };
    cx.finish(&mut body);
    body
}

//! THIR -> NIR lowering.
//!
//! Evaluation order follows spec §4.1: left to right; the places passed by reference
//! (and assignment targets) are resolved after every value has been computed.

use crate::nir::*;
use nxc_sema::defs::{AdtKind, Builtin, ConstValue, FnId, Program};
use nxc_sema::thir::{self, Callee as TCallee, Expr, ExprKind as T, InterpPart, IterKind, LocalKind as TLocalKind, Pat, PatKind, StmtKind as S, TryKind};
use nxc_sema::ty::{Ty, TyKind};
use nxc_syntax::ast::{BinOp, Convention, UnOp};
use nxc_syntax::{SourceMap, Span};

/// Is a parameter of declared type `ty` passed by reference? (spec §5.3: the choice is
/// an implementation detail; it depends only on the callee's declaration.)
pub fn param_by_ref(p: &Program, conv: Convention, ty: Ty) -> bool {
    match conv {
        Convention::Inout => true,
        Convention::Owned => false,
        Convention::Borrow => !p.is_copy(ty),
    }
}

#[derive(Clone, Copy, Debug)]
struct Mapped {
    local: Local,
    by_ref: bool,
}

struct LoopInfo {
    id: thir::LoopId,
    break_bb: BlockId,
    continue_bb: BlockId,
    depth: usize,
}

enum Step {
    Field(u32),
    ArrayIndex { index: Operand, len: u64, span: Span },
    IndexFn { f: FnId, targs: Vec<Ty>, index: Operand, index_ty: Ty, elem: Ty, span: Span },
}

struct PrePlace {
    base: Place,
    steps: Vec<Step>,
}

pub struct Builder<'a> {
    p: &'a Program,
    sm: &'a SourceMap,
    thir: &'a thir::Body,
    locals: Vec<LocalDecl>,
    blocks: Vec<BasicBlock>,
    terminated: Vec<bool>,
    cur: BlockId,
    map: Vec<Option<Mapped>>,
    scopes: Vec<Vec<Local>>,
    loops: Vec<LoopInfo>,
    /// The trait bounds of the function: a parameter `T: Copy` is copyable.
    bounds: Vec<nxc_sema::defs::Bound>,
}

pub fn build(p: &Program, sm: &SourceMap, f: FnId, thir: &thir::Body) -> Body {
    let fd = p.func(f);
    let mut b = Builder {
        p,
        sm,
        thir,
        locals: Vec::new(),
        blocks: Vec::new(),
        terminated: Vec::new(),
        cur: 0,
        map: vec![None; thir.locals.len()],
        scopes: vec![Vec::new()],
        loops: Vec::new(),
        bounds: fd.bounds.clone(),
    };
    b.locals.push(LocalDecl { ty: thir.ret_ty, name: None, kind: LocalKind::Return, span: thir.span });
    let entry = b.new_block();
    b.cur = entry;
    for (i, &tl) in thir.params.iter().enumerate() {
        let decl = &thir.locals[tl as usize];
        let param = &fd.params[i];
        let by_ref = param_by_ref(p, param.conv, fd.abi_param_ty(i));
        let ty = if by_ref { p.tcx.ptr(decl.ty) } else { decl.ty };
        let l = b.locals.len() as Local;
        b.locals.push(LocalDecl { ty, name: Some(decl.name.clone()), kind: LocalKind::Arg { by_ref }, span: decl.span });
        b.map[tl as usize] = Some(Mapped { local: l, by_ref });
        if !by_ref && !b.is_copy(decl.ty) {
            b.scopes[0].push(l);
        }
    }
    let arg_count = thir.params.len();
    b.block(&thir.block, None);
    if !b.terminated[b.cur as usize] {
        b.exit_scopes(0, thir.block.span.end());
        b.terminate(Terminator::Return, thir.block.span.end());
    }
    Body { fn_id: f, locals: b.locals, arg_count, blocks: b.blocks, span: thir.span }
}

impl<'a> Builder<'a> {
    // -----------------------------------------------------------------------
    // CFG helpers

    fn new_block(&mut self) -> BlockId {
        self.blocks.push(BasicBlock { stmts: Vec::new(), term: Terminator::Unreachable, term_span: Span::DUMMY });
        self.terminated.push(false);
        (self.blocks.len() - 1) as BlockId
    }

    fn ensure_open(&mut self) {
        if self.terminated[self.cur as usize] {
            // Code after a diverging construct lands in a fresh unreachable block.
            self.cur = self.new_block();
        }
    }

    fn emit(&mut self, kind: StmtKind, span: Span) {
        self.ensure_open();
        self.blocks[self.cur as usize].stmts.push(Statement { kind, span });
    }

    fn assign(&mut self, place: Place, rv: Rvalue, span: Span) {
        self.emit(StmtKind::Assign(place, rv), span);
    }

    fn terminate(&mut self, t: Terminator, span: Span) {
        self.ensure_open();
        let b = &mut self.blocks[self.cur as usize];
        b.term = t;
        b.term_span = span;
        self.terminated[self.cur as usize] = true;
    }

    fn goto(&mut self, target: BlockId, span: Span) {
        if !self.terminated[self.cur as usize] {
            self.terminate(Terminator::Goto(target), span);
        }
    }

    fn switch_bool(&mut self, cond: Operand, if_true: BlockId, if_false: BlockId, span: Span) {
        self.terminate(Terminator::Switch { discr: cond, targets: vec![(0, if_false)], otherwise: if_true }, span);
    }

    fn temp(&mut self, ty: Ty, span: Span) -> Local {
        let l = self.locals.len() as Local;
        self.locals.push(LocalDecl { ty, name: None, kind: LocalKind::Temp, span });
        l
    }

    /// A temporary destroyed at the end of the enclosing statement.
    fn scheduled_temp(&mut self, ty: Ty, span: Span) -> Local {
        let l = self.temp(ty, span);
        if !self.is_copy(ty) {
            self.scopes.last_mut().unwrap().push(l);
        }
        l
    }

    fn user_local(&mut self, tl: thir::LocalId) -> Local {
        let d = &self.thir.locals[tl as usize];
        let l = self.locals.len() as Local;
        self.locals.push(LocalDecl { ty: d.ty, name: Some(d.name.clone()), kind: LocalKind::User, span: d.span });
        self.map[tl as usize] = Some(Mapped { local: l, by_ref: false });
        if !self.is_copy(d.ty) {
            self.scopes.last_mut().unwrap().push(l);
        }
        l
    }

    fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    /// Ends a scope: destroys its locals in reverse declaration order.
    fn pop_scope(&mut self, span: Span) {
        let s = self.scopes.pop().unwrap();
        if !self.terminated[self.cur as usize] {
            for &l in s.iter().rev() {
                self.emit(StmtKind::DropIfInit(l), span);
            }
        }
    }

    /// Destroys the locals of every scope above `depth` (for early exits).
    fn exit_scopes(&mut self, depth: usize, span: Span) {
        let mut drops = Vec::new();
        for s in self.scopes[depth..].iter().rev() {
            for &l in s.iter().rev() {
                drops.push(l);
            }
        }
        for l in drops {
            self.emit(StmtKind::DropIfInit(l), span);
        }
    }

    fn with_temps<R>(&mut self, span: Span, f: impl FnOnce(&mut Self) -> R) -> R {
        self.push_scope();
        let r = f(self);
        self.pop_scope(span);
        r
    }

    // -----------------------------------------------------------------------
    // Types

    fn tcx(&self) -> &'a nxc_sema::ty::TyCtx {
        &self.p.tcx
    }

    pub fn place_ty(&self, place: &Place) -> Ty {
        let tcx = self.tcx();
        let mut t = self.locals[place.local as usize].ty;
        for pr in &place.proj {
            t = match (pr, &*tcx.kind(t)) {
                (Proj::Deref, TyKind::Ptr(e)) => *e,
                (Proj::Field(i), TyKind::Tuple(ts)) => ts[*i as usize],
                (Proj::Field(i), TyKind::Adt(id, args)) => tcx.subst(self.p.adt(*id).fields()[*i as usize].ty, args),
                (Proj::VariantField(v, i), TyKind::Adt(id, args)) => tcx.subst(self.p.adt(*id).variants()[*v as usize].fields[*i as usize], args),
                (Proj::Index(_), TyKind::Array(e, _)) => *e,
                _ => tcx.error,
            };
        }
        t
    }

    fn is_copy(&self, t: Ty) -> bool {
        self.p.is_copy_with(t, &self.bounds)
    }

    // -----------------------------------------------------------------------
    // Places

    fn mapped(&self, tl: thir::LocalId) -> Mapped {
        self.map[tl as usize].unwrap_or_else(|| panic!("internal: unmapped local {} ({})", tl, self.thir.locals[tl as usize].name))
    }

    fn local_place(&self, tl: thir::LocalId) -> Place {
        let m = self.mapped(tl);
        if m.by_ref {
            Place { local: m.local, proj: vec![Proj::Deref] }
        } else {
            Place::local(m.local)
        }
    }

    fn prepare_place(&mut self, e: &Expr) -> PrePlace {
        match &e.kind {
            T::Local(l) => PrePlace { base: self.local_place(*l), steps: vec![] },
            T::Static(st) => {
                let ty = self.p.statics[st.idx()].ty;
                let p = self.temp(self.tcx().ptr(ty), e.span);
                self.assign(Place::local(p), Rvalue::StaticAddr(*st), e.span);
                PrePlace { base: Place { local: p, proj: vec![Proj::Deref] }, steps: vec![] }
            }
            T::Field(b, i) => {
                let mut pp = self.prepare_place(b);
                pp.steps.push(Step::Field(*i));
                pp
            }
            T::Index(b, idx) => {
                let mut pp = self.prepare_place(b);
                let len = match &*self.tcx().kind(b.ty) {
                    TyKind::Array(_, n) => *n,
                    _ => 0,
                };
                let index = self.operand_read(idx);
                pp.steps.push(Step::ArrayIndex { index, len, span: e.span });
                pp
            }
            T::IndexFn { base, index, index_fn, type_args } => {
                let mut pp = self.prepare_place(base);
                let idx = self.operand_read(index);
                pp.steps.push(Step::IndexFn { f: *index_fn, targs: type_args.clone(), index: idx, index_ty: index.ty, elem: e.ty, span: e.span });
                pp
            }
            _ => {
                let t = self.scheduled_temp(e.ty, e.span);
                self.expr_into(Place::local(t), e);
                PrePlace { base: Place::local(t), steps: vec![] }
            }
        }
    }

    fn resolve_place(&mut self, pp: PrePlace) -> Place {
        let tcx = self.tcx();
        let mut place = pp.base;
        for step in pp.steps {
            match step {
                Step::Field(i) => place = place.project(Proj::Field(i)),
                Step::ArrayIndex { index, len, span } => {
                    let t = self.temp(tcx.usize, span);
                    self.emit(StmtKind::BoundsCheck { index, len: Operand::Const(ConstValue::Int(len as i128), tcx.usize), dest: t }, span);
                    place = place.project(Proj::Index(t));
                }
                Step::IndexFn { f, targs, index, index_ty, elem, span } => {
                    let fd = self.p.func(f);
                    let self_ty = self.place_ty(&place);
                    let recv = if param_by_ref(self.p, fd.params[0].conv, fd.abi_param_ty(0)) {
                        let r = self.temp(tcx.ptr(self_ty), span);
                        self.assign(Place::local(r), Rvalue::Ref(place.clone()), span);
                        Operand::Copy(Place::local(r))
                    } else {
                        Operand::Copy(place.clone())
                    };
                    let idx = if index_ty != tcx.usize {
                        let t = self.temp(tcx.usize, span);
                        self.assign(Place::local(t), Rvalue::Cast(index, tcx.usize), span);
                        Operand::Copy(Place::local(t))
                    } else {
                        index
                    };
                    let ptr = self.temp(tcx.ptr(elem), span);
                    self.call(Callee::Fn(f, targs), vec![recv, idx], Some(Place::local(ptr)), false, span);
                    place = Place { local: ptr, proj: vec![Proj::Deref] };
                }
            }
        }
        place
    }

    fn place_of(&mut self, e: &Expr) -> Place {
        let pp = self.prepare_place(e);
        self.resolve_place(pp)
    }

    // -----------------------------------------------------------------------
    // Operands

    /// The value of `e`, consumed (moved if it is a resource).
    fn operand_owned(&mut self, e: &Expr) -> Operand {
        match &e.kind {
            T::Lit(v) => return Operand::Const(v.clone(), e.ty),
            T::Const(c) => return Operand::Const(self.p.const_values.get(c).cloned().unwrap_or(ConstValue::Unit), e.ty),
            T::FnRef(f) => return Operand::FnRef(*f),
            _ => {}
        }
        // A field or element of a temporary (`make().field`) is read through a place too:
        // `place_of` stores the temporary first.
        let place_like = matches!(e.kind, T::Local(_) | T::Static(_) | T::Field(..) | T::Index(..) | T::IndexFn { .. });
        if place_like {
            let place = self.place_of(e);
            if self.is_copy(e.ty) {
                Operand::Copy(place)
            } else {
                Operand::Move(place)
            }
        } else if let T::Move(inner) = &e.kind {
            let place = self.place_of(inner);
            if self.is_copy(e.ty) {
                Operand::Copy(place)
            } else {
                Operand::Move(place)
            }
        } else {
            let t = self.scheduled_temp(e.ty, e.span);
            self.expr_into(Place::local(t), e);
            if self.is_copy(e.ty) {
                Operand::Copy(Place::local(t))
            } else {
                Operand::Move(Place::local(t))
            }
        }
    }

    /// The value of `e` for an operator or a by-value parameter (copyable types).
    fn operand_read(&mut self, e: &Expr) -> Operand {
        if self.is_copy(e.ty) {
            self.operand_owned(e)
        } else {
            // A resource read by value: only reachable for by-value borrowed parameters
            // of resource types, which do not exist (they are passed by reference).
            let pl = self.place_of(e);
            Operand::Copy(pl)
        }
    }

    /// An owned operand materialized in a fresh temporary (precise spans for moves).
    fn operand_in_temp(&mut self, e: &Expr) -> Operand {
        let op = self.operand_owned(e);
        match &op {
            Operand::Move(p) | Operand::Copy(p) if !p.proj.is_empty() || self.locals[p.local as usize].kind != LocalKind::Temp => {
                let t = self.scheduled_temp(e.ty, e.span);
                self.assign(Place::local(t), Rvalue::Use(op), e.span);
                if self.is_copy(e.ty) {
                    Operand::Copy(Place::local(t))
                } else {
                    Operand::Move(Place::local(t))
                }
            }
            _ => op,
        }
    }

    /// A reference to the value of `e` (a place, or a temporary holding the value).
    fn ref_to(&mut self, e: &Expr) -> Operand {
        let place = self.place_of(e);
        let ty = self.place_ty(&place);
        let r = self.temp(self.tcx().ptr(ty), e.span);
        self.assign(Place::local(r), Rvalue::Ref(place), e.span);
        Operand::Copy(Place::local(r))
    }

    fn ref_place(&mut self, place: Place, span: Span) -> Operand {
        let ty = self.place_ty(&place);
        let r = self.temp(self.tcx().ptr(ty), span);
        self.assign(Place::local(r), Rvalue::Ref(place), span);
        Operand::Copy(Place::local(r))
    }

    fn const_str(&mut self, s: &str, span: Span) -> Place {
        let st = self.p.string_ty();
        let t = self.scheduled_temp(st, span);
        self.assign(Place::local(t), Rvalue::Use(Operand::Const(ConstValue::Str(s.to_string()), st)), span);
        Place::local(t)
    }

    // -----------------------------------------------------------------------
    // Calls

    /// Emits a call terminator and continues in its return block.
    fn call(&mut self, callee: Callee, args: Vec<Operand>, dest: Option<Place>, diverges: bool, span: Span) {
        if diverges {
            self.terminate(Terminator::Call { callee, args, dest: None, target: None }, span);
        } else {
            let next = self.new_block();
            self.terminate(Terminator::Call { callee, args, dest, target: Some(next) }, span);
            self.cur = next;
        }
    }

    fn lower_args(&mut self, params: &[(Convention, Ty)], args: &[thir::CallArg]) -> Vec<Operand> {
        // Phase 1: evaluate every argument, left to right.
        enum Pending {
            Value(Operand),
            Ref(PrePlace, Span),
        }
        let mut pending = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let (conv, decl_ty) = params.get(i).copied().unwrap_or((a.conv, a.expr.ty));
            if param_by_ref(self.p, conv, decl_ty) {
                if a.expr.is_place() {
                    let pp = self.prepare_place(&a.expr);
                    pending.push(Pending::Ref(pp, a.expr.span));
                } else {
                    let t = self.scheduled_temp(a.expr.ty, a.expr.span);
                    self.expr_into(Place::local(t), &a.expr);
                    pending.push(Pending::Ref(PrePlace { base: Place::local(t), steps: vec![] }, a.expr.span));
                }
            } else {
                let op = self.operand_in_temp(&a.expr);
                pending.push(Pending::Value(op));
            }
        }
        // Phase 2: resolve the places passed by reference.
        let mut out = Vec::new();
        for p in pending {
            match p {
                Pending::Value(op) => out.push(op),
                Pending::Ref(pp, span) => {
                    let place = self.resolve_place(pp);
                    let r = self.ref_place(place, span);
                    out.push(r);
                }
            }
        }
        out
    }

    fn lower_call(&mut self, e: &Expr, callee: &TCallee, args: &[thir::CallArg], dest: Option<Place>) {
        let tcx = self.tcx();
        let span = e.span;
        let ret_ty = e.ty;
        let diverges = tcx.is_never(ret_ty);
        let unit_ret = ret_ty == tcx.unit || diverges;
        let call_dest = if unit_ret { None } else { dest.clone() };
        match callee {
            TCallee::Fn(f, targs) => {
                let fd = self.p.func(*f);
                let params: Vec<(Convention, Ty)> = fd.params.iter().enumerate().map(|(i, p)| (p.conv, fd.abi_param_ty(i))).collect();
                let ops = self.lower_args(&params, args);
                self.call(Callee::Fn(*f, targs.clone()), ops, call_dest, diverges, span);
            }
            TCallee::Indirect(fexpr) => {
                // An anonymous function is called in place (it is never moved).
                let fop = if matches!(&*tcx.kind(fexpr.ty), TyKind::Closure(..)) {
                    let pl = self.place_of(fexpr);
                    Operand::Copy(pl)
                } else {
                    self.operand_in_temp(fexpr)
                };
                let params: Vec<(Convention, Ty)> = match &*tcx.kind(fexpr.ty) {
                    TyKind::Fn(ps, _) | TyKind::Closure(ps, _) => ps.clone(),
                    _ => vec![],
                };
                let ops = self.lower_args(&params, args);
                self.call(Callee::Indirect(fop), ops, call_dest, diverges, span);
            }
            TCallee::Prim(op) => {
                let mut ops = Vec::new();
                for a in args {
                    let o = if a.conv == Convention::Owned { self.operand_in_temp(&a.expr) } else { self.operand_read(&a.expr) };
                    ops.push(o);
                }
                self.call(Callee::Prim(*op), ops, call_dest, false, span);
            }
            TCallee::Builtin(b) => self.lower_builtin(*b, args, span),
        }
        if unit_ret && !diverges {
            if let Some(d) = dest {
                self.assign(d, Rvalue::Use(Operand::Const(ConstValue::Unit, tcx.unit)), span);
            }
        }
    }

    fn lower_builtin(&mut self, b: Builtin, args: &[thir::CallArg], span: Span) {
        let tcx = self.tcx();
        match b {
            Builtin::Print | Builtin::Println | Builtin::Eprint | Builtin::Eprintln => {
                let newline = matches!(b, Builtin::Println | Builtin::Eprintln);
                let stderr = matches!(b, Builtin::Eprint | Builtin::Eprintln);
                let s = match args.first() {
                    Some(a) if self.p.is_string(a.expr.ty) => self.ref_to(&a.expr),
                    Some(a) => {
                        let st = self.p.string_ty();
                        let t = self.scheduled_temp(st, span);
                        self.call(Callee::Runtime(RtFn::StrEmpty), vec![], Some(Place::local(t)), false, span);
                        let v = self.operand_read(&a.expr);
                        let r = self.ref_place(Place::local(t), span);
                        self.call(Callee::Runtime(RtFn::Fmt { spec: None }), vec![r.clone(), v], None, false, span);
                        r
                    }
                    None => {
                        let p = self.const_str("", span);
                        self.ref_place(p, span)
                    }
                };
                self.call(Callee::Runtime(RtFn::Print { newline, stderr }), vec![s], None, false, span);
            }
            Builtin::Panic => {
                let m = self.ref_to(&args[0].expr);
                self.call(Callee::Runtime(RtFn::PanicStr), vec![m], None, true, span);
            }
            Builtin::Unreachable => {
                self.call(Callee::Runtime(RtFn::PanicMsg("entered unreachable code".into())), vec![], None, true, span);
            }
            Builtin::Assert => {
                let c = self.with_temps(span, |b| b.operand_read(&args[0].expr));
                let fail = self.new_block();
                let ok = self.new_block();
                self.switch_bool(c, ok, fail, span);
                self.cur = fail;
                match args.get(1) {
                    Some(m) => {
                        let r = self.ref_to(&m.expr);
                        self.call(Callee::Runtime(RtFn::PanicStr), vec![r], None, true, span);
                    }
                    None => {
                        let text = self.sm.snippet(args[0].expr.span).to_string();
                        let msg = if text.is_empty() || text.len() > 200 { "assertion failed".to_string() } else { format!("assertion failed: {}", text) };
                        self.call(Callee::Runtime(RtFn::PanicMsg(msg)), vec![], None, true, span);
                    }
                }
                self.cur = ok;
                let _ = tcx;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Expressions

    /// Evaluates `e` for its side effects only.
    fn expr_discard(&mut self, e: &Expr) {
        let tcx = self.tcx();
        match &e.kind {
            T::If { .. } | T::IfLet { .. } | T::Match { .. } | T::Block(_) => self.control_into(None, e),
            T::Return(_) | T::Break(_) | T::Continue(_) => self.expr_into(Place::local(RETURN), e),
            T::Call { callee, args } => self.lower_call(e, callee, args, None),
            T::Lit(_) | T::Const(_) | T::FnRef(_) | T::Error => {}
            _ if e.is_place() && self.is_copy(e.ty) => {
                // Evaluate for bounds checks only.
                let _ = self.place_of(e);
            }
            _ => {
                let t = self.scheduled_temp(e.ty, e.span);
                self.expr_into(Place::local(t), e);
                let _ = tcx;
            }
        }
    }

    /// Writes the value of `e` into the (uninitialized) place `dest`.
    fn expr_into(&mut self, dest: Place, e: &Expr) {
        let tcx = self.tcx();
        let span = e.span;
        match &e.kind {
            T::Error => self.terminate(Terminator::Unreachable, span),
            T::Lit(_) | T::Const(_) | T::FnRef(_) => {
                let op = self.operand_owned(e);
                self.assign(dest, Rvalue::Use(op), span);
            }
            T::Local(_) | T::Static(_) | T::Field(..) | T::Index(..) | T::IndexFn { .. } | T::Move(_) => {
                let op = self.operand_owned(e);
                self.assign(dest, Rvalue::Use(op), span);
            }
            T::Unary(op, x) => {
                let o = self.operand_read(x);
                self.assign(dest, Rvalue::Unary(*op, o), span);
            }
            T::Binary(op, a, b) => self.binary_into(dest, *op, a, b, span),
            T::Cast(x) => {
                let o = self.operand_read(x);
                self.assign(dest, Rvalue::Cast(o, e.ty), span);
            }
            T::Call { callee, args } => self.lower_call(e, callee, args, Some(dest)),
            T::Adt { adt, variant, type_args, fields } => {
                let ops: Vec<Operand> = fields.iter().map(|f| self.operand_owned(f)).collect();
                self.assign(dest, Rvalue::Aggregate(AggKind::Adt { adt: *adt, variant: *variant, type_args: type_args.clone() }, ops), span);
            }
            T::Tuple(xs) => {
                let ops: Vec<Operand> = xs.iter().map(|f| self.operand_owned(f)).collect();
                self.assign(dest, Rvalue::Aggregate(AggKind::Tuple, ops), span);
            }
            T::Array(xs) => {
                let ops: Vec<Operand> = xs.iter().map(|f| self.operand_owned(f)).collect();
                self.assign(dest, Rvalue::Aggregate(AggKind::Array, ops), span);
            }
            T::ArrayRepeat(x, n) => {
                let o = self.operand_owned(x);
                self.assign(dest, Rvalue::Repeat(o, *n), span);
            }
            T::If { .. } | T::IfLet { .. } | T::Match { .. } | T::Block(_) => self.control_into(Some(dest), e),
            T::Return(v) => {
                if let Some(v) = v {
                    self.expr_into(Place::local(RETURN), v);
                } else if self.thir.ret_ty == tcx.unit {
                    self.assign(Place::local(RETURN), Rvalue::Use(Operand::Const(ConstValue::Unit, tcx.unit)), span);
                }
                self.exit_scopes(0, span);
                self.terminate(Terminator::Return, span);
            }
            T::Break(id) | T::Continue(id) => {
                let is_break = matches!(e.kind, T::Break(_));
                let li = self.loops.iter().rposition(|l| l.id == *id).expect("loop");
                let (target, depth) = {
                    let l = &self.loops[li];
                    (if is_break { l.break_bb } else { l.continue_bb }, l.depth)
                };
                self.exit_scopes(depth, span);
                self.terminate(Terminator::Goto(target), span);
            }
            T::Try(x, kind) => self.lower_try(dest, x, *kind, span),
            T::Interp(parts) => self.lower_interp(dest, parts, span),
            T::Closure { f, captures } => {
                let mut env = Vec::new();
                for &(l, _) in captures {
                    let place = self.local_place(l);
                    env.push(self.ref_place(place, span));
                }
                self.assign(dest, Rvalue::Closure { f: *f, env }, span);
            }
            T::FnToClosure(x) => {
                let o = self.operand_read(x);
                self.assign(dest, Rvalue::FnToClosure(o), span);
            }
            T::ToDyn(x) => {
                let o = self.operand_in_temp(x);
                self.assign(dest, Rvalue::ToDyn(o, x.ty), span);
            }
        }
    }

    fn binary_into(&mut self, dest: Place, op: BinOp, a: &Expr, b: &Expr, span: Span) {
        let tcx = self.tcx();
        match op {
            BinOp::And | BinOp::Or => {
                // Short-circuit evaluation.
                let av = self.operand_read(a);
                self.assign(dest.clone(), Rvalue::Use(av), span);
                let rhs = self.new_block();
                let done = self.new_block();
                let cond = Operand::Copy(dest.clone());
                if op == BinOp::And {
                    self.switch_bool(cond, rhs, done, span);
                } else {
                    self.switch_bool(cond, done, rhs, span);
                }
                self.cur = rhs;
                let bv = self.with_temps(b.span, |bl| bl.operand_read(b));
                self.assign(dest, Rvalue::Use(bv), span);
                self.goto(done, span);
                self.cur = done;
            }
            _ if self.p.is_string(a.ty) => {
                let ra = self.ref_to(a);
                let rb = self.ref_to(b);
                match op {
                    BinOp::Add => self.call(Callee::Runtime(RtFn::StrConcat), vec![ra, rb], Some(dest), false, span),
                    BinOp::Eq | BinOp::Ne => {
                        if op == BinOp::Eq {
                            self.call(Callee::Runtime(RtFn::StrEq), vec![ra, rb], Some(dest), false, span);
                        } else {
                            let t = self.temp(tcx.bool, span);
                            self.call(Callee::Runtime(RtFn::StrEq), vec![ra, rb], Some(Place::local(t)), false, span);
                            self.assign(dest, Rvalue::Unary(UnOp::Not, Operand::Copy(Place::local(t))), span);
                        }
                    }
                    _ => {
                        let t = self.temp(tcx.i32, span);
                        self.call(Callee::Runtime(RtFn::StrCmp), vec![ra, rb], Some(Place::local(t)), false, span);
                        self.assign(dest, Rvalue::Binary(op, Operand::Copy(Place::local(t)), Operand::Const(ConstValue::Int(0), tcx.i32)), span);
                    }
                }
            }
            _ => {
                let av = self.operand_read(a);
                let bv = self.operand_read(b);
                self.assign(dest, Rvalue::Binary(op, av, bv), span);
            }
        }
    }

    fn lower_interp(&mut self, dest: Place, parts: &[InterpPart], span: Span) {
        self.call(Callee::Runtime(RtFn::StrEmpty), vec![], Some(dest.clone()), false, span);
        for part in parts {
            match part {
                InterpPart::Text(t) => {
                    let lit = self.const_str(t, span);
                    let r = self.ref_place(dest.clone(), span);
                    let l = self.ref_place(lit, span);
                    self.call(Callee::Runtime(RtFn::StrPushStr), vec![r, l], None, false, span);
                }
                InterpPart::Value(x, spec) => {
                    let v = if self.p.is_string(x.ty) { self.ref_to(x) } else { self.operand_read(x) };
                    let r = self.ref_place(dest.clone(), span);
                    self.call(Callee::Runtime(RtFn::Fmt { spec: spec.clone() }), vec![r, v], None, false, x.span);
                }
            }
        }
    }

    fn lower_try(&mut self, dest: Place, x: &Expr, kind: TryKind, span: Span) {
        let tcx = self.tcx();
        let h = self.scheduled_temp(x.ty, x.span);
        self.expr_into(Place::local(h), x);
        let (adt, targs) = tcx.as_adt(x.ty).expect("try operand");
        let a = self.p.adt(adt);
        let ok_v = a.variant_index(if kind == TryKind::Option { "Some" } else { "Ok" }).unwrap_or(0);
        let err_v = a.variant_index(if kind == TryKind::Option { "None" } else { "Err" }).unwrap_or(1);
        let d = self.temp(tcx.u32, span);
        self.assign(Place::local(d), Rvalue::Discriminant(Place::local(h)), span);
        let ok_bb = self.new_block();
        let err_bb = self.new_block();
        self.terminate(Terminator::Switch { discr: Operand::Copy(Place::local(d)), targets: vec![(ok_v as u128, ok_bb)], otherwise: err_bb }, span);
        // Error path: build the function's own Err/None and return.
        self.cur = err_bb;
        let ret_ty = self.thir.ret_ty;
        let (ret_adt, ret_args) = tcx.as_adt(ret_ty).unwrap_or((adt, targs.clone()));
        let ops = if kind == TryKind::Result {
            vec![Operand::Move(Place::local(h).project(Proj::VariantField(err_v, 0)))]
        } else {
            vec![]
        };
        let ret_err_v = self.p.adt(ret_adt).variant_index(if kind == TryKind::Option { "None" } else { "Err" }).unwrap_or(1);
        self.assign(Place::local(RETURN), Rvalue::Aggregate(AggKind::Adt { adt: ret_adt, variant: Some(ret_err_v), type_args: ret_args }, ops), span);
        self.emit(StmtKind::MarkMoved(h), span);
        self.exit_scopes(0, span);
        self.terminate(Terminator::Return, span);
        // Success path: take the payload.
        self.cur = ok_bb;
        let payload = Place::local(h).project(Proj::VariantField(ok_v, 0));
        let op = if self.is_copy(self.place_ty(&payload)) { Operand::Copy(payload) } else { Operand::Move(payload) };
        self.assign(dest, Rvalue::Use(op), span);
        self.emit(StmtKind::MarkMoved(h), span);
    }

    // -----------------------------------------------------------------------
    // Control flow

    fn control_into(&mut self, dest: Option<Place>, e: &Expr) {
        let span = e.span;
        match &e.kind {
            T::Block(b) => self.block(b, dest),
            T::If { cond, then, else_ } => {
                let c = self.with_temps(cond.span, |bl| bl.operand_read(cond));
                let then_bb = self.new_block();
                let else_bb = self.new_block();
                let join = self.new_block();
                self.switch_bool(c, then_bb, else_bb, span);
                self.cur = then_bb;
                self.block(then, dest.clone());
                self.goto(join, span);
                self.cur = else_bb;
                if let Some(x) = else_ {
                    self.branch_into(dest, x);
                }
                self.goto(join, span);
                self.cur = join;
            }
            T::IfLet { pat, scrutinee, then, else_, mutable } => {
                self.push_scope();
                let (sp, owned) = self.scrutinee(scrutinee);
                let else_bb = self.new_block();
                let join = self.new_block();
                self.test_pat(&sp, pat, else_bb);
                self.push_scope();
                self.bind_arm(&sp, pat, owned, *mutable, None, else_bb);
                self.block(then, dest.clone());
                self.pop_scope(span);
                self.goto(join, span);
                self.cur = else_bb;
                if let Some(x) = else_ {
                    self.branch_into(dest, x);
                }
                self.goto(join, span);
                self.cur = join;
                self.pop_scope(span);
            }
            T::Match { scrutinee, arms, mutable } => {
                self.push_scope();
                let (sp, owned) = self.scrutinee(scrutinee);
                let join = self.new_block();
                for arm in arms {
                    let next = self.new_block();
                    self.test_pat(&sp, &arm.pat, next);
                    self.push_scope();
                    self.bind_arm(&sp, &arm.pat, owned, *mutable, arm.guard.as_ref(), next);
                    self.branch_into(dest.clone(), &arm.body);
                    self.pop_scope(arm.span);
                    self.goto(join, arm.span);
                    self.cur = next;
                }
                // Exhaustiveness is guaranteed by the front end.
                self.terminate(Terminator::Unreachable, span);
                self.cur = join;
                self.pop_scope(span);
            }
            _ => unreachable!(),
        }
    }

    fn branch_into(&mut self, dest: Option<Place>, e: &Expr) {
        match dest {
            Some(d) => self.expr_into(d, e),
            None => self.expr_discard(e),
        }
    }

    /// Evaluates a scrutinee: a place is matched in place; a value is stored in a
    /// hidden local owned by the enclosing scope.
    fn scrutinee(&mut self, s: &Expr) -> (Place, bool) {
        if s.is_place() {
            (self.place_of(s), false)
        } else {
            let h = self.temp(s.ty, s.span);
            if !self.is_copy(s.ty) {
                self.scopes.last_mut().unwrap().push(h);
            }
            self.with_temps(s.span, |b| b.expr_into(Place::local(h), s));
            (Place::local(h), true)
        }
    }

    fn block(&mut self, b: &thir::Block, dest: Option<Place>) {
        self.push_scope();
        for s in &b.stmts {
            self.stmt(s);
        }
        match (&b.tail, dest) {
            (Some(t), Some(d)) => self.with_temps(t.span, |bl| bl.expr_into(d, t)),
            (Some(t), None) => self.with_temps(t.span, |bl| bl.expr_discard(t)),
            (None, Some(d)) => {
                if !self.terminated[self.cur as usize] && self.place_ty(&d) == self.tcx().unit {
                    self.assign(d, Rvalue::Use(Operand::Const(ConstValue::Unit, self.tcx().unit)), b.span);
                }
            }
            (None, None) => {}
        }
        self.pop_scope(b.span.end());
    }

    // -----------------------------------------------------------------------
    // Patterns

    /// Emits the tests of `pat` against `place`; falls through on success.
    fn test_pat(&mut self, place: &Place, pat: &Pat, fail: BlockId) {
        let tcx = self.tcx();
        let span = pat.span;
        match &pat.kind {
            PatKind::Wild | PatKind::Binding(_) | PatKind::Error => {}
            PatKind::Lit(ConstValue::Str(s)) => {
                let lit = self.const_str(s, span);
                let a = self.ref_place(place.clone(), span);
                let b = self.ref_place(lit, span);
                let t = self.temp(tcx.bool, span);
                self.call(Callee::Runtime(RtFn::StrEq), vec![a, b], Some(Place::local(t)), false, span);
                let ok = self.new_block();
                self.switch_bool(Operand::Copy(Place::local(t)), ok, fail, span);
                self.cur = ok;
            }
            PatKind::Lit(v) => {
                let t = self.temp(tcx.bool, span);
                self.assign(Place::local(t), Rvalue::Binary(BinOp::Eq, Operand::Copy(place.clone()), Operand::Const(v.clone(), pat.ty)), span);
                let ok = self.new_block();
                self.switch_bool(Operand::Copy(Place::local(t)), ok, fail, span);
                self.cur = ok;
            }
            PatKind::Range(lo, hi, inclusive) => {
                let t = self.temp(tcx.bool, span);
                self.assign(Place::local(t), Rvalue::Binary(BinOp::Ge, Operand::Copy(place.clone()), Operand::Const(lo.clone(), pat.ty)), span);
                let ok1 = self.new_block();
                self.switch_bool(Operand::Copy(Place::local(t)), ok1, fail, span);
                self.cur = ok1;
                let t2 = self.temp(tcx.bool, span);
                let op = if *inclusive { BinOp::Le } else { BinOp::Lt };
                self.assign(Place::local(t2), Rvalue::Binary(op, Operand::Copy(place.clone()), Operand::Const(hi.clone(), pat.ty)), span);
                let ok2 = self.new_block();
                self.switch_bool(Operand::Copy(Place::local(t2)), ok2, fail, span);
                self.cur = ok2;
            }
            PatKind::Tuple(ps) | PatKind::Struct { fields: ps, .. } => {
                for (i, p) in ps.iter().enumerate() {
                    self.test_pat(&place.project(Proj::Field(i as u32)), p, fail);
                }
            }
            PatKind::Variant { variant, fields, .. } => {
                let d = self.temp(tcx.u32, span);
                self.assign(Place::local(d), Rvalue::Discriminant(place.clone()), span);
                let ok = self.new_block();
                self.terminate(Terminator::Switch { discr: Operand::Copy(Place::local(d)), targets: vec![(*variant as u128, ok)], otherwise: fail }, span);
                self.cur = ok;
                for (i, p) in fields.iter().enumerate() {
                    self.test_pat(&place.project(Proj::VariantField(*variant, i as u32)), p, fail);
                }
            }
            PatKind::Or(alts) => {
                let success = self.new_block();
                for alt in alts {
                    let next = self.new_block();
                    self.test_pat(place, alt, next);
                    self.goto(success, span);
                    self.cur = next;
                }
                self.goto(fail, span);
                self.cur = success;
            }
        }
    }

    /// Collects (binding, sub-place) pairs of a pattern.
    fn binding_places(&self, place: &Place, pat: &Pat, out: &mut Vec<(thir::LocalId, Place)>) {
        match &pat.kind {
            PatKind::Binding(l) => out.push((*l, place.clone())),
            PatKind::Tuple(ps) | PatKind::Struct { fields: ps, .. } => {
                for (i, p) in ps.iter().enumerate() {
                    self.binding_places(&place.project(Proj::Field(i as u32)), p, out);
                }
            }
            PatKind::Variant { variant, fields, .. } => {
                for (i, p) in fields.iter().enumerate() {
                    self.binding_places(&place.project(Proj::VariantField(*variant, i as u32)), p, out);
                }
            }
            _ => {}
        }
    }

    /// Binds as views: copies for copyable values, pointers otherwise. Modifiable views
    /// (`mutable`) are always pointers.
    fn bind_views(&mut self, place: &Place, pat: &Pat, mutable: bool) {
        let mut bs = Vec::new();
        self.binding_places(place, pat, &mut bs);
        for (tl, sub) in bs {
            let d = &self.thir.locals[tl as usize];
            let ty = d.ty;
            let name = d.name.clone();
            let span = d.span;
            let l = self.locals.len() as Local;
            if self.is_copy(ty) && !mutable {
                self.locals.push(LocalDecl { ty, name: Some(name), kind: LocalKind::User, span });
                self.assign(Place::local(l), Rvalue::Use(Operand::Copy(sub)), span);
                self.map[tl as usize] = Some(Mapped { local: l, by_ref: false });
            } else {
                self.locals.push(LocalDecl { ty: self.tcx().ptr(ty), name: Some(name), kind: LocalKind::User, span });
                self.assign(Place::local(l), Rvalue::Ref(sub), span);
                self.map[tl as usize] = Some(Mapped { local: l, by_ref: true });
            }
        }
    }

    /// Does the pattern take ownership of a resource?
    fn pat_moves(&self, pat: &Pat) -> bool {
        let mut bs = Vec::new();
        pat.bindings(&mut bs);
        bs.iter().any(|&l| !self.is_copy(self.thir.locals[l as usize].ty))
    }

    /// Binds the pattern of a matched arm, evaluating its guard if any.
    fn bind_arm(&mut self, place: &Place, pat: &Pat, owned: bool, mutable: bool, guard: Option<&Expr>, fail: BlockId) {
        let moves = owned && !mutable && self.pat_moves(pat);
        if let Some(g) = guard {
            self.bind_views(place, pat, mutable);
            let c = self.with_temps(g.span, |b| b.operand_read(g));
            let ok = self.new_block();
            self.switch_bool(c, ok, fail, g.span);
            self.cur = ok;
        }
        if !moves {
            if guard.is_none() {
                self.bind_views(place, pat, mutable);
            }
            return;
        }
        // Owned bindings: move the parts out, destroy the rest, forget the whole.
        let mut bs = Vec::new();
        self.binding_places(place, pat, &mut bs);
        for (tl, sub) in bs {
            let l = self.user_local(tl);
            let ty = self.locals[l as usize].ty;
            let op = if self.is_copy(ty) { Operand::Copy(sub) } else { Operand::Move(sub) };
            self.assign(Place::local(l), Rvalue::Use(op), pat.span);
        }
        self.drop_rest(place, pat);
        if place.proj.is_empty() {
            self.emit(StmtKind::MarkMoved(place.local), pat.span);
        }
    }

    /// Destroys the parts of `place` that `pat` does not move out.
    fn drop_rest(&mut self, place: &Place, pat: &Pat) {
        let span = pat.span;
        if !self.pat_moves(pat) {
            self.emit(StmtKind::Drop(place.clone()), span);
            return;
        }
        match &pat.kind {
            PatKind::Binding(_) => {}
            PatKind::Tuple(ps) => {
                for (i, p) in ps.iter().enumerate() {
                    self.drop_rest(&place.project(Proj::Field(i as u32)), p);
                }
            }
            PatKind::Struct { adt, fields, .. } => {
                let indirect: Vec<bool> = match &self.p.adt(*adt).kind {
                    AdtKind::Struct { fields } => fields.iter().map(|f| f.indirect).collect(),
                    _ => vec![],
                };
                for (i, p) in fields.iter().enumerate() {
                    self.drop_rest(&place.project(Proj::Field(i as u32)), p);
                    if indirect.get(i).copied().unwrap_or(false) {
                        self.emit(StmtKind::FreeBox(place.clone(), BoxRef::Field(i as u32)), span);
                    }
                }
            }
            PatKind::Variant { adt, variant, fields, .. } => {
                for (i, p) in fields.iter().enumerate() {
                    self.drop_rest(&place.project(Proj::VariantField(*variant, i as u32)), p);
                }
                if self.p.adt(*adt).variants()[*variant as usize].indirect {
                    self.emit(StmtKind::FreeBox(place.clone(), BoxRef::Variant(*variant)), span);
                }
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Statements

    fn stmt(&mut self, s: &thir::Stmt) {
        let tcx = self.tcx();
        let span = s.span;
        match &s.kind {
            S::Let { pat, init } => match &pat.kind {
                PatKind::Binding(tl) => {
                    let l = self.user_local(*tl);
                    self.with_temps(span, |b| b.expr_into(Place::local(l), init));
                }
                PatKind::Wild => self.with_temps(span, |b| b.expr_discard(init)),
                _ => {
                    let h = self.scheduled_temp(init.ty, init.span);
                    self.with_temps(span, |b| b.expr_into(Place::local(h), init));
                    let place = Place::local(h);
                    self.bind_arm(&place, pat, true, false, None, 0);
                    if !self.pat_moves(pat) {
                        // Copyable parts were copied: nothing else to do.
                    }
                }
            },
            S::Var { local, init } => {
                let l = self.user_local(*local);
                if let Some(e) = init {
                    self.with_temps(span, |b| b.expr_into(Place::local(l), e));
                }
            }
            S::Assign { place, value } => {
                self.with_temps(span, |b| {
                    let pp = b.prepare_place(place);
                    let v = b.operand_owned(value);
                    let dst = b.resolve_place(pp);
                    if !b.is_copy(place.ty) {
                        if dst.proj.is_empty() {
                            b.emit(StmtKind::DropIfInit(dst.local), span);
                        } else {
                            b.emit(StmtKind::Drop(dst.clone()), span);
                        }
                    }
                    b.assign(dst, Rvalue::Use(v), span);
                });
            }
            S::CompoundAssign { op, place, value } => {
                self.with_temps(span, |b| {
                    let pp = b.prepare_place(place);
                    if b.p.is_string(place.ty) {
                        let v = b.ref_to(value);
                        let dst = b.resolve_place(pp);
                        let r = b.ref_place(dst, span);
                        b.call(Callee::Runtime(RtFn::StrPushStr), vec![r, v], None, false, span);
                    } else {
                        let v = b.operand_read(value);
                        let dst = b.resolve_place(pp);
                        b.assign(dst.clone(), Rvalue::Binary(*op, Operand::Copy(dst), v), span);
                    }
                });
            }
            S::Expr(e) => self.with_temps(span, |b| b.expr_discard(e)),
            S::While { id, cond, body } => {
                let head = self.new_block();
                let body_bb = self.new_block();
                let exit = self.new_block();
                self.goto(head, span);
                self.cur = head;
                let c = self.with_temps(cond.span, |b| b.operand_read(cond));
                self.switch_bool(c, body_bb, exit, cond.span);
                self.cur = body_bb;
                self.loops.push(LoopInfo { id: *id, break_bb: exit, continue_bb: head, depth: self.scopes.len() });
                self.block(body, None);
                self.loops.pop();
                self.goto(head, span);
                self.cur = exit;
            }
            S::ForRange { id, var, start, end, inclusive, body } => {
                let ty = start.ty;
                let (s0, e0) = self.with_temps(span, |b| (b.operand_read(start), b.operand_read(end)));
                let cur = self.temp(ty, span);
                let endv = self.temp(ty, span);
                self.assign(Place::local(cur), Rvalue::Use(s0), span);
                self.assign(Place::local(endv), Rvalue::Use(e0), span);
                let head = self.new_block();
                let body_bb = self.new_block();
                let cont = self.new_block();
                let exit = self.new_block();
                self.goto(head, span);
                self.cur = head;
                let c = self.temp(tcx.bool, span);
                let cmp = if *inclusive { BinOp::Le } else { BinOp::Lt };
                self.assign(Place::local(c), Rvalue::Binary(cmp, Operand::Copy(Place::local(cur)), Operand::Copy(Place::local(endv))), span);
                self.switch_bool(Operand::Copy(Place::local(c)), body_bb, exit, span);
                self.cur = body_bb;
                self.push_scope();
                if let Some(v) = var {
                    let l = self.user_local(*v);
                    self.assign(Place::local(l), Rvalue::Use(Operand::Copy(Place::local(cur))), span);
                }
                self.loops.push(LoopInfo { id: *id, break_bb: exit, continue_bb: cont, depth: self.scopes.len() - 1 });
                self.block(body, None);
                self.loops.pop();
                self.pop_scope(span);
                self.goto(cont, span);
                self.cur = cont;
                if *inclusive {
                    // Stop before incrementing past the end: no overflow at `T.MAX`.
                    let last = self.temp(tcx.bool, span);
                    self.assign(Place::local(last), Rvalue::Binary(BinOp::Eq, Operand::Copy(Place::local(cur)), Operand::Copy(Place::local(endv))), span);
                    let inc = self.new_block();
                    self.switch_bool(Operand::Copy(Place::local(last)), exit, inc, span);
                    self.cur = inc;
                }
                self.assign(
                    Place::local(cur),
                    Rvalue::Binary(BinOp::AddWrap, Operand::Copy(Place::local(cur)), Operand::Const(ConstValue::Int(1), ty)),
                    span,
                );
                self.goto(head, span);
                self.cur = exit;
            }
            S::ForEach { id, pat, iter, kind, elem_ty, body, mutable } => {
                self.push_scope();
                let (sp, _) = self.scrutinee(iter);
                let idx = self.temp(tcx.usize, span);
                self.assign(Place::local(idx), Rvalue::Use(Operand::Const(ConstValue::Int(0), tcx.usize)), span);
                let head = self.new_block();
                let body_bb = self.new_block();
                let cont = self.new_block();
                let exit = self.new_block();
                self.goto(head, span);
                self.cur = head;
                let len_op = match kind {
                    IterKind::Array(n) => Operand::Const(ConstValue::Int(*n as i128), tcx.usize),
                    IterKind::Indexed { len_field, .. } => Operand::Copy(sp.project(Proj::Field(*len_field))),
                    IterKind::Str => {
                        let sid = self.p.lang.string.unwrap();
                        let lf = self.p.adt(sid).field_index("len").unwrap_or(1);
                        Operand::Copy(sp.project(Proj::Field(lf)))
                    }
                };
                let c = self.temp(tcx.bool, span);
                self.assign(Place::local(c), Rvalue::Binary(BinOp::Lt, Operand::Copy(Place::local(idx)), len_op), span);
                self.switch_bool(Operand::Copy(Place::local(c)), body_bb, exit, span);
                self.cur = body_bb;
                self.push_scope();
                match kind {
                    IterKind::Array(_) => {
                        let elem = sp.project(Proj::Index(idx));
                        self.bind_views(&elem, pat, *mutable);
                        self.assign(
                            Place::local(idx),
                            Rvalue::Binary(BinOp::AddWrap, Operand::Copy(Place::local(idx)), Operand::Const(ConstValue::Int(1), tcx.usize)),
                            span,
                        );
                    }
                    IterKind::Indexed { index_fn, type_args, .. } => {
                        let fd = self.p.func(*index_fn);
                        let self_ty = self.place_ty(&sp);
                        let recv = if param_by_ref(self.p, fd.params[0].conv, fd.abi_param_ty(0)) {
                            self.ref_place(sp.clone(), span)
                        } else {
                            let _ = self_ty;
                            Operand::Copy(sp.clone())
                        };
                        let ptr = self.temp(tcx.ptr(*elem_ty), span);
                        self.call(Callee::Fn(*index_fn, type_args.clone()), vec![recv, Operand::Copy(Place::local(idx))], Some(Place::local(ptr)), false, span);
                        self.assign(
                            Place::local(idx),
                            Rvalue::Binary(BinOp::AddWrap, Operand::Copy(Place::local(idx)), Operand::Const(ConstValue::Int(1), tcx.usize)),
                            span,
                        );
                        let elem = Place { local: ptr, proj: vec![Proj::Deref] };
                        self.bind_views(&elem, pat, *mutable);
                    }
                    IterKind::Str => {
                        let ch = self.temp(tcx.char, span);
                        let r = self.ref_place(sp.clone(), span);
                        let ri = self.ref_place(Place::local(idx), span);
                        self.call(Callee::Runtime(RtFn::StrDecodeChar), vec![r, ri], Some(Place::local(ch)), false, span);
                        self.bind_views(&Place::local(ch), pat, false);
                    }
                }
                self.loops.push(LoopInfo { id: *id, break_bb: exit, continue_bb: cont, depth: self.scopes.len() - 1 });
                self.block(body, None);
                self.loops.pop();
                self.pop_scope(span);
                self.goto(cont, span);
                self.cur = cont;
                self.goto(head, span);
                self.cur = exit;
                self.pop_scope(span);
            }
        }
    }
}

#[allow(dead_code)]
fn is_hidden(k: TLocalKind) -> bool {
    k == TLocalKind::Hidden
}

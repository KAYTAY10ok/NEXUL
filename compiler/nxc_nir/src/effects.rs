//! Effects `with noalloc, nonblocking` (ADR 0018 §1), checked on the graph of the
//! instances a function can reach after monomorphization: the callees need no annotation.
//! A function with effects may reach neither an allocation (E0601) nor a blocking call
//! (E0602), nor an indirect call or a recursion (E0603), which would make the other
//! checks and the stack bound impossible. Panic paths are not effects: a panic stops.

use crate::nir::*;
use nxc_sema::defs::{AdtKind, Builtin, FnId, FnKind, Program};
use nxc_sema::ty::{Ty, TyKind};
use nxc_syntax::{DiagSink, Diagnostic, Span};
use std::collections::HashMap;

type Inst = (FnId, Vec<Ty>);

/// What a function does by itself, or a call to another instance.
#[derive(Clone)]
enum Fact {
    Calls(usize),
    Allocates(String),
    Blocks(String),
    Indirect(String),
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Alloc,
    Block,
    Indirect,
    Recursion,
}

/// The first violation of each kind reachable from an instance: the path of calls (the
/// instances after the first one, and where each call is written) and what it ends on.
#[derive(Clone, Default)]
struct Summary {
    alloc: Option<Path>,
    block: Option<Path>,
    indirect: Option<Path>,
    recursion: Option<Path>,
}

#[derive(Clone)]
struct Path {
    /// The call or operation, in the body of the instance the summary belongs to.
    span: Span,
    /// The instances called, in order.
    via: Vec<usize>,
    what: String,
}

struct Graph<'a> {
    p: &'a Program,
    bodies: &'a HashMap<FnId, Body>,
    ids: HashMap<Inst, usize>,
    insts: Vec<Inst>,
    facts: Vec<Option<Vec<(Fact, Span)>>>,
}

pub fn check(p: &Program, bodies: &HashMap<FnId, Body>, diags: &DiagSink) {
    let declared: Vec<FnId> = (0..p.fns.len() as u32).map(FnId).filter(|&f| p.func(f).effects.declared() && bodies.contains_key(&f)).collect();
    if declared.is_empty() {
        return;
    }
    let mut g = Graph { p, bodies, ids: HashMap::new(), insts: Vec::new(), facts: Vec::new() };
    // A generic function is checked for each of its instances: they are found from the
    // functions without generics, as the code generator finds them.
    let mut roots: Vec<usize> = Vec::new();
    if declared.iter().any(|&f| !p.func(f).generics.is_empty()) {
        let mut ids: Vec<FnId> = bodies.keys().copied().filter(|&f| p.func(f).generics.is_empty()).collect();
        ids.sort();
        let mut stack: Vec<usize> = ids.into_iter().map(|f| g.node(f, vec![])).collect();
        while let Some(n) = stack.pop() {
            if g.facts[n].is_none() {
                let fs = g.scan(n);
                stack.extend(fs.iter().filter_map(|(f, _)| if let Fact::Calls(m) = f { Some(*m) } else { None }));
                g.facts[n] = Some(fs);
            }
        }
        roots.extend((0..g.insts.len()).filter(|&n| p.func(g.insts[n].0).effects.declared()));
    }
    for &f in &declared {
        if p.func(f).generics.is_empty() {
            roots.push(g.node(f, vec![]));
        }
    }
    roots.sort();
    roots.dedup();
    let mut memo: HashMap<usize, Summary> = HashMap::new();
    for n in roots {
        let mut on_stack = Vec::new();
        let s = g.summary(n, &mut memo, &mut on_stack);
        g.report(n, &s, diags);
    }
}

impl<'a> Graph<'a> {
    fn node(&mut self, f: FnId, targs: Vec<Ty>) -> usize {
        let key = (f, targs);
        if let Some(&n) = self.ids.get(&key) {
            return n;
        }
        let n = self.insts.len();
        self.ids.insert(key.clone(), n);
        self.insts.push(key);
        self.facts.push(None);
        n
    }

    fn name(&self, n: usize) -> String {
        let (f, targs) = &self.insts[n];
        let fd = self.p.func(*f);
        let mut s = format!("{}.{}", self.p.modules[fd.module.idx()].path, fd.qualified_name(self.p));
        if !targs.is_empty() {
            let a: Vec<String> = targs.iter().map(|&t| self.p.ty_str(t)).collect();
            s = format!("{}[{}]", s, a.join(", "));
        }
        s
    }

    fn fn_name(&self, f: FnId) -> String {
        let fd = self.p.func(f);
        format!("{}.{}", self.p.modules[fd.module.idx()].path, fd.qualified_name(self.p))
    }

    /// The facts of an instance: its calls and what it does itself.
    fn scan(&mut self, n: usize) -> Vec<(Fact, Span)> {
        let (f, targs) = self.insts[n].clone();
        let Some(body) = self.bodies.get(&f) else { return vec![] };
        let tcx = &self.p.tcx;
        let mut out = Vec::new();
        for b in &body.blocks {
            for st in &b.stmts {
                match &st.kind {
                    StmtKind::Assign(_, rv) => match rv {
                        Rvalue::Closure { heap: true, .. } => out.push((Fact::Allocates("the environment of a task's function is put on the heap".into()), st.span)),
                        Rvalue::ToDyn(..) => out.push((Fact::Allocates("a value converted to `any` is put on the heap".into()), st.span)),
                        Rvalue::Aggregate(AggKind::Adt { adt, variant, .. }, _) => {
                            let a = self.p.adt(*adt);
                            let boxed = match (&a.kind, variant) {
                                (AdtKind::Struct { fields }, _) => fields.iter().any(|f| f.indirect),
                                (AdtKind::Enum { variants }, Some(v)) => variants[*v as usize].indirect,
                                _ => false,
                            };
                            if boxed {
                                out.push((Fact::Allocates(format!("an `indirect` part of `{}` is put on the heap", a.name)), st.span));
                            }
                        }
                        _ => {}
                    },
                    StmtKind::Drop(place) => {
                        let t = tcx.subst(place_ty(self.p, body, place), &targs);
                        self.drop_facts(t, st.span, &mut out);
                    }
                    StmtKind::DropIfInit(l) => {
                        let t = tcx.subst(body.locals[*l as usize].ty, &targs);
                        self.drop_facts(t, st.span, &mut out);
                    }
                    StmtKind::FreeBox(..) => out.push((Fact::Allocates("an `indirect` part is freed".into()), st.span)),
                    _ => {}
                }
            }
            if let Terminator::Call { callee, .. } = &b.term {
                let span = b.term_span;
                match callee {
                    Callee::Fn(g, gargs) => {
                        let gargs: Vec<Ty> = gargs.iter().map(|&t| tcx.subst(t, &targs)).collect();
                        self.call_facts(*g, gargs, span, &mut out);
                    }
                    Callee::Builtin(Builtin::Print | Builtin::Println | Builtin::Eprint | Builtin::Eprintln) => {
                        out.push((Fact::Blocks("writing to the console waits for it".into()), span))
                    }
                    Callee::Builtin(_) | Callee::Prim(_) => {}
                    Callee::Indirect(_) => out.push((Fact::Indirect("a call through a function value".into()), span)),
                    Callee::Runtime(rt) => match rt {
                        RtFn::StrPushStr | RtFn::StrConcat | RtFn::Fmt { .. } => out.push((Fact::Allocates("a string grows".into()), span)),
                        RtFn::Print { .. } => out.push((Fact::Blocks("writing to the console waits for it".into()), span)),
                        _ => {}
                    },
                }
            }
        }
        out
    }

    fn call_facts(&mut self, g: FnId, gargs: Vec<Ty>, span: Span, out: &mut Vec<(Fact, Span)>) {
        let (g, gargs) = if self.p.func(g).is_trait_decl() {
            match self.p.resolve_trait_method(g, &gargs) {
                Some(x) => x,
                None => {
                    out.push((Fact::Indirect(format!("`{}` is called through `any`", self.p.func(g).name)), span));
                    return;
                }
            }
        } else {
            (g, gargs)
        };
        let gd = self.p.func(g);
        // A function that never returns (a panic, `exit`) stops the program: what it does
        // on the way is not an effect of the caller.
        if matches!(&*self.p.tcx.kind(gd.ret), TyKind::Never) {
            return;
        }
        match gd.intrinsic() {
            Some("drop_in_place") => return self.drop_facts(gargs[0], span, out),
            Some("call_closure") => return out.push((Fact::Indirect("a call through the code of a function value".into()), span)),
            _ => {}
        }
        if gd.effects.allocates {
            out.push((Fact::Allocates(format!("`{}` allocates or frees memory", self.fn_name(g))), span));
        }
        if gd.effects.blocks {
            out.push((Fact::Blocks(format!("`{}` can wait", self.fn_name(g))), span));
        }
        if gd.effects.allocates || gd.effects.blocks {
            return;
        }
        if let FnKind::Extern { .. } = gd.kind {
            // A C function: only the promise written on its declaration.
            if !gd.effects.noalloc {
                out.push((Fact::Allocates(format!("the C function `{}` is not declared `with noalloc`", gd.name)), span));
            }
            if !gd.effects.nonblocking {
                out.push((Fact::Blocks(format!("the C function `{}` is not declared `with nonblocking`", gd.name)), span));
            }
            return;
        }
        if self.bodies.contains_key(&g) {
            let m = self.node(g, gargs);
            out.push((Fact::Calls(m), span));
        }
    }

    /// Destroying a value of type `t`: its destructors, and the heap parts it frees.
    fn drop_facts(&mut self, t: Ty, span: Span, out: &mut Vec<(Fact, Span)>) {
        if !self.p.needs_drop(t) {
            return;
        }
        let tcx = &self.p.tcx;
        match &*tcx.kind(t) {
            TyKind::Tuple(ts) => {
                for &e in ts.clone().iter() {
                    self.drop_facts(e, span, out);
                }
            }
            TyKind::Array(e, _) => self.drop_facts(*e, span, out),
            TyKind::Dyn(_) => {
                out.push((Fact::Allocates("a value of type `any` is freed".into()), span));
                out.push((Fact::Indirect("a value of type `any` is destroyed through its table".into()), span));
            }
            TyKind::Adt(id, args) => {
                let a = self.p.adt(*id);
                if let Some(df) = a.drop_fn {
                    self.call_facts(df, args.clone(), span, out);
                }
                match &a.kind {
                    AdtKind::Struct { fields } => {
                        for f in fields {
                            if f.indirect {
                                out.push((Fact::Allocates(format!("an `indirect` field of `{}` is freed", a.name)), span));
                            }
                            self.drop_facts(tcx.subst(f.ty, args), span, out);
                        }
                    }
                    AdtKind::Enum { variants } => {
                        for v in variants {
                            if v.indirect {
                                out.push((Fact::Allocates(format!("an `indirect` variant of `{}` is freed", a.name)), span));
                            }
                            for &ft in &v.fields {
                                self.drop_facts(tcx.subst(ft, args), span, out);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn summary(&mut self, n: usize, memo: &mut HashMap<usize, Summary>, on_stack: &mut Vec<usize>) -> Summary {
        if let Some(s) = memo.get(&n) {
            return s.clone();
        }
        if self.facts[n].is_none() {
            let fs = self.scan(n);
            self.facts[n] = Some(fs);
        }
        on_stack.push(n);
        let mut s = Summary::default();
        for (fact, span) in self.facts[n].clone().unwrap() {
            let set = |slot: &mut Option<Path>, p: Path| {
                if slot.is_none() {
                    *slot = Some(p);
                }
            };
            match fact {
                Fact::Allocates(w) => set(&mut s.alloc, Path { span, via: vec![], what: w }),
                Fact::Blocks(w) => set(&mut s.block, Path { span, via: vec![], what: w }),
                Fact::Indirect(w) => set(&mut s.indirect, Path { span, via: vec![], what: w }),
                Fact::Calls(m) => {
                    if on_stack.contains(&m) {
                        let what = if m == n { format!("`{}` calls itself", self.name(m)) } else { format!("`{}` is called again", self.name(m)) };
                        set(&mut s.recursion, Path { span, via: vec![m], what });
                        continue;
                    }
                    let c = self.summary(m, memo, on_stack);
                    let extend = |p: &Option<Path>| p.as_ref().map(|p| Path { span, via: std::iter::once(m).chain(p.via.iter().copied()).collect(), what: p.what.clone() });
                    if s.alloc.is_none() {
                        s.alloc = extend(&c.alloc);
                    }
                    if s.block.is_none() {
                        s.block = extend(&c.block);
                    }
                    if s.indirect.is_none() {
                        s.indirect = extend(&c.indirect);
                    }
                    if s.recursion.is_none() {
                        s.recursion = extend(&c.recursion);
                    }
                }
            }
        }
        on_stack.pop();
        // Inside a cycle, a summary can miss what the rest of the cycle does; it then has a
        // recursion, which is an error from wherever it is reached: it can be kept.
        memo.insert(n, s.clone());
        s
    }

    fn report(&self, n: usize, s: &Summary, diags: &DiagSink) {
        let fd = self.p.func(self.insts[n].0);
        let e = &fd.effects;
        let clause = e.span.unwrap_or(fd.span);
        let emit = |code: &'static str, kind: Kind, path: &Path| {
            let (msg, label) = match kind {
                Kind::Alloc => (format!("`{}` is declared `with noalloc` but can allocate or free memory", self.name(n)), "this can allocate or free"),
                Kind::Block => (format!("`{}` is declared `with nonblocking` but can wait", self.name(n)), "this can wait"),
                Kind::Indirect => (format!("`{}` has effects but makes a call that cannot be checked", self.name(n)), "the function called is not known"),
                Kind::Recursion => (format!("`{}` has effects but can call itself again", self.name(n)), "this call starts a recursion"),
            };
            let mut d = Diagnostic::error(code, msg).primary(path.span, label).secondary(clause, "effects declared here");
            if !path.via.is_empty() {
                let chain: Vec<String> = std::iter::once(n).chain(path.via.iter().copied()).map(|m| format!("`{}`", self.name(m))).collect();
                d = d.note(format!("calls: {}", chain.join(" → ")));
            }
            d = d.note(path.what.clone());
            if matches!(kind, Kind::Indirect | Kind::Recursion) {
                d = d.help("call the function directly, without a function value, `any` or a cycle: the effects and the stack must be known");
            }
            diags.emit(d);
        };
        if e.noalloc {
            if let Some(p) = &s.alloc {
                emit("E0601", Kind::Alloc, p);
            }
        }
        if e.nonblocking {
            if let Some(p) = &s.block {
                emit("E0602", Kind::Block, p);
            }
        }
        if let Some(p) = &s.indirect {
            emit("E0603", Kind::Indirect, p);
        } else if let Some(p) = &s.recursion {
            emit("E0603", Kind::Recursion, p);
        }
    }
}

/// The type of a place of a body (before substitution of the generic parameters).
fn place_ty(p: &Program, body: &Body, place: &Place) -> Ty {
    let tcx = &p.tcx;
    let mut t = body.locals[place.local as usize].ty;
    for pr in &place.proj {
        t = match (pr, &*tcx.kind(t)) {
            (Proj::Deref, TyKind::Ptr(e)) => *e,
            (Proj::Field(i), TyKind::Tuple(ts)) => ts[*i as usize],
            (Proj::Field(i), TyKind::Adt(id, args)) => tcx.subst(p.adt(*id).fields()[*i as usize].ty, args),
            (Proj::VariantField(v, i), TyKind::Adt(id, args)) => tcx.subst(p.adt(*id).variants()[*v as usize].fields[*i as usize], args),
            (Proj::Index(_), TyKind::Array(e, _)) => *e,
            _ => tcx.error,
        };
    }
    t
}

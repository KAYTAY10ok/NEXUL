//! Ownership analysis on the NIR (spec §5.2, §5.5):
//! - definite initialization and use-after-move (E0401, E0402), by forward dataflow;
//! - drop elaboration: every `DropIfInit` becomes nothing, an unconditional drop, or a
//!   drop guarded by a runtime drop flag.

use crate::nir::*;
use nxc_sema::defs::{ConstValue, Program};
use nxc_syntax::{DiagSink, Diagnostic, Span};

#[derive(Clone, PartialEq, Eq, Debug)]
struct BitSet {
    words: Vec<u64>,
}

impl BitSet {
    fn new(n: usize) -> BitSet {
        BitSet { words: vec![0; (n + 63) / 64] }
    }
    fn insert(&mut self, i: usize) {
        self.words[i / 64] |= 1 << (i % 64);
    }
    fn remove(&mut self, i: usize) {
        self.words[i / 64] &= !(1 << (i % 64));
    }
    fn contains(&self, i: usize) -> bool {
        self.words[i / 64] & (1 << (i % 64)) != 0
    }
    fn union(&mut self, o: &BitSet) -> bool {
        let mut changed = false;
        for (a, b) in self.words.iter_mut().zip(o.words.iter()) {
            let n = *a | *b;
            if n != *a {
                *a = n;
                changed = true;
            }
        }
        changed
    }
    fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.words.len() * 64).filter(move |&i| self.contains(i))
    }
}

/// The effect of a statement or terminator on initialization state.
#[derive(Clone, Debug)]
enum Effect {
    /// The local must be initialized here.
    Use(Local, Span),
    /// The whole local becomes initialized.
    Init(Local),
    /// The whole local becomes uninitialized (move site index if user-visible).
    Uninit(Local, Option<usize>),
}

struct MoveSite {
    local: Local,
    span: Span,
}

fn place_uses(p: &Place, span: Span, out: &mut Vec<Effect>) {
    out.push(Effect::Use(p.local, span));
    for pr in &p.proj {
        if let Proj::Index(l) = pr {
            out.push(Effect::Use(*l, span));
        }
    }
}

fn operand_effects(o: &Operand, span: Span, sites: &mut Vec<MoveSite>, out: &mut Vec<Effect>) {
    match o {
        Operand::Copy(p) => place_uses(p, span, out),
        Operand::Move(p) => {
            place_uses(p, span, out);
            if p.proj.is_empty() {
                sites.push(MoveSite { local: p.local, span });
                out.push(Effect::Uninit(p.local, Some(sites.len() - 1)));
            }
        }
        Operand::Const(..) | Operand::FnRef(_) => {}
    }
}

fn stmt_effects(s: &Statement, sites: &mut Vec<MoveSite>, out: &mut Vec<Effect>) {
    let span = s.span;
    match &s.kind {
        StmtKind::Assign(place, rv) => {
            match rv {
                Rvalue::Use(o) | Rvalue::Unary(_, o) | Rvalue::Cast(o, _) | Rvalue::Repeat(o, _) => operand_effects(o, span, sites, out),
                Rvalue::Binary(_, a, b) => {
                    operand_effects(a, span, sites, out);
                    operand_effects(b, span, sites, out);
                }
                Rvalue::Aggregate(_, ops) => ops.iter().for_each(|o| operand_effects(o, span, sites, out)),
                Rvalue::Ref(p) | Rvalue::Discriminant(p) => place_uses(p, span, out),
            }
            if place.proj.is_empty() {
                out.push(Effect::Init(place.local));
            } else {
                place_uses(place, span, out);
            }
        }
        StmtKind::Drop(p) => {
            place_uses(p, span, out);
            if p.proj.is_empty() {
                out.push(Effect::Uninit(p.local, None));
            }
        }
        StmtKind::DropIfInit(_) | StmtKind::Nop => {}
        StmtKind::MarkMoved(l) => out.push(Effect::Uninit(*l, None)),
        StmtKind::BoundsCheck { index, len, dest } => {
            operand_effects(index, span, sites, out);
            operand_effects(len, span, sites, out);
            out.push(Effect::Init(*dest));
        }
        StmtKind::FreeBox(p, _) => place_uses(p, span, out),
    }
}

fn term_effects(b: &BasicBlock, sites: &mut Vec<MoveSite>, out: &mut Vec<Effect>) {
    let span = b.term_span;
    match &b.term {
        Terminator::Switch { discr, .. } => operand_effects(discr, span, sites, out),
        Terminator::Call { callee, args, dest, .. } => {
            if let Callee::Indirect(f) = callee {
                operand_effects(f, span, sites, out);
            }
            for a in args {
                operand_effects(a, span, sites, out);
            }
            if let Some(d) = dest {
                if d.proj.is_empty() {
                    out.push(Effect::Init(d.local));
                } else {
                    place_uses(d, span, out);
                }
            }
        }
        _ => {}
    }
}

#[derive(Clone)]
struct State {
    maybe_init: BitSet,
    maybe_uninit: BitSet,
    moves: BitSet,
}

impl State {
    fn apply(&mut self, e: &Effect, sites: &[MoveSite]) {
        match e {
            Effect::Use(..) => {}
            Effect::Init(l) => {
                self.maybe_init.insert(*l as usize);
                self.maybe_uninit.remove(*l as usize);
                for (i, s) in sites.iter().enumerate() {
                    if s.local == *l {
                        self.moves.remove(i);
                    }
                }
            }
            Effect::Uninit(l, site) => {
                self.maybe_init.remove(*l as usize);
                self.maybe_uninit.insert(*l as usize);
                if let Some(s) = site {
                    self.moves.insert(*s);
                }
            }
        }
    }

    fn join(&mut self, o: &State) -> bool {
        let a = self.maybe_init.union(&o.maybe_init);
        let b = self.maybe_uninit.union(&o.maybe_uninit);
        let c = self.moves.union(&o.moves);
        a || b || c
    }
}

/// Per-block effects: one list per statement, then one for the terminator.
struct Effects {
    stmts: Vec<Vec<Vec<Effect>>>,
    terms: Vec<Vec<Effect>>,
    sites: Vec<MoveSite>,
}

fn compute_effects(body: &Body) -> Effects {
    let mut sites = Vec::new();
    let mut stmts = Vec::new();
    let mut terms = Vec::new();
    for b in &body.blocks {
        let mut per = Vec::new();
        for s in &b.stmts {
            let mut v = Vec::new();
            stmt_effects(s, &mut sites, &mut v);
            per.push(v);
        }
        stmts.push(per);
        let mut t = Vec::new();
        term_effects(b, &mut sites, &mut t);
        terms.push(t);
    }
    Effects { stmts, terms, sites }
}

fn entry_state(body: &Body, n_sites: usize) -> State {
    let n = body.locals.len();
    let mut st = State { maybe_init: BitSet::new(n), maybe_uninit: BitSet::new(n), moves: BitSet::new(n_sites.max(1)) };
    for (i, l) in body.locals.iter().enumerate() {
        match l.kind {
            LocalKind::Arg { .. } => st.maybe_init.insert(i),
            _ => st.maybe_uninit.insert(i),
        }
    }
    st
}

/// Fixpoint: the state at the entry of every reachable block.
fn solve(body: &Body, fx: &Effects) -> Vec<Option<State>> {
    let nb = body.blocks.len();
    let mut entry: Vec<Option<State>> = vec![None; nb];
    entry[0] = Some(entry_state(body, fx.sites.len()));
    let mut work: Vec<BlockId> = vec![0];
    let mut queued = vec![false; nb];
    queued[0] = true;
    while let Some(b) = work.pop() {
        queued[b as usize] = false;
        let mut st = entry[b as usize].clone().unwrap();
        for effs in &fx.stmts[b as usize] {
            for e in effs {
                st.apply(e, &fx.sites);
            }
        }
        for e in &fx.terms[b as usize] {
            st.apply(e, &fx.sites);
        }
        for s in body.successors(b) {
            let changed = match &mut entry[s as usize] {
                Some(es) => es.join(&st),
                slot @ None => {
                    *slot = Some(st.clone());
                    true
                }
            };
            if changed && !queued[s as usize] {
                queued[s as usize] = true;
                work.push(s);
            }
        }
    }
    entry
}

/// Reports reads of uninitialized or moved variables.
pub fn check(p: &Program, body: &Body, diags: &DiagSink) {
    let fx = compute_effects(body);
    let entry = solve(body, &fx);
    let mut reported = vec![false; body.locals.len()];
    for (bi, st0) in entry.iter().enumerate() {
        let Some(st0) = st0 else { continue };
        let mut st = st0.clone();
        let mut all: Vec<&Vec<Effect>> = fx.stmts[bi].iter().collect();
        all.push(&fx.terms[bi]);
        for effs in all {
            for e in effs {
                if let Effect::Use(l, span) = e {
                    let li = *l as usize;
                    let decl = &body.locals[li];
                    if st.maybe_uninit.contains(li) && !reported[li] && matches!(decl.kind, LocalKind::User | LocalKind::Arg { .. }) && decl.name.is_some() {
                        reported[li] = true;
                        report(p, body, &fx, &st, *l, *span, diags);
                    }
                }
                st.apply(e, &fx.sites);
            }
        }
    }
}

fn report(p: &Program, body: &Body, fx: &Effects, st: &State, l: Local, span: Span, diags: &DiagSink) {
    let decl = &body.locals[l as usize];
    let name = decl.name.clone().unwrap_or_default();
    let moves: Vec<&MoveSite> = st.moves.iter().filter(|&i| i < fx.sites.len()).map(|i| &fx.sites[i]).filter(|s| s.local == l).collect();
    if moves.is_empty() {
        diags.emit(
            Diagnostic::error("E0401", format!("`{}` is used before being initialized", name))
                .primary(span, "value read here")
                .secondary(decl.span, "declared here without a value")
                .help("assign a value on every path before reading it"),
        );
        return;
    }
    let ty = p.ty_str(decl.ty);
    let same_place = moves.iter().any(|m| m.span == span);
    let primary = if same_place { "value moved here, in a previous iteration of the loop" } else { "value used here after move" };
    let mut d = Diagnostic::error("E0402", format!("use of moved value `{}`", name)).primary(span, primary);
    let mut in_loop = same_place;
    for m in &moves {
        if m.span == span {
            continue;
        }
        if m.span.lo >= span.lo && m.span.file == span.file {
            in_loop = true;
            d = d.secondary(m.span, "value moved here, in a previous iteration of the loop");
        } else {
            d = d.secondary(m.span, "value moved here");
        }
    }
    d = d.note(format!("`{}` has type `{}`, which owns resources and is moved rather than copied", name, ty));
    if in_loop {
        d = d.help("assign a new value before the next iteration, or move a copy");
    } else if p.is_string(decl.ty) {
        d = d.help(format!("use `{}.clone()` where the value is moved to keep using `{}`", name, name));
    } else {
        d = d.help(format!("use `{}` before it is moved, or assign it a new value first", name));
    }
    diags.emit(d);
}

/// Replaces every `DropIfInit` by the appropriate drop.
pub fn elaborate_drops(p: &Program, body: &mut Body) {
    let fx = compute_effects(body);
    let entry = solve(body, &fx);
    #[derive(Clone, Copy, PartialEq)]
    enum Decision {
        Remove,
        Drop,
        Flag,
    }
    // Decide each DropIfInit.
    let mut decisions: Vec<Vec<Option<Decision>>> = Vec::new();
    let mut needs_flag = vec![false; body.locals.len()];
    for (bi, b) in body.blocks.iter().enumerate() {
        let mut ds = vec![None; b.stmts.len()];
        if let Some(st0) = &entry[bi] {
            let mut st = st0.clone();
            for (si, s) in b.stmts.iter().enumerate() {
                if let StmtKind::DropIfInit(l) = s.kind {
                    let li = l as usize;
                    let d = if !p.needs_drop(body.locals[li].ty) || !st.maybe_init.contains(li) {
                        Decision::Remove
                    } else if !st.maybe_uninit.contains(li) {
                        Decision::Drop
                    } else {
                        needs_flag[li] = true;
                        Decision::Flag
                    };
                    ds[si] = Some(d);
                }
                for e in &fx.stmts[bi][si] {
                    st.apply(e, &fx.sites);
                }
            }
        } else {
            for (si, s) in b.stmts.iter().enumerate() {
                if matches!(s.kind, StmtKind::DropIfInit(_)) {
                    ds[si] = Some(Decision::Remove);
                }
            }
        }
        decisions.push(ds);
    }

    // Create drop flags.
    let bool_ty = p.tcx.bool;
    let mut flag_of: Vec<Option<Local>> = vec![None; body.locals.len()];
    for (l, &nf) in needs_flag.iter().enumerate() {
        if nf {
            let f = body.locals.len() as Local;
            body.locals.push(LocalDecl { ty: bool_ty, name: None, kind: LocalKind::DropFlag, span: Span::DUMMY });
            flag_of[l] = Some(f);
        }
    }
    let set_flag = |f: Local, v: bool, span: Span| Statement {
        kind: StmtKind::Assign(Place::local(f), Rvalue::Use(Operand::Const(ConstValue::Bool(v), bool_ty))),
        span,
    };

    // Rewrite blocks. Conditional drops split a block in two.
    let nblocks = body.blocks.len();
    let mut call_inits: Vec<(BlockId, Local)> = Vec::new();
    for bi in 0..nblocks {
        let old = std::mem::replace(&mut body.blocks[bi].stmts, Vec::new());
        let mut cur_block = bi;
        let mut out: Vec<Statement> = Vec::new();
        for (si, s) in old.into_iter().enumerate() {
            let span = s.span;
            match decisions[bi][si] {
                Some(Decision::Remove) => continue,
                Some(Decision::Drop) => {
                    let StmtKind::DropIfInit(l) = s.kind else { unreachable!() };
                    out.push(Statement { kind: StmtKind::Drop(Place::local(l)), span });
                    if let Some(f) = flag_of[l as usize] {
                        out.push(set_flag(f, false, span));
                    }
                    continue;
                }
                Some(Decision::Flag) => {
                    let StmtKind::DropIfInit(l) = s.kind else { unreachable!() };
                    let f = flag_of[l as usize].unwrap();
                    // Split: [out...] switch(flag) -> drop_bb -> cont
                    let drop_bb = body.blocks.len() as BlockId;
                    body.blocks.push(BasicBlock { stmts: vec![], term: Terminator::Unreachable, term_span: span });
                    let cont = body.blocks.len() as BlockId;
                    let orig_term = std::mem::replace(&mut body.blocks[cur_block].term, Terminator::Unreachable);
                    let orig_span = body.blocks[cur_block].term_span;
                    body.blocks.push(BasicBlock { stmts: vec![], term: orig_term, term_span: orig_span });
                    body.blocks[cur_block].stmts = std::mem::take(&mut out);
                    body.blocks[cur_block].term = Terminator::Switch { discr: Operand::Copy(Place::local(f)), targets: vec![(0, cont)], otherwise: drop_bb };
                    body.blocks[cur_block].term_span = span;
                    body.blocks[drop_bb as usize].stmts = vec![Statement { kind: StmtKind::Drop(Place::local(l)), span }, set_flag(f, false, span)];
                    body.blocks[drop_bb as usize].term = Terminator::Goto(cont);
                    cur_block = cont as usize;
                    continue;
                }
                None => {}
            }
            // Flag maintenance after initializations and moves.
            let mut after: Vec<Statement> = Vec::new();
            match &s.kind {
                StmtKind::Assign(place, rv) => {
                    let mut moved: Vec<Local> = Vec::new();
                    let mut collect = |o: &Operand| {
                        if let Operand::Move(pl) = o {
                            if pl.proj.is_empty() {
                                moved.push(pl.local);
                            }
                        }
                    };
                    match rv {
                        Rvalue::Use(o) | Rvalue::Unary(_, o) | Rvalue::Cast(o, _) | Rvalue::Repeat(o, _) => collect(o),
                        Rvalue::Binary(_, a, b) => {
                            collect(a);
                            collect(b);
                        }
                        Rvalue::Aggregate(_, ops) => ops.iter().for_each(&mut collect),
                        _ => {}
                    }
                    for m in moved {
                        if let Some(f) = flag_of[m as usize] {
                            after.push(set_flag(f, false, span));
                        }
                    }
                    if place.proj.is_empty() {
                        if let Some(f) = flag_of[place.local as usize] {
                            after.push(set_flag(f, true, span));
                        }
                    }
                }
                StmtKind::MarkMoved(l) => {
                    if let Some(f) = flag_of[*l as usize] {
                        after.push(set_flag(f, false, span));
                    }
                }
                StmtKind::Drop(pl) if pl.proj.is_empty() => {
                    if let Some(f) = flag_of[pl.local as usize] {
                        after.push(set_flag(f, false, span));
                    }
                }
                _ => {}
            }
            out.push(s);
            out.extend(after);
        }
        // Moves into call arguments happen at the call; the destination is initialized
        // in the (fresh) return block.
        let span = body.blocks[cur_block].term_span;
        if let Terminator::Call { args, dest, target, callee } = &body.blocks[cur_block].term {
            let mut moved = Vec::new();
            for a in args.iter().chain(match callee {
                Callee::Indirect(f) => Some(f),
                _ => None,
            }) {
                if let Operand::Move(pl) = a {
                    if pl.proj.is_empty() {
                        moved.push(pl.local);
                    }
                }
            }
            for m in moved {
                if let Some(f) = flag_of[m as usize] {
                    out.push(set_flag(f, false, span));
                }
            }
            if let (Some(d), Some(t)) = (dest, target) {
                if d.proj.is_empty() && flag_of[d.local as usize].is_some() {
                    call_inits.push((*t, d.local));
                }
            }
        }
        body.blocks[cur_block].stmts = out;
    }
    for (t, l) in call_inits {
        let f = flag_of[l as usize].unwrap();
        body.blocks[t as usize].stmts.insert(0, set_flag(f, true, Span::DUMMY));
    }
    // Initial flag values: arguments start initialized, everything else does not.
    let mut init = Vec::new();
    for (l, f) in flag_of.iter().enumerate() {
        if let Some(f) = f {
            let v = matches!(body.locals[l].kind, LocalKind::Arg { .. });
            init.push(set_flag(*f, v, Span::DUMMY));
        }
    }
    if !init.is_empty() {
        let b0 = &mut body.blocks[0].stmts;
        let rest = std::mem::take(b0);
        *b0 = init;
        b0.extend(rest);
    }
}

//! Uses of the real-time tasks and resources in the bodies (ADR 0018 §2, §3, §5). The
//! scheduler alone calls a task, with its resources: a task is not called nor taken as a
//! value (E0605), except as the argument of `rt.overruns` (E0608 otherwise); a resource is
//! used only by its tasks and by `main`, which never runs at the same time as them (E0605).
//! Without a call to `rt.run`, the tasks never start (W0009).

use crate::nir::*;
use nxc_sema::defs::{FnId, Program};
use nxc_syntax::{DiagSink, Diagnostic, Span};
use std::collections::HashMap;

pub fn check(p: &Program, bodies: &mut HashMap<FnId, Body>, diags: &DiagSink) {
    let tasks: Vec<FnId> = p.fns.iter().enumerate().filter(|(_, f)| is_task(f)).map(|(i, _)| FnId(i as u32)).collect();
    let rt_fn = |name: &str| (0..p.fns.len() as u32).map(FnId).find(|&f| p.func(f).name == name && p.modules[p.func(f).module.idx()].path == "std.rt");
    let (run, overruns) = (rt_fn("run"), rt_fn("overruns"));
    if let Some(ov) = overruns {
        for body in bodies.values_mut() {
            name_the_task(body, ov, &tasks);
        }
    }
    let resources: Vec<bool> = p.statics.iter().map(|s| s.resource).collect();
    let mut ids: Vec<&FnId> = bodies.keys().collect();
    ids.sort();
    let mut runs = false;
    for &id in ids {
        let body = &bodies[&id];
        let in_main = p.main_fn == Some(id);
        let fn_value = |op: &Operand, span: Span, allowed: bool| {
            if let Operand::FnRef(f) = op {
                if tasks.contains(f) && !allowed {
                    diags.emit(
                        Diagnostic::error("E0605", format!("the task `{}` is run by the scheduler only", p.func(*f).name))
                            .primary(span, "a task is not taken as a value")
                            .help("put the shared code in an ordinary function"),
                    );
                }
            }
        };
        for b in &body.blocks {
            for st in &b.stmts {
                if let StmtKind::Assign(_, rv) = &st.kind {
                    match rv {
                        Rvalue::StaticAddr(s) if resources[s.idx()] && !in_main => {
                            diags.emit(
                                Diagnostic::error("E0605", format!("the resource `{}` is used only by its tasks and by `main`", p.statics[s.idx()].name))
                                    .primary(st.span, "outside a task")
                                    .help("receive it as a parameter of a task, or pass it on from the task"),
                            );
                        }
                        Rvalue::Use(op) | Rvalue::Cast(op, _) | Rvalue::FnToClosure(op) => fn_value(op, st.span, false),
                        Rvalue::Aggregate(_, ops) => ops.iter().for_each(|op| fn_value(op, st.span, false)),
                        _ => {}
                    }
                }
            }
            if let Terminator::Call { callee: Callee::Fn(f, _), args, .. } = &b.term {
                let span = b.term_span;
                if tasks.contains(f) {
                    diags.emit(
                        Diagnostic::error("E0605", format!("the task `{}` is called by the scheduler only", p.func(*f).name))
                            .primary(span, "a task is not called")
                            .help("put the shared code in an ordinary function"),
                    );
                }
                if Some(*f) == run {
                    runs = true;
                }
                let is_overruns = Some(*f) == overruns;
                if is_overruns && !matches!(args.first(), Some(Operand::FnRef(t)) if tasks.contains(t)) {
                    diags.emit(Diagnostic::error("E0608", "`rt.overruns` counts the missed deadlines of a task").primary(span, "its argument is not the name of a task"));
                }
                for a in args {
                    fn_value(a, span, is_overruns);
                }
            }
        }
    }
    if let (Some(plan), false) = (&p.rt, runs) {
        if let Some(main) = p.main_fn {
            diags.emit(
                Diagnostic::warning("W0009", format!("{} task(s) are declared but `rt.run()` is never called", plan.tasks.len()))
                    .primary(p.func(main).span, "the tasks never start")
                    .help("call `rt.run()` in `main`, after the preparation of the hardware"),
            );
        }
    }
}

fn is_task(f: &nxc_sema::defs::FnDef) -> bool {
    let periodic = matches!(&f.kind, nxc_sema::defs::FnKind::User(d) if d.period.is_some());
    periodic || f.rt.interrupt.is_some()
}

/// `rt.overruns(regulation)` is lowered as a temporary that holds the task, then the call:
/// the call receives the task itself, which the code generator needs to know.
fn name_the_task(body: &mut Body, overruns: FnId, tasks: &[FnId]) {
    // The temporaries that hold a task, and the statements that put it there.
    let mut held: HashMap<Local, (FnId, Vec<(usize, usize)>)> = HashMap::new();
    for (bi, b) in body.blocks.iter().enumerate() {
        for (si, st) in b.stmts.iter().enumerate() {
            let StmtKind::Assign(Place { local, proj }, rv) = &st.kind else { continue };
            if !proj.is_empty() {
                continue;
            }
            match rv {
                Rvalue::Use(Operand::FnRef(f)) if tasks.contains(f) => {
                    held.insert(*local, (*f, vec![(bi, si)]));
                }
                // Copied, or lent (the argument of `overruns` is borrowed).
                Rvalue::Use(Operand::Copy(from) | Operand::Move(from)) | Rvalue::Ref(from) if from.proj.is_empty() => {
                    if let Some((f, mut at)) = held.get(&from.local).cloned() {
                        at.push((bi, si));
                        held.insert(*local, (f, at));
                    }
                }
                _ => {}
            }
        }
    }
    let mut done = Vec::new();
    for b in body.blocks.iter_mut() {
        if let Terminator::Call { callee: Callee::Fn(f, _), args, .. } = &mut b.term {
            if *f == overruns {
                if let Some(Operand::Copy(pl) | Operand::Move(pl)) = args.first() {
                    if let Some((t, at)) = held.get(&pl.local).filter(|_| pl.proj.is_empty()) {
                        args[0] = Operand::FnRef(*t);
                        done.extend(at.iter().copied());
                    }
                }
            }
        }
    }
    for (bi, si) in done {
        body.blocks[bi].stmts[si].kind = StmtKind::Nop;
    }
}

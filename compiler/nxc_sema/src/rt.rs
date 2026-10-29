//! Real-time tasks (ADR 0018 §2 to §6): periodic tasks (`every … within …`), hardware
//! interrupts (`@interrupt`), resources (`@resource`) and their ceilings, the tick of
//! SysTick and the interrupt lines of the dispatchers. The result, `Program::rt`, is what
//! the code generator schedules; the uses of tasks and resources in the bodies are checked
//! on the NIR (`nxc_nir::effects`).

use crate::defs::*;
use crate::target::{Arch, Os};
use crate::ty::TyKind;
use nxc_syntax::ast::{self, Convention, ExprKind, Lit, ParamKind};
use nxc_syntax::{DiagSink, Diagnostic, Span};

/// Levels of priority available to the tasks: 3 bits of priority, SysTick above them.
pub const LEVELS: u8 = 7;
/// Interrupt lines of the vector table (`IRQ0_Handler` to `IRQ31_Handler`).
pub const LINES: u32 = 32;

pub fn plan(prog: &mut Program, diags: &DiagSink) {
    let bare_arm = prog.cfg.target.arch == Arch::Arm && prog.cfg.target.os == Os::None;
    let mut tasks: Vec<(RtTask, Span, Option<u64>)> = Vec::new();
    let mut overrun = None;
    let mut ok = true;
    for i in 0..prog.fns.len() {
        let id = FnId(i as u32);
        let fd = &prog.fns[i];
        let FnKind::User(decl) = &fd.kind else { continue };
        let decl = decl.clone();
        let rt = fd.rt.clone();
        if let Some(sp) = rt.overrun {
            if overrun.is_some() {
                diags.emit(Diagnostic::error("E0606", "a program has one `@overrun` function").primary(sp, "second `@overrun`"));
            } else if !fd.effects.noalloc || !fd.effects.nonblocking || !fd.params.is_empty() {
                diags.emit(
                    Diagnostic::error("E0604", "the `@overrun` function has no parameter and is declared `with noalloc, nonblocking`")
                        .primary(fd.span, "called after each missed deadline, from the scheduler"),
                );
            } else {
                overrun = Some(id);
            }
        }
        if prog.main_fn != Some(id) {
            for (attr, what) in [(&rt.clock, "@clock"), (&rt.stack, "@stack")] {
                if let Some((_, sp)) = attr {
                    diags.emit(Diagnostic::error("E0604", format!("`{}` is written on `main`", what)).primary(*sp, "not on `main`"));
                }
            }
        }
        let periodic = decl.period.is_some();
        if !periodic && rt.interrupt.is_none() {
            if let Some((_, sp)) = rt.priority {
                diags.emit(Diagnostic::error("E0604", "`@priority` is written on a task or an interrupt").primary(sp, "this function is neither"));
            }
            continue;
        }
        let head = decl.period.as_ref().map(|p| p.span).or(rt.interrupt.map(|(_, s)| s)).unwrap_or(fd.span);
        let mut err = |d: Diagnostic| {
            diags.emit(d);
            ok = false;
        };
        if !bare_arm {
            err(Diagnostic::error("E0604", format!("real-time tasks need a Cortex-M target without system, not `{}`", prog.cfg.target.name))
                .primary(head, "a task or an interrupt")
                .note("compile with `--target thumbv7em-none` (ADR 0018)"));
            continue;
        }
        if periodic && rt.interrupt.is_some() {
            err(Diagnostic::error("E0604", "a function is a periodic task or an interrupt, not both").primary(head, "here"));
            continue;
        }
        if !fd.effects.noalloc || !fd.effects.nonblocking {
            err(Diagnostic::error("E0604", format!("the task `{}` must be declared `with noalloc, nonblocking`", fd.name))
                .primary(fd.span, "missing effects")
                .help("add `with noalloc, nonblocking` after the parameters: the compiler checks them"));
        }
        if !matches!(&*prog.tcx.kind(fd.ret), TyKind::Unit) || fd.owner.is_some() || !fd.generics.is_empty() {
            err(Diagnostic::error("E0604", format!("the task `{}` is a free function without result or generic parameter", fd.name)).primary(fd.span, "here"));
        }
        // The parameters: resources of the same name and type.
        let mut resources = Vec::new();
        for (k, p) in decl.params.iter().enumerate() {
            let ParamKind::Named { name, .. } = &p.kind else {
                err(Diagnostic::error("E0605", "a task has no `self`").primary(p.span, "here"));
                continue;
            };
            let ty = fd.params.get(k).map(|pd| pd.ty).unwrap_or(prog.tcx.error);
            let found = match prog.modules[fd.module.idx()].items.get(&name.name) {
                Some((ItemRef::Static(st), _, _)) => Some(*st),
                _ => None,
            };
            match found {
                Some(st) if prog.statics[st.idx()].resource && prog.statics[st.idx()].ty == ty && p.conv != Convention::Owned => {
                    resources.push((st, p.conv == Convention::Inout));
                }
                Some(st) if prog.statics[st.idx()].resource && p.conv == Convention::Owned => {
                    err(Diagnostic::error("E0605", format!("the resource `{}` is lent to the task, not given", name.name)).primary(p.span, "remove `owned`"));
                }
                Some(st) if prog.statics[st.idx()].resource => {
                    err(Diagnostic::error("E0605", format!("the parameter `{}` does not have the type of the resource `{}`", name.name, name.name))
                        .primary(p.span, format!("the resource is a `{}`", prog.ty_str(prog.statics[st.idx()].ty))));
                }
                _ => {
                    err(Diagnostic::error("E0605", format!("the parameter `{}` of a task is not a resource", name.name))
                        .primary(p.span, "no `@resource static var` of this name in the module")
                        .help(format!("declare it: `@resource static var {} {} = …`", prog.ty_str(ty), name.name)));
                }
            }
        }
        let priority = match rt.priority {
            Some((v, sp)) if v == 0 || v > LEVELS as u128 => {
                err(Diagnostic::error("E0604", format!("a priority is between 1 (lowest) and {}", LEVELS)).primary(sp, "out of range"));
                None
            }
            Some((v, _)) => Some(v as u8),
            None => None,
        };
        if let Some((irq, sp)) = rt.interrupt {
            if irq >= LINES as u128 {
                err(Diagnostic::error("E0604", format!("an interrupt line is between 0 and {}", LINES - 1)).primary(sp, "out of range"));
                continue;
            }
            let Some(prio) = priority else {
                if rt.priority.is_none() {
                    err(Diagnostic::error("E0604", format!("the interrupt `{}` needs a `@priority`", fd.name)).primary(sp, "add `@priority(n)`, 1 (lowest) to 7"));
                }
                continue;
            };
            let t = RtTask { f: id, irq: Some(irq as u32), period_ticks: 0, deadline: 0, priority: prio, run_level: prio, line: irq as u32, resources };
            tasks.push((t, head, None));
            continue;
        }
        let period = decl.period.as_ref().unwrap();
        let every = duration(prog, fd.module, &period.every, diags);
        let within = match &period.within {
            Some(e) => duration(prog, fd.module, e, diags),
            None => every,
        };
        let (Some(every), Some(within)) = (every, within) else {
            ok = false;
            continue;
        };
        if every == 0 || within == 0 || within > every {
            err(Diagnostic::error("E0604", "the deadline (`within`) is positive and at most the period (`every`)").primary(period.span, "here"));
            continue;
        }
        let t = RtTask { f: id, irq: None, period_ticks: 0, deadline: within, priority: priority.unwrap_or(0), run_level: 0, line: 0, resources };
        tasks.push((t, head, Some(every)));
    }
    if tasks.is_empty() || !ok {
        return;
    }
    let Some(main) = prog.main_fn else { return };
    let main_rt = prog.func(main).rt.clone();
    let Some((hz, _)) = main_rt.clock.filter(|(v, _)| *v > 0 && *v < u64::MAX as u128) else {
        diags.emit(
            Diagnostic::error("E0604", "a program with tasks gives the clock frequency of the core on `main`: `@clock(hz)`")
                .primary(prog.func(main).span, "add `@clock(25_000_000)` for example")
                .note("periods and deadlines are converted to cycles of this clock"),
        );
        return;
    };
    let hz = hz as u64;
    let stack = match main_rt.stack {
        Some((v, sp)) if v < 1024 || v % 32 != 0 || v > 1 << 30 => {
            diags.emit(Diagnostic::error("E0604", "the reserve of the stack is a multiple of 32 bytes, at least 1024").primary(sp, "here"));
            return;
        }
        Some((v, _)) => v as u64,
        None => 8192,
    };
    // Periods and deadlines in cycles.
    let cycles = |ns: u64| -> Option<u64> {
        let c = ns as u128 * hz as u128;
        if c % 1_000_000_000 != 0 {
            None
        } else {
            Some((c / 1_000_000_000) as u64)
        }
    };
    let mut periods = Vec::new();
    for (t, sp, every) in tasks.iter_mut() {
        let Some(ns) = every else { continue };
        match (cycles(*ns), cycles(t.deadline)) {
            (Some(p), Some(d)) => {
                periods.push(p);
                t.deadline = d;
                *ns = p;
            }
            _ => {
                diags.emit(Diagnostic::error("E0606", "this duration is not a whole number of cycles of the clock").primary(*sp, format!("with `@clock({})`", hz)));
                return;
            }
        }
    }
    let mut tick = 0u64;
    if !periods.is_empty() {
        tick = periods.iter().copied().fold(0, gcd);
        // The largest divisor that fits the 24 bits of SysTick.
        if tick > 1 << 24 {
            tick = (1..=1u64 << 24).rev().find(|d| tick % d == 0).unwrap_or(1);
        }
        if tick < 1000 {
            diags.emit(
                Diagnostic::error("E0606", "the periods do not give a tick of SysTick of at least 1000 cycles")
                    .primary(tasks.iter().find(|t| t.2.is_some()).unwrap().1, "with these periods")
                    .note(format!("the tick is the greatest common divisor of the periods: {} cycles", tick)),
            );
            return;
        }
    }
    // Rate monotonic: the shortest period gets the highest level.
    let mut distinct: Vec<u64> = tasks.iter().filter_map(|t| t.2).collect();
    distinct.sort();
    distinct.dedup();
    if distinct.len() > LEVELS as usize {
        diags.emit(Diagnostic::error("E0606", format!("more than {} different periods: not enough levels of priority", LEVELS)).primary(tasks[0].1, "give some tasks the same period or a `@priority`"));
        return;
    }
    for (t, _, every) in tasks.iter_mut() {
        if let Some(p) = every {
            t.period_ticks = (*p / tick) as u32;
            if t.priority == 0 {
                t.priority = LEVELS - distinct.iter().position(|d| d == p).unwrap() as u8;
            }
        }
    }
    // Ceilings: the highest priority of the tasks that receive a resource.
    let mut ceiling: std::collections::HashMap<StaticId, u8> = std::collections::HashMap::new();
    for (t, _, _) in &tasks {
        for (st, _) in &t.resources {
            let c = ceiling.entry(*st).or_insert(0);
            *c = (*c).max(t.priority);
        }
    }
    for (t, _, _) in tasks.iter_mut() {
        t.run_level = t.resources.iter().map(|(st, _)| ceiling[st]).fold(t.priority, u8::max);
    }
    // Interrupt lines: those of the hardware interrupts, those exported by the program,
    // then one dispatcher per level of periodic tasks, from the last line down.
    let mut used: Vec<u32> = Vec::new();
    for (t, sp, _) in &tasks {
        if let Some(irq) = t.irq {
            if used.contains(&irq) {
                diags.emit(Diagnostic::error("E0606", format!("the interrupt line {} has two handlers", irq)).primary(*sp, "second handler"));
                return;
            }
            used.push(irq);
        }
    }
    for f in &prog.fns {
        if let Some(sym) = &f.export {
            if sym == "SysTick_Handler" && tick > 0 {
                diags.emit(Diagnostic::error("E0606", "`SysTick_Handler` is the scheduler of the periodic tasks").primary(f.span, "remove this `@export`"));
                return;
            }
            if let Some(n) = sym.strip_prefix("IRQ").and_then(|r| r.strip_suffix("_Handler")).and_then(|n| n.parse::<u32>().ok()) {
                if used.contains(&n) {
                    diags.emit(Diagnostic::error("E0606", format!("the interrupt line {} has two handlers", n)).primary(f.span, "this `@export` and an `@interrupt`"));
                    return;
                }
                used.push(n);
            }
        }
    }
    let mut levels: Vec<u8> = tasks.iter().filter(|t| t.2.is_some()).map(|t| t.0.priority).collect();
    levels.sort_by(|a, b| b.cmp(a));
    levels.dedup();
    let mut free = (0..LINES).rev().filter(|l| !used.contains(l));
    let mut line_of = std::collections::HashMap::new();
    for l in levels {
        let Some(line) = free.next() else {
            diags.emit(Diagnostic::error("E0606", "no interrupt line is left for the scheduler").primary(tasks[0].1, "too many interrupts and levels"));
            return;
        };
        line_of.insert(l, line);
    }
    for (t, _, every) in tasks.iter_mut() {
        if every.is_some() {
            t.line = line_of[&t.priority];
        }
    }
    prog.rt = Some(RtPlan { clock_hz: hz, stack, tick: tick as u32, tasks: tasks.into_iter().map(|t| t.0).collect(), overrun });
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// `ms(10)`, `rt.us(400)`, `std.rt.s(1)`: a constant duration, in nanoseconds.
fn duration(prog: &Program, module: ModId, e: &ast::Expr, diags: &DiagSink) -> Option<u64> {
    let bad = || {
        diags.emit(
            Diagnostic::error("E0604", "a period or a deadline is a call of `ms`, `us` or `s` of `std.rt` with a whole number")
                .primary(e.span, "not a constant duration")
                .help("write `ms(10)` or `us(400)`, with `import std.rt.{ms, us}`"),
        );
        None
    };
    let ExprKind::Call(callee, args) = &e.kind else { return bad() };
    let m = &prog.modules[module.idx()];
    let rt_module = |id: ModId| prog.modules[id.idx()].path == "std.rt";
    let unit = match &callee.kind {
        ExprKind::Ident(n) => match m.imports.get(n) {
            Some(ImportEntry { item: ItemRef::Fn(f), used, .. }) if rt_module(prog.func(*f).module) => {
                used.set(true);
                n.clone()
            }
            _ => return bad(),
        },
        ExprKind::Field(base, name) => {
            let target = match &base.kind {
                ExprKind::Ident(b) => m.imports.get(b).and_then(|i| {
                    i.used.set(true);
                    match i.item {
                        ItemRef::Module(id) => Some(id),
                        _ => None,
                    }
                }),
                _ => None,
            };
            match target {
                Some(id) if rt_module(id) => name.name.clone(),
                _ => return bad(),
            }
        }
        _ => return bad(),
    };
    let scale: u64 = match unit.as_str() {
        "ms" => 1_000_000,
        "us" => 1_000,
        "s" => 1_000_000_000,
        _ => return bad(),
    };
    match args.as_slice() {
        [a] if a.label.is_none() => match &a.expr.kind {
            ExprKind::Lit(Lit::Int(v)) if *v <= (u64::MAX / scale) as u128 => Some(*v as u64 * scale),
            _ => bad(),
        },
        _ => bad(),
    }
}

//! The scheduler of the real-time tasks on Cortex-M (ADR 0018 §2 to §5), generated from
//! `Program::rt`: no kernel, the NVIC schedules.
//!
//! - SysTick (highest priority) counts the ticks and releases the periodic tasks: it notes
//!   the instant of release and makes the line of the dispatcher of their level pending.
//! - One dispatcher per level runs the released tasks of its level, in order.
//! - A trampoline per task passes it its resources (the statics) and runs it at its
//!   ceiling (`BASEPRI`, ADR 0018 §3).
//! - A release while the previous instance is not finished, or an end after the deadline,
//!   counts an overrun and calls the `@overrun` function.
//!
//! `rt.run()` programs the priorities, enables the lines and SysTick, then waits (`wfi`)
//! until a task calls `rt.stop()`; it then disables them and returns to `main`.

use crate::{PKind, Gen};
use nxc_sema::defs::{FnId, RtPlan};
use nxc_sema::rt::LEVELS;
use std::fmt::Write;

const NVIC_ISER: u32 = 0xE000_E100;
const NVIC_ICER: u32 = 0xE000_E180;
const NVIC_ISPR: u32 = 0xE000_E200;
const NVIC_ICPR: u32 = 0xE000_E280;
const NVIC_IPR: u32 = 0xE000_E400;
const SHPR3_SYSTICK: u32 = 0xE000_ED23;
const ICSR: u32 = 0xE000_ED04;
const SYST_CSR: u32 = 0xE000_E010;
const SYST_RVR: u32 = 0xE000_E014;
const SYST_CVR: u32 = 0xE000_E018;
const SHCSR: u32 = 0xE000_ED24;
const MPU_CTRL: u32 = 0xE000_ED94;
const MPU_RNR: u32 = 0xE000_ED98;
const MPU_RBAR: u32 = 0xE000_ED9C;
const MPU_RASR: u32 = 0xE000_EDA0;

/// Bytes guarded by the MPU under the reserve of the stack (ADR 0018 §6).
pub const GUARD: u64 = 32;

/// The value of the priority registers for a level (1 lowest .. 7): 3 bits, in the high
/// bits of the byte; 0 is SysTick.
fn prio(level: u8) -> u32 {
    ((LEVELS + 1 - level) as u32) << 5
}

impl<'a> Gen<'a> {
    /// The scheduler, if the program has tasks; returns the handlers of the vector table it
    /// replaces (`SysTick_Handler`, `IRQn_Handler` → generated function).
    pub(crate) fn emit_rt(&mut self) -> Vec<(String, String)> {
        let Some(plan) = self.p.rt.clone() else { return vec![] };
        let n = plan.tasks.len();
        let mut handlers = Vec::new();
        let mut g = String::new();
        let _ = writeln!(g, "@nx_rt_pend = internal global [{n} x i8] zeroinitializer\n@nx_rt_busy = internal global [{n} x i8] zeroinitializer\n@nx_rt_cnt = internal global [{n} x i32] zeroinitializer\n@nx_rt_rel = internal global [{n} x i64] zeroinitializer\n@nx_rt_over = internal global [{n} x i32] zeroinitializer\n@nx_rt_ticks = internal global i64 0\n@nx_rt_stopped = internal global i8 0\n@nx_rt_used = internal global i32 0");
        self.globals.push_str(&g);
        let mut f = String::new();
        // Trampolines.
        for (i, t) in plan.tasks.iter().enumerate() {
            let name = self.instance(t.f, vec![]);
            let sig = self.fn_sig(t.f, &[]);
            let mut body = String::new();
            let mut args = Vec::new();
            for (k, (st, _)) in t.resources.iter().enumerate() {
                let gl = self.static_global(*st);
                match &sig.params[k] {
                    PKind::Ptr => args.push(format!("ptr {}", gl)),
                    PKind::Scalar(ty) if ty == "i1" => {
                        let _ = writeln!(body, "  %m{k} = load volatile i8, ptr {gl}\n  %a{k} = trunc i8 %m{k} to i1");
                        args.push(format!("i1 %a{k}"));
                    }
                    PKind::Scalar(ty) => {
                        let _ = writeln!(body, "  %a{k} = load volatile {ty}, ptr {gl}");
                        args.push(format!("{ty} %a{k}"));
                    }
                }
            }
            let raise = t.run_level > t.priority;
            let _ = writeln!(f, "define internal void @nx_rt_task_{i}() nounwind {{\nstart:");
            if raise {
                let _ = writeln!(f, "  %old = call i32 asm sideeffect \"mrs $0, basepri\", \"=r,~{{memory}}\"()\n  call void asm sideeffect \"msr basepri_max, $0\", \"r,~{{memory}}\"(i32 {})", prio(t.run_level));
            }
            f.push_str(&body);
            let _ = writeln!(f, "  call void {}({})", name, args.join(", "));
            if raise {
                let _ = writeln!(f, "  call void asm sideeffect \"msr basepri, $0\", \"r,~{{memory}}\"(i32 %old)");
            }
            let _ = writeln!(f, "  ret void\n}}\n");
            if let Some(irq) = t.irq {
                handlers.push((format!("IRQ{}_Handler", irq), format!("nx_rt_task_{}", i)));
            }
        }
        // An overrun: count it, call the `@overrun` function.
        let hook = plan.overrun.map(|h| self.instance(h, vec![]));
        let _ = writeln!(f, "define internal void @nx_rt_overrun(i32 %i) nounwind {{\nstart:\n  %p = getelementptr inbounds [{n} x i32], ptr @nx_rt_over, i32 0, i32 %i\n  %c = load volatile i32, ptr %p\n  %c1 = add i32 %c, 1\n  store volatile i32 %c1, ptr %p");
        if let Some(h) = &hook {
            let _ = writeln!(f, "  call void {}()", h);
        }
        let _ = writeln!(f, "  ret void\n}}\n");
        self.emit_rt_now(&plan, &mut f);
        if plan.tick > 0 {
            self.emit_rt_systick(&plan, &mut f);
            handlers.push(("SysTick_Handler".into(), "nx_rt_systick".into()));
            let mut levels: Vec<(u8, u32)> = plan.tasks.iter().filter(|t| t.irq.is_none()).map(|t| (t.priority, t.line)).collect();
            levels.sort();
            levels.dedup();
            for (level, line) in levels {
                let _ = writeln!(f, "define internal void @nx_rt_dispatch_{level}() nounwind {{\nstart:\n  br label %t0");
                let mine: Vec<usize> = (0..n).filter(|&i| plan.tasks[i].irq.is_none() && plan.tasks[i].priority == level).collect();
                for (k, &i) in mine.iter().enumerate() {
                    let next = format!("t{}", k + 1);
                    let d = plan.tasks[i].deadline;
                    let _ = writeln!(
                        f,
                        "t{k}:\n  %pp{k} = getelementptr inbounds [{n} x i8], ptr @nx_rt_pend, i32 0, i32 {i}\n  %p{k} = load volatile i8, ptr %pp{k}\n  %go{k} = icmp ne i8 %p{k}, 0\n  br i1 %go{k}, label %run{k}, label %{next}\nrun{k}:\n  %bp{k} = getelementptr inbounds [{n} x i8], ptr @nx_rt_busy, i32 0, i32 {i}\n  store volatile i8 1, ptr %bp{k}\n  store volatile i8 0, ptr %pp{k}\n  call void @nx_rt_task_{i}()\n  store volatile i8 0, ptr %bp{k}\n  %now{k} = call i64 @nx_rt_now()\n  %rp{k} = getelementptr inbounds [{n} x i64], ptr @nx_rt_rel, i32 0, i32 {i}\n  %rel{k} = load volatile i64, ptr %rp{k}\n  %el{k} = sub i64 %now{k}, %rel{k}\n  %late{k} = icmp ugt i64 %el{k}, {d}\n  br i1 %late{k}, label %over{k}, label %{next}\nover{k}:\n  call void @nx_rt_overrun(i32 {i})\n  br label %{next}"
                    );
                }
                let _ = writeln!(f, "t{}:\n  ret void\n}}\n", mine.len());
                handlers.push((format!("IRQ{}_Handler", line), format!("nx_rt_dispatch_{}", level)));
            }
        }
        self.emit_rt_run(&plan, &mut f);
        self.emit_rt_guard(&plan, &mut f);
        // What the driver bounds on the executable (ADR 0018 §6).
        let mut levels: Vec<(u8, Vec<String>)> = Vec::new();
        for (i, t) in plan.tasks.iter().enumerate() {
            let root = match t.irq {
                Some(_) => format!("nx_rt_task_{}", i),
                None => format!("nx_rt_dispatch_{}", t.priority),
            };
            match levels.iter_mut().find(|(l, _)| *l == t.priority) {
                Some((_, roots)) if !roots.contains(&root) => roots.push(root),
                Some(_) => {}
                None => levels.push((t.priority, vec![root])),
            }
        }
        levels.sort();
        let mut chain = self.entry_chain.clone();
        chain.push("nx_rt_run".into());
        let fp = self.opts.target.features.contains("vfp");
        self.stack = Some(crate::StackSpec {
            reserve: plan.stack,
            // 26 words with the floating-point context, 8 without, plus the alignment.
            exception: if fp { 112 } else { 40 },
            chain,
            levels,
            systick: (plan.tick > 0).then(|| "nx_rt_systick".to_string()),
            stops: Vec::new(),
        });
        for (vector, thunk) in [("MemManage_Handler", "nx_rt_memmanage"), ("HardFault_Handler", "nx_rt_hardfault"), ("BusFault_Handler", "nx_rt_hardfault"), ("UsageFault_Handler", "nx_rt_hardfault")] {
            handlers.push((vector.into(), thunk.into()));
        }
        self.fn_defs.push_str(&f);
        handlers
    }

    /// The guard of the stack (a region of the MPU without access under the reserve), and
    /// the faults: a safe stop, on a stack pointer reset to the top (the overflowed stack
    /// cannot hold the frame of the handler).
    fn emit_rt_guard(&mut self, plan: &RtPlan, f: &mut String) {
        let below = plan.stack + GUARD;
        // XN, no access, 32 bytes (SIZE = 4), enabled.
        let rasr = 0x1000_0009u32;
        let _ = writeln!(
            f,
            "define internal void @nx_rt_guard() nounwind {{
start:
  %top = ptrtoint ptr @__stack_top to i32
  %base = sub i32 %top, {below}
  store volatile i32 0, ptr inttoptr (i32 {MPU_RNR} to ptr)
  store volatile i32 %base, ptr inttoptr (i32 {MPU_RBAR} to ptr)
  store volatile i32 {rasr}, ptr inttoptr (i32 {MPU_RASR} to ptr)
  store volatile i32 5, ptr inttoptr (i32 {MPU_CTRL} to ptr)
  %sh = load volatile i32, ptr inttoptr (i32 {SHCSR} to ptr)
  %sh1 = or i32 %sh, 65536
  store volatile i32 %sh1, ptr inttoptr (i32 {SHCSR} to ptr)
  call void asm sideeffect \"dsb\0Aisb\", \"~{{memory}}\"()
  ret void
}}
"
        );
        let (fault, _) = self.lang_fn("rt_fault");
        let _ = writeln!(f, "define void @nx_rt_fault(i32 %k) nounwind noreturn {{\nstart:\n  call void {}(i32 %k)\n  unreachable\n}}\n", fault);
        for (name, kind) in [("nx_rt_memmanage", 1), ("nx_rt_hardfault", 2)] {
            self.declare(&format!("declare void @{}()", name));
            self.module_asm.push(format!(
                ".section .text.{name},\"ax\",%progbits\n.globl {name}\n.type {name},%function\n.thumb_func\n{name}:\n  cpsid i\n  ldr r0, ={top}\n  mov sp, r0\n  movs r0, #{kind}\n  bl nx_rt_fault\n  b .\n.ltorg\n.text\n",
                top = "__stack_top"
            ));
        }
    }

    /// `@nx_rt_now`: cycles since `rt.run`, from the ticks and the current value of SysTick
    /// (consistent if SysTick interrupts the reading, or is pending under a mask).
    fn emit_rt_now(&mut self, plan: &RtPlan, f: &mut String) {
        if plan.tick == 0 {
            let _ = writeln!(f, "define internal i64 @nx_rt_now() nounwind {{\nstart:\n  ret i64 0\n}}\n");
            return;
        }
        let tick = plan.tick as u64;
        let _ = writeln!(
            f,
            "define internal i64 @nx_rt_now() nounwind {{
start:
  br label %again
again:
  %t1 = load volatile i64, ptr @nx_rt_ticks
  %cv = load volatile i32, ptr inttoptr (i32 {SYST_CVR} to ptr)
  %icsr = load volatile i32, ptr inttoptr (i32 {ICSR} to ptr)
  %t2 = load volatile i64, ptr @nx_rt_ticks
  %same = icmp eq i64 %t1, %t2
  br i1 %same, label %done, label %again
done:
  %pend = and i32 %icsr, 67108864
  %wrapped = icmp ne i32 %pend, 0
  %t1b = add i64 %t1, 1
  %t = select i1 %wrapped, i64 %t1b, i64 %t1
  %base = mul i64 %t, {tick}
  %cv64 = zext i32 %cv to i64
  %into = sub i64 {last}, %cv64
  %now = add i64 %base, %into
  ret i64 %now
}}
",
            last = tick - 1
        );
    }

    fn emit_rt_systick(&mut self, plan: &RtPlan, f: &mut String) {
        let n = plan.tasks.len();
        let tick = plan.tick as u64;
        let _ = writeln!(f, "define internal void @nx_rt_systick() nounwind {{\nstart:\n  %t = load volatile i64, ptr @nx_rt_ticks\n  %t1 = add i64 %t, 1\n  store volatile i64 %t1, ptr @nx_rt_ticks\n  %now = mul i64 %t1, {tick}\n  br label %s0");
        let periodic: Vec<usize> = (0..n).filter(|&i| plan.tasks[i].irq.is_none()).collect();
        for (k, &i) in periodic.iter().enumerate() {
            let t = &plan.tasks[i];
            let next = format!("s{}", k + 1);
            let bit = 1u32 << (t.line % 32);
            let ispr = NVIC_ISPR + 4 * (t.line / 32);
            let _ = writeln!(
                f,
                "s{k}:\n  %cp{k} = getelementptr inbounds [{n} x i32], ptr @nx_rt_cnt, i32 0, i32 {i}\n  %c{k} = load volatile i32, ptr %cp{k}\n  %c1{k} = sub i32 %c{k}, 1\n  %z{k} = icmp eq i32 %c1{k}, 0\n  br i1 %z{k}, label %rel{k}, label %keep{k}\nkeep{k}:\n  store volatile i32 %c1{k}, ptr %cp{k}\n  br label %{next}\nrel{k}:\n  store volatile i32 {period}, ptr %cp{k}\n  %bp{k} = getelementptr inbounds [{n} x i8], ptr @nx_rt_busy, i32 0, i32 {i}\n  %b{k} = load volatile i8, ptr %bp{k}\n  %pp{k} = getelementptr inbounds [{n} x i8], ptr @nx_rt_pend, i32 0, i32 {i}\n  %p{k} = load volatile i8, ptr %pp{k}\n  %bp2{k} = or i8 %b{k}, %p{k}\n  %late{k} = icmp ne i8 %bp2{k}, 0\n  br i1 %late{k}, label %miss{k}, label %go{k}\nmiss{k}:\n  call void @nx_rt_overrun(i32 {i})\n  br label %{next}\ngo{k}:\n  %rp{k} = getelementptr inbounds [{n} x i64], ptr @nx_rt_rel, i32 0, i32 {i}\n  store volatile i64 %now, ptr %rp{k}\n  store volatile i8 1, ptr %pp{k}\n  store volatile i32 {bit}, ptr inttoptr (i32 {ispr} to ptr)\n  br label %{next}",
                period = t.period_ticks
            );
        }
        let _ = writeln!(f, "s{}:\n  ret void\n}}\n", periodic.len());
    }

    /// `@nx_rt_run`: priorities, lines, SysTick; wait for `rt.stop`; then disable.
    fn emit_rt_run(&mut self, plan: &RtPlan, f: &mut String) {
        let n = plan.tasks.len();
        let mut lines: Vec<(u32, u8)> = plan.tasks.iter().map(|t| (t.line, t.priority)).collect();
        lines.sort();
        lines.dedup();
        let mut mask = [0u32; 2];
        let mut setup = String::new();
        for (line, level) in &lines {
            mask[(*line / 32) as usize] |= 1 << (line % 32);
            let _ = writeln!(setup, "  store volatile i8 {}, ptr inttoptr (i32 {} to ptr)", prio(*level), NVIC_IPR + line);
        }
        for i in 0..n {
            let t = &plan.tasks[i];
            let _ = writeln!(setup, "  store volatile i32 {}, ptr getelementptr inbounds ([{n} x i32], ptr @nx_rt_cnt, i32 0, i32 {i})", if t.irq.is_none() { 1 } else { 0 });
        }
        let mut enable = String::new();
        let mut disable = String::new();
        for (w, m) in mask.iter().enumerate() {
            if *m != 0 {
                let _ = writeln!(enable, "  store volatile i32 {}, ptr inttoptr (i32 {} to ptr)", m, NVIC_ICPR + 4 * w as u32);
                let _ = writeln!(enable, "  store volatile i32 {}, ptr inttoptr (i32 {} to ptr)", m, NVIC_ISER + 4 * w as u32);
                let _ = writeln!(disable, "  store volatile i32 {}, ptr inttoptr (i32 {} to ptr)", m, NVIC_ICER + 4 * w as u32);
                let _ = writeln!(disable, "  store volatile i32 {}, ptr inttoptr (i32 {} to ptr)", m, NVIC_ICPR + 4 * w as u32);
            }
        }
        if plan.tick > 0 {
            let _ = writeln!(enable, "  store volatile i8 0, ptr inttoptr (i32 {SHPR3_SYSTICK} to ptr)\n  store volatile i32 {}, ptr inttoptr (i32 {SYST_RVR} to ptr)\n  store volatile i32 0, ptr inttoptr (i32 {SYST_CVR} to ptr)\n  store volatile i32 7, ptr inttoptr (i32 {SYST_CSR} to ptr)", plan.tick - 1);
            let _ = writeln!(disable, "  store volatile i32 0, ptr inttoptr (i32 {SYST_CSR} to ptr)");
        }
        // The free part of the stack is painted, then the deepest word touched while the
        // tasks ran is found: `rt.stack_used()`, an observed maximum (ADR 0018 §6).
        let below = plan.stack;
        let paint = format!(
            "  %top = ptrtoint ptr @__stack_top to i32
  %bottom = sub i32 %top, {below}
  %sp = call i32 asm sideeffect \"mov $0, sp\", \"=r\"()
  %limit = sub i32 %sp, 64
  br label %paint
paint:
  %pa = phi i32 [ %bottom, %start ], [ %pa1, %paint ]
  %pp = inttoptr i32 %pa to ptr
  store volatile i32 2779096485, ptr %pp
  %pa1 = add i32 %pa, 4
  %pmore = icmp ult i32 %pa1, %limit
  br i1 %pmore, label %paint, label %painted
painted:
"
        );
        let measure = "  br label %scan
scan:
  %sa = phi i32 [ %bottom, %end ], [ %sa1, %scan ]
  %sp2 = inttoptr i32 %sa to ptr
  %w = load volatile i32, ptr %sp2
  %same = icmp eq i32 %w, 2779096485
  %sa1 = add i32 %sa, 4
  %smore = icmp ult i32 %sa1, %limit
  %go = and i1 %same, %smore
  br i1 %go, label %scan, label %scanned
scanned:
  %used = sub i32 %top, %sa
  store volatile i32 %used, ptr @nx_rt_used
".to_string();
        let _ = writeln!(
            f,
            "define internal void @nx_rt_run() nounwind {{
start:
  store volatile i8 0, ptr @nx_rt_stopped
  store volatile i64 0, ptr @nx_rt_ticks
  call void @llvm.memset.p0.i32(ptr @nx_rt_pend, i8 0, i32 {n}, i1 true)
  call void @llvm.memset.p0.i32(ptr @nx_rt_busy, i8 0, i32 {n}, i1 true)
  call void @llvm.memset.p0.i32(ptr @nx_rt_over, i8 0, i32 {over}, i1 true)
{setup}{paint}  fence seq_cst
{enable}  br label %wait
wait:
  %s = load volatile i8, ptr @nx_rt_stopped
  %done = icmp ne i8 %s, 0
  br i1 %done, label %end, label %sleep
sleep:
  call void asm sideeffect \"wfi\", \"~{{memory}}\"()
  br label %wait
end:
{disable}  fence seq_cst
{measure}  ret void
}}
",
            over = 4 * n,
            paint = paint,
            measure = measure
        );
        self.declare("declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)");
    }

    /// The intrinsics of `std.rt` for the tasks: `run`, `stop`, `overruns`, `now_cycles`.
    /// Without tasks, `run` returns at once and the others give 0.
    pub(crate) fn rt_intrinsic(&mut self, name: &str, task: Option<FnId>) -> Option<String> {
        let has = self.p.rt.is_some();
        Some(match name {
            "run" if has => "call void @nx_rt_run()".into(),
            "stop" if has => "store volatile i8 1, ptr @nx_rt_stopped".into(),
            "run" | "stop" => String::new(),
            "overruns" => match (&self.p.rt, task) {
                (Some(plan), Some(t)) => match plan.tasks.iter().position(|x| x.f == t) {
                    Some(i) => format!("load volatile i32, ptr getelementptr inbounds ([{} x i32], ptr @nx_rt_over, i32 0, i32 {})", plan.tasks.len(), i),
                    None => "add i32 0, 0".into(),
                },
                _ => "add i32 0, 0".into(),
            },
            "now_cycles" if has => "call i64 @nx_rt_now()".into(),
            "stack_used" if has => "load volatile i32, ptr @nx_rt_used".into(),
            "stack_used" => "add i32 0, 0".into(),
            "now_cycles" => "add i64 0, 0".into(),
            _ => return None,
        })
    }
}

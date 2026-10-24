//! Structural checks of the generated LLVM IR, without the LLVM library (they run on
//! every machine). The behavior itself is checked by `tests/run` where LLVM is present.

use nxc_driver::{compile_source, CompileOptions, Mode, Target};

fn ir_for(target: &str, src: &str) -> String {
    let mut opts = CompileOptions::new(Mode::Program);
    opts.target = Target::from_name(target).unwrap();
    let src = src.to_string();
    // The analyses are recursive: a large stack, as in `nxc` (contract K6.3).
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            let c = compile_source("main.nx", src.into_bytes(), None, &opts);
            assert_eq!(c.errors, 0, "{}", c.diagnostics);
            c.output.expect("generated IR").ir
        })
        .unwrap()
        .join()
        .unwrap()
}

/// On Linux, memory faults reach the runtime through `nx_linux_fault`, installed by the
/// entry point before `main` (ADR 0011, task T2: a stack overflow ends like a panic).
#[test]
fn linux_programs_install_the_fault_handler() {
    for target in ["x86_64-linux", "aarch64-linux"] {
        let ir = ir_for(target, "fn main() {\n    println(\"ok\")\n}\n");
        assert!(ir.contains("define void @nx_linux_fault(i32 %sig, ptr %info, ptr %ctx)"), "{}: no fault entry", target);
        let start = &ir[ir.find("define void @nx_linux_start").expect("entry point")..];
        let start = &start[..start.find("\n}").unwrap()];
        let install = start.find("(ptr @nx_linux_fault)").expect("handler installed by the entry point");
        let main_call = start.rfind("call void").unwrap();
        assert!(install < main_call, "{}: the handler must be installed before the program starts:\n{}", target, start);
    }
}

#[test]
fn bare_runtime_saves_and_restores_interrupt_masks() {
    let src = "fn main() {\n    var v: Vec[u64] = Vec.with_capacity(8)\n    v.push(42)\n    println(v[0])\n}\n";
    for (target, save, restore) in [
        ("thumbv7em-none", "primask\\0Acpsid i", "msr primask"),
        ("thumbv7m-none", "primask\\0Acpsid i", "msr primask"),
        ("riscv32-none", "csrrci $0, mstatus, 8", "csrs mstatus"),
        ("riscv64-none", "csrrci $0, mstatus, 8", "csrs mstatus"),
        ("aarch64-none", "daif\\0Amsr daifset", "msr daif"),
    ] {
        let ir = ir_for(target, src);
        assert!(ir.contains(save), "{}: no save/mask", target);
        assert!(ir.contains(restore), "{}: no restore", target);
        assert!(ir.contains("~{memory}"), "{}: missing compiler barrier", target);
    }
    let hosted = ir_for("x86_64-linux", src);
    assert!(!hosted.contains("irq_save") && !hosted.contains("irq_restore"));
}

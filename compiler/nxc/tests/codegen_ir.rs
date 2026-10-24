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
fn stack_probes_cover_all_generated_functions_on_supported_targets() {
    let src = "fn deep(n: u64) -> u64 {\n    let buf: [u64; 524288] = [n; 524288]\n    let r = deep(n +% 1)\n    return r ^ buf[(r % 524288) as usize]\n}\nfn main() { println(deep(0)) }\n";
    for target in ["x86_64-linux", "aarch64-linux", "x86_64-macos", "aarch64-macos", "x86_64-windows", "wasm32-wasi", "thumbv7em-none", "riscv64-linux"] {
        let ir = ir_for(target, src);
        let probes = matches!(target, "x86_64-linux" | "aarch64-linux" | "x86_64-macos" | "aarch64-macos");
        assert_eq!(ir.contains("attributes #0 = { \"probe-stack\"=\"inline-asm\" }"), probes, "{}", target);
        let headers: Vec<_> = ir.lines().filter(|line| line.starts_with("define ")).collect();
        assert!(!headers.is_empty());
        for header in headers {
            assert_eq!(header.contains(" #0"), probes, "{}: {}", target, header);
        }
        assert_eq!(ir, ir_for(target, src), "{}: non-deterministic IR", target);
    }
}

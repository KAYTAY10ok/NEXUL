//! Structural checks of the generated LLVM IR, without the LLVM library (they run on
//! every machine). The behavior itself is checked by `tests/run` where LLVM is present.

use nxc_driver::{compile_source, CompileOptions, Mode, Target};

fn ir_for(target: &str, src: &str) -> String {
    let mut opts = CompileOptions::new(Mode::Program);
    opts.target = Target::from_name(target).unwrap();
    compile_ir(opts, src)
}

/// The IR of a program written in edition 1.
fn ir_for_edition1(target: &str, src: &str) -> String {
    let mut opts = CompileOptions::new(Mode::Program);
    opts.target = Target::from_name(target).unwrap();
    opts.edition = nxc_syntax::Edition::V1;
    compile_ir(opts, src)
}

fn compile_ir(opts: CompileOptions, src: &str) -> String {
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

/// The wrapper of an `@export` function passes its other parameters unchanged and in
/// their order, between the checked ones, and returns nothing for a function without a
/// result (task C4).
#[test]
fn export_wrappers_pass_the_other_parameters_and_return_nothing() {
    let src = "type Pourcent = i32 where it in 0..=100\n\n@export(\"mix\")\npub fn i64 mix(bool b, i64 x, char c, f64 y, Pourcent p) = x + ((c as u32) as i64) + (y as i64)\n\n@export(\"reset\")\npub fn reset(bool b) {\n    println(\"{b}\")\n}\n\nfn main() {\n    println(\"{mix(true, 1, 'a', 2.0, 3)}\")\n    reset(false)\n}\n";
    for target in ["x86_64-linux", "x86_64-windows", "wasm32-web"] {
        let ir = ir_for_edition1(target, src);
        let def = |sym: &str| -> String {
            let at = ir.find(&format!("@\"{}\"(", sym)).and_then(|i| ir[..i].rfind("define ")).unwrap_or_else(|| panic!("{}: no definition of `{}`", target, sym));
            let body = &ir[at..];
            body[..body.find("\n}").unwrap()].to_string()
        };
        let mix = def("mix");
        assert!(mix.contains("@\"mix\"(i8 %a0, i64 %a1, i32 %a2, double %a3, i32 %a4)"), "{}: parameters of the wrapper:\n{}", target, mix);
        assert!(mix.contains("icmp ugt i8 %a0, 1") && mix.contains("icmp ugt i32 %a2, 1114111") && mix.contains("__nexul_validate"), "{}: checks:\n{}", target, mix);
        let call = mix.lines().find(|l| l.contains("%r = call i64")).unwrap_or_else(|| panic!("{}: no call:\n{}", target, mix));
        assert!(call.contains("i64 %a1, ") && call.contains("double %a3, "), "{}: the other parameters are passed unchanged:\n{}", target, call);
        let reset = def("reset");
        assert!(reset.contains("@\"reset\"(i8 %a0)") && reset.contains("icmp ugt i8 %a0, 1"), "{}: `bool` not checked:\n{}", target, reset);
        assert!(reset.contains("  call void ") && reset.trim_end().ends_with("ret void"), "{}: a function without a result:\n{}", target, reset);
    }
}

/// The values C passes to an `@export` function are checked on entry (ADR 0011 T4, task
/// C4; ADR 0014 §2.7): its symbol is a wrapper that checks a `bool` byte, a `char` and a
/// constrained type, then calls the function; NEXUL code calls the function directly.
#[test]
fn exports_check_the_values_received_from_c() {
    let src = "type Pourcent = i32 where it in 0..=100\ntype Flag = distinct bool is Copy\ntype Letter = distinct char is Copy\n\n@export(\"flip\")\npub fn bool flip(bool b) = not b\n\n@export(\"code\")\npub fn u32 code(char c) = c as u32\n\n@export(\"flag\")\npub fn bool flag(Flag b) = b as bool\n\n@export(\"letter\")\npub fn u32 letter(Letter c) = (c as char) as u32\n\n@export(\"half\")\npub fn i32 half(Pourcent p) = p / 2\n\n@export(\"plain\")\npub fn i32 plain(i32 x) = x\n\nfn main() {\n    println(\"{flip(true)} {code('a')} {half(40)} {plain(1)}\")\n}\n";
    for target in ["x86_64-linux", "x86_64-windows", "wasm32-web"] {
        let mut opts = CompileOptions::new(Mode::Program);
        opts.target = Target::from_name(target).unwrap();
        opts.edition = nxc_syntax::Edition::V1;
        let src = src.to_string();
        let ir = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(move || {
                let c = compile_source("main.nx", src.into_bytes(), None, &opts);
                assert_eq!(c.errors, 0, "{}", c.diagnostics);
                c.output.expect("generated IR").ir
            })
            .unwrap()
            .join()
            .unwrap();
        let def = |sym: &str| -> String {
            let at = ir.find(&format!("@\"{}\"(", sym)).and_then(|i| ir[..i].rfind("define ")).unwrap_or_else(|| panic!("{}: no definition of `{}`", target, sym));
            let body = &ir[at..];
            body[..body.find("\n}").unwrap()].to_string()
        };
        let flip = def("flip");
        assert!(flip.contains("@\"flip\"(i8 %a0)") && flip.contains("icmp ugt i8 %a0, 1") && flip.contains("(i32 15, ptr @loc."), "{}: `bool` not checked:\n{}", target, flip);
        let flag = def("flag");
        assert!(flag.contains("@\"flag\"(i8 %a0)") && flag.contains("icmp ugt i8 %a0, 1"), "{}: distinct `bool` not checked at its external width:\n{}", target, flag);
        assert!(flag.find("icmp ugt i8").unwrap() < flag.find("trunc i8").unwrap(), "{}: distinct `bool` must be checked before truncation:\n{}", target, flag);
        let code = def("code");
        assert!(code.contains("@\"code\"(i32 %a0)") && code.contains("icmp ugt i32 %a0, 1114111") && code.contains("(i32 13, ptr @loc."), "{}: `char` not checked:\n{}", target, code);
        let letter = def("letter");
        assert!(letter.contains("@\"letter\"(i32 %a0)") && letter.contains("icmp ugt i32 %a0, 1114111") && letter.contains("icmp ult i32 %s0, 2048"), "{}: distinct `char` not checked:\n{}", target, letter);
        let half = def("half");
        assert!(half.contains("@\"half\"(i32 %a0)") && half.contains("__nexul_validate"), "{}: constrained type not checked:\n{}", target, half);
        let plain = def("plain");
        assert!(plain.contains("@\"plain\"(i32 %a1)"), "{}: an export without such parameters is the function itself:\n{}", target, plain);
        // NEXUL code calls the checked function itself, not its wrapper.
        assert!(!ir.contains("call i1 @\"flip\"("), "{}: internal call through the wrapper", target);
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

/// Tasks of `std.tache` (ADR 0013): on Linux a thread is started by `clone` on its own
/// stack, then the child calls the function of the task; the anonymous function of a
/// launched task gets its environment on the heap. Without threads (WebAssembly), no
/// `clone`.
#[test]
fn tasks_start_threads_on_linux() {
    let src = "import std.tache\n\nfn main() {\n    var a = 0\n    var b = 0\n    tache.portee(fn(inout p: tache.Portee) {\n        p.lancer(fn() { a = 1 })\n        p.lancer(fn() { b = 2 })\n    })\n    println(\"{a} {b}\")\n}\n";
    for (target, call) in [("x86_64-linux", "call *%r13"), ("aarch64-linux", "blr x19"), ("riscv64-linux", "jalr s1")] {
        let ir = ir_for(target, src);
        let clone = ir.lines().find(|l| l.contains("asm sideeffect") && l.contains(call)).unwrap_or_else(|| panic!("{}: no clone", target));
        assert!(clone.contains(" 3477248, "), "{}: clone flags:\n{}", target, clone);
        assert!(ir.contains("atomic") && ir.contains("cmpxchg"), "{}: no atomic operation", target);
        let closure = &ir[ir.find("define internal void @\"main.main.closure").expect("closure of the portee")..];
        let closure = &closure[..closure.find("\n}").unwrap()];
        assert_eq!(closure.matches("call ptr @\"std.rt.alloc").count(), 2, "{}: environments of the tasks:\n{}", target, closure);
    }
    let wasm = ir_for("wasm32-wasi", src);
    assert!(!wasm.contains("asm sideeffect \"syscall"), "no thread on wasm32-wasi");
}

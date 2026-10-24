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

/// The values C passes to an `@export` function are checked on entry (ADR 0011 T4, task
/// C4; ADR 0014 §2.7): its symbol is a wrapper that checks a `bool` byte, a `char` and a
/// constrained type, then calls the function; NEXUL code calls the function directly.
#[test]
fn exports_check_the_values_received_from_c() {
    let src = "type Pourcent = i32 where it in 0..=100\n\n@export(\"flip\")\npub fn bool flip(bool b) = not b\n\n@export(\"code\")\npub fn u32 code(char c) = c as u32\n\n@export(\"half\")\npub fn i32 half(Pourcent p) = p / 2\n\n@export(\"plain\")\npub fn i32 plain(i32 x) = x\n\nfn main() {\n    println(\"{flip(true)} {code('a')} {half(40)} {plain(1)}\")\n}\n";
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
        let code = def("code");
        assert!(code.contains("@\"code\"(i32 %a0)") && code.contains("icmp ugt i32 %a0, 1114111") && code.contains("(i32 13, ptr @loc."), "{}: `char` not checked:\n{}", target, code);
        let half = def("half");
        assert!(half.contains("@\"half\"(i32 %a0)") && half.contains("__nexul_validate"), "{}: constrained type not checked:\n{}", target, half);
        let plain = def("plain");
        assert!(plain.contains("@\"plain\"(i32 %a1)"), "{}: an export without such parameters is the function itself:\n{}", target, plain);
        // NEXUL code calls the checked function itself, not its wrapper.
        assert!(!ir.contains("call i1 @\"flip\"("), "{}: internal call through the wrapper", target);
    }
}

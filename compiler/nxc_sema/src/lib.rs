//! NEXUL semantic analysis: modules, name resolution, types, inference, typed tree
//! (THIR), exhaustiveness and the place rules of the memory model.

pub mod check;
pub mod collect;
pub mod consts;
pub mod defs;
pub mod derive;
pub mod exhaust;
pub mod load;
pub mod places;
pub mod target;
pub mod thir;
pub mod ty;
pub mod util;

pub use defs::Program;
pub use load::ProgramSource;

use nxc_syntax::{DiagSink, Diagnostic, SourceMap};

/// Runs every front-end pass. Diagnostics are reported to `diags`; the returned
/// program is only complete if no error was reported.
pub fn analyze(src: ProgramSource, sm: &mut SourceMap, diags: &DiagSink) -> Program {
    let mut prog = load::load(src, sm, diags);
    collect::collect(&mut prog, diags);
    consts::eval_all(&mut prog, diags);
    consts::eval_statics(&mut prog, diags);
    check::check_program(&mut prog, diags);
    exhaust::check_program(&prog, diags);
    places::check_program(&prog, diags);
    report_unused_imports(&prog, diags);
    prog
}

fn report_unused_imports(prog: &Program, diags: &DiagSink) {
    for m in &prog.modules {
        if m.is_std {
            continue;
        }
        let mut entries: Vec<_> = m.imports.iter().collect();
        entries.sort_by_key(|(_, e)| e.span.lo);
        for (name, e) in entries {
            if !e.used.get() {
                diags.emit(
                    Diagnostic::warning("W0004", format!("unused import `{}`", name))
                        .primary(e.span, "imported but never used")
                        .help("remove this import"),
                );
            }
        }
    }
}

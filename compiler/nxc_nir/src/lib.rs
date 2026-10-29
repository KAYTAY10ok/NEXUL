//! NEXUL intermediate representation (NIR): lowering from the typed tree, ownership
//! analysis and drop elaboration.

pub mod build;
pub mod effects;
pub mod nir;
pub mod ownership;

use nxc_sema::defs::{FnId, Program};
use nxc_syntax::{DiagSink, SourceMap};
use std::collections::HashMap;

/// Lowers every function body, checks initialization and moves, then elaborates drops.
pub fn lower_program(p: &Program, sm: &SourceMap, diags: &DiagSink) -> HashMap<FnId, nir::Body> {
    let mut ids: Vec<FnId> = p.bodies.keys().copied().collect();
    ids.sort();
    let mut out = HashMap::new();
    for id in ids {
        let body = build::build(p, sm, id, &p.bodies[&id]);
        ownership::check(p, &body, diags);
        out.insert(id, body);
    }
    if !diags.has_errors() {
        for body in out.values_mut() {
            ownership::elaborate_drops(p, body);
        }
        effects::check(p, &out, diags);
    }
    out
}

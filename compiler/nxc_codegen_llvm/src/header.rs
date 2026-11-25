//! The C header of a library (`nxc build --lib --header nxlib.h`, spec §8.4): the
//! declaration of each `@export` function, and the `@repr("C")` structures they use,
//! in the order C needs them.
//!
//! The types, in C:
//! - the integers are those of `<stdint.h>`: `usize` is `size_t`, `isize` is `ptrdiff_t`;
//! - `bool` is that of `<stdbool.h>`, and `char` is `uint32_t`, a Unicode scalar value;
//! - `Ptr[T]` is `T *`, and `Ptr[()]` is `void *`;
//! - a `fn` type is a pointer to a function;
//! - a distinct or constrained type is its base;
//! - a `@repr("C")` structure is a `typedef struct`.

use nxc_sema::defs::{AdtId, FnKind, Program};
use nxc_sema::ty::{FloatTy, IntTy, Ty, TyKind};
use std::fmt::Write;

struct Header<'a> {
    p: &'a Program,
    structs: Vec<AdtId>,
    defs: String,
}

impl Header<'_> {
    /// The declaration of `name` with type `t` (`int32_t x`, `int32_t (*f)(int32_t)`).
    fn decl(&mut self, t: Ty, name: &str) -> String {
        let p = self.p;
        let named = |base: &str| if name.is_empty() { base.to_string() } else { format!("{} {}", base, name) };
        match &*p.tcx.kind(t) {
            TyKind::Int(i) => named(match i {
                IntTy::I8 => "int8_t",
                IntTy::I16 => "int16_t",
                IntTy::I32 => "int32_t",
                IntTy::I64 => "int64_t",
                IntTy::Isize => "ptrdiff_t",
                IntTy::U8 => "uint8_t",
                IntTy::U16 => "uint16_t",
                IntTy::U32 => "uint32_t",
                IntTy::U64 => "uint64_t",
                IntTy::Usize => "size_t",
            }),
            TyKind::Float(FloatTy::F32) => named("float"),
            TyKind::Float(FloatTy::F64) => named("double"),
            TyKind::Bool => named("bool"),
            TyKind::Char => named("uint32_t"),
            TyKind::Unit | TyKind::Never => named("void"),
            TyKind::Ptr(e) => {
                let e = *e;
                self.decl(e, &format!("*{}", name))
            }
            TyKind::Array(e, n) => {
                let (e, n) = (*e, *n);
                self.decl(e, &format!("{}[{}]", name, n))
            }
            TyKind::Fn(params, ret) => {
                let (params, ret) = (params.clone(), *ret);
                let ps = self.params(&params.iter().map(|(_, t)| (String::new(), *t)).collect::<Vec<_>>());
                self.decl(ret, &format!("(*{})({})", name, ps))
            }
            TyKind::Adt(id, args) => {
                let adt = p.adt(*id);
                if adt.newtype.is_some() {
                    let base = p.tcx.subst(adt.fields()[0].ty, args);
                    return self.decl(base, name);
                }
                let id = *id;
                self.structure(id);
                named(&p.adt(id).name)
            }
            _ => named(&format!("/* {} */ void", p.ty_str(t))),
        }
    }

    fn params(&mut self, ps: &[(String, Ty)]) -> String {
        if ps.is_empty() {
            return "void".into();
        }
        ps.iter().map(|(n, t)| self.decl(*t, n)).collect::<Vec<_>>().join(", ")
    }

    /// Defines a `@repr("C")` structure, after the structures of its fields.
    fn structure(&mut self, id: AdtId) {
        if self.structs.contains(&id) {
            return;
        }
        self.structs.push(id);
        let p = self.p;
        let fields: Vec<(String, Ty)> = p.adt(id).fields().iter().map(|f| (f.name.clone(), f.ty)).collect();
        let mut body = String::new();
        for (n, t) in &fields {
            let d = self.decl(*t, n);
            let _ = writeln!(body, "    {};", d);
        }
        let name = &p.adt(id).name;
        let _ = writeln!(self.defs, "typedef struct {} {{\n{}}} {};\n", name, body, name);
    }
}

/// The header of the `@export` functions of `p`; `guard` names its include guard.
pub fn c_header(p: &Program, source: &str, guard: &str) -> String {
    let mut h = Header { p, structs: Vec::new(), defs: String::new() };
    let mut fns = Vec::new();
    for f in &p.fns {
        let Some(sym) = &f.export else { continue };
        if !matches!(f.kind, FnKind::User(_)) {
            continue;
        }
        let ps: Vec<(String, Ty)> = f.params.iter().map(|q| (q.name.clone(), q.ty)).collect();
        let params = h.params(&ps);
        fns.push(format!("{};", h.decl(f.ret, &format!("{}({})", sym, params))));
    }
    let guard: String = guard.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
    let mut out = String::new();
    let _ = writeln!(out, "/* The C interface of `{}`, written by nxc. Do not edit. */", source);
    let _ = writeln!(out, "#ifndef {}\n#define {}\n", guard, guard);
    out.push_str("#include <stdbool.h>\n#include <stddef.h>\n#include <stdint.h>\n\n");
    out.push_str("#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n");
    out.push_str(&h.defs);
    for f in fns {
        let _ = writeln!(out, "{}", f);
    }
    out.push_str("\n#ifdef __cplusplus\n}\n#endif\n\n");
    let _ = writeln!(out, "#endif /* {} */", guard);
    out
}

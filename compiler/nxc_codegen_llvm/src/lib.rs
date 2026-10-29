//! LLVM IR back end (spec §10.1).
//!
//! The NIR of every reachable function is monomorphized and translated into textual LLVM
//! IR, which the driver gives to LLVM (used as a library, as infrastructure only) to
//! produce machine code. Every rule of NEXUL is decided before this point; this crate
//! only chooses the machine representation:
//!
//! - scalars (integers, floats, `bool`, `char`, pointers, functions) are SSA values;
//! - aggregates (tuples, arrays, structures, enumerations) live in memory and are copied
//!   with `llvm.memcpy`;
//! - aggregates are passed by address and returned through a hidden pointer (NEXUL's own
//!   calling convention); exported and external functions use the C convention.
//!
//! The runtime (panics, heap, formatting...) is NEXUL code from the standard library,
//! called through the functions marked `@lang`.

pub mod cabi;
mod debuginfo;
mod func;
pub mod layout;
mod rt;

use debuginfo::DebugInfo;
use layout::LayoutCx;
use nxc_nir::build::param_by_ref;
use nxc_nir::nir::Body;
use nxc_sema::defs::{TraitId, AdtId, AdtKind, ConstValue, FnId, FnKind, Program, StaticId, StaticValue};
use nxc_sema::target::{Arch, Os, Target};
use nxc_sema::ty::{FloatTy, IntTy, Ty, TyKind};
use nxc_syntax::ast::Convention;
use nxc_syntax::token::{Align, FormatKind, FormatSpec};
use nxc_syntax::{SourceMap, Span};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

pub enum Entry {
    /// A program: its `main` function.
    Main(FnId),
    /// The `@test` functions; the first command-line argument selects one.
    Tests(Vec<FnId>),
    /// A library: only the `@export` functions are entry points.
    Library,
}

pub struct Options {
    pub target: Target,
    pub debug_info: bool,
    pub optimize: bool,
    /// Directory of the program (for the debug information).
    pub root_dir: String,
    /// Counts calls and time per function (`nxc build --profile`, used by `nxprof`).
    pub profile: bool,
}

pub struct Output {
    pub ir: String,
    /// Imported functions by library: Windows DLLs, WebAssembly import modules.
    pub imports: Vec<(String, String)>,
    /// Libraries to link (`@link`), for the other targets.
    pub links: Vec<String>,
    /// Exported symbols (`@export`).
    pub exports: Vec<String>,
    /// Names of the profiled functions, by counter index (`--profile`).
    pub profile_names: Vec<String>,
    /// What bounds the stack of a program with real-time tasks (ADR 0018 §6).
    pub stack: Option<StackSpec>,
}

/// The functions whose stack the driver bounds on the linked executable (ADR 0018 §6),
/// by their symbols.
#[derive(Clone, Debug)]
pub struct StackSpec {
    /// Reserve of the stack, in bytes.
    pub reserve: u64,
    /// Bytes the core pushes on an exception.
    pub exception: u64,
    /// The startup chain up to `rt.run()`: their frames only.
    pub chain: Vec<String>,
    /// For each level of priority, its handlers (dispatcher, interrupts).
    pub levels: Vec<(u8, Vec<String>)>,
    pub systick: Option<String>,
    /// Functions that never return (panics, `exit`): a safe stop, whose stack is guarded
    /// by the MPU; not part of the bound.
    pub stops: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum TypeKey {
    Ty(Ty),
    Payload(Ty, u32),
}

/// How a value of some type is passed and returned.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PKind {
    /// By value, as this LLVM scalar type.
    Scalar(String),
    /// By address (aggregates, `()`, and parameters passed by reference).
    Ptr,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RetKind {
    Void,
    Scalar(String),
    /// Through a hidden first parameter pointing to the destination.
    Sret(Ty),
}

#[derive(Clone, Debug)]
pub(crate) struct Sig {
    pub ret: RetKind,
    pub params: Vec<PKind>,
    pub track_caller: bool,
    /// Anonymous functions receive their environment pointer first (after `sret`).
    pub env: bool,
    /// The (concrete) parameter types, to adapt arguments passed by reference by
    /// generic code to parameters passed by value.
    pub param_tys: Vec<Ty>,
}

pub(crate) struct Gen<'a> {
    pub p: &'a Program,
    pub sm: &'a SourceMap,
    pub bodies: &'a std::collections::HashMap<FnId, Body>,
    pub opts: &'a Options,
    pub lx: LayoutCx<'a>,
    type_names: HashMap<TypeKey, String>,
    type_defs: String,
    pub instances: HashMap<(FnId, Vec<Ty>), String>,
    pub fn_queue: Vec<(FnId, Vec<Ty>)>,
    drop_names: HashMap<Ty, String>,
    drop_queue: Vec<Ty>,
    externs: HashMap<FnId, String>,
    pub decls: String,
    declared: HashSet<String>,
    pub globals: String,
    strings: HashMap<Vec<u8>, String>,
    string_values: HashMap<Vec<u8>, String>,
    locs: HashMap<(usize, usize, usize), String>,
    specs: HashMap<String, String>,
    statics: HashMap<StaticId, String>,
    pub fn_defs: String,
    pub helpers: HashSet<String>,
    counter: usize,
    pub di: Option<DebugInfo>,
    pub imports: Vec<(String, String)>,
    pub links: Vec<String>,
    pub exports: Vec<String>,
    pub profile_names: Vec<String>,
    pub module_asm: Vec<String>,
    /// Filled by the scheduler of the real-time tasks (`rt.rs`).
    pub stack: Option<StackSpec>,
    /// The startup chain of a bare-metal program, for `stack`.
    pub(crate) entry_chain: Vec<String>,
    vtables: HashMap<(TraitId, Ty), String>,
}

/// Escapes a quoted LLVM identifier.
/// The symbol of an LLVM global: `@"main.f#3"` → `main.f#3`.
pub(crate) fn symbol_of(name: &str) -> String {
    let n = name.trim_start_matches('@');
    n.strip_prefix('"').and_then(|n| n.strip_suffix('"')).unwrap_or(n).to_string()
}

pub(crate) fn quote_name(s: &str) -> String {
    let mut o = String::from("\"");
    for b in s.bytes() {
        if b == b'"' || b == b'\\' || !(0x20..0x7f).contains(&b) {
            let _ = write!(o, "\\{:02X}", b);
        } else {
            o.push(b as char);
        }
    }
    o.push('"');
    o
}

fn c_string(bytes: &[u8]) -> String {
    let mut s = String::from("c\"");
    for &b in bytes {
        if (0x20..0x7f).contains(&b) && b != b'"' && b != b'\\' {
            s.push(b as char);
        } else {
            let _ = write!(s, "\\{:02X}", b);
        }
    }
    s.push_str("\\00\"");
    s
}

/// An integer constant of `bits` bits, written as LLVM expects (two's complement).
pub(crate) fn int_const(n: i128, bits: u32) -> String {
    if bits >= 128 {
        return n.to_string();
    }
    let m = 1i128 << bits;
    let mut v = n.rem_euclid(m);
    if v >= m / 2 {
        v -= m;
    }
    v.to_string()
}

pub(crate) fn float_const(x: f64, f: FloatTy) -> String {
    match f {
        FloatTy::F64 => format!("0x{:016X}", x.to_bits()),
        FloatTy::F32 => format!("0x{:016X}", ((x as f32) as f64).to_bits()),
    }
}

/// Generates the LLVM module of a whole program.
pub fn generate(p: &Program, sm: &SourceMap, bodies: &HashMap<FnId, Body>, entry: &Entry, opts: &Options) -> Output {
    let lx = LayoutCx::new(p, opts.target.ptr_bytes());
    let di = if opts.debug_info {
        let root_name = sm.try_get(p.modules[p.root.idx()].file).map(|f| f.name.clone()).unwrap_or_else(|| "main.nx".into());
        Some(DebugInfo::new(&root_name, &opts.root_dir, opts.target.os == Os::Windows, opts.optimize))
    } else {
        None
    };
    let mut g = Gen {
        p,
        sm,
        bodies,
        opts,
        lx,
        type_names: HashMap::new(),
        type_defs: String::new(),
        instances: HashMap::new(),
        fn_queue: Vec::new(),
        drop_names: HashMap::new(),
        drop_queue: Vec::new(),
        externs: HashMap::new(),
        decls: String::new(),
        declared: HashSet::new(),
        globals: String::new(),
        strings: HashMap::new(),
        string_values: HashMap::new(),
        locs: HashMap::new(),
        specs: HashMap::new(),
        statics: HashMap::new(),
        fn_defs: String::new(),
        helpers: HashSet::new(),
        counter: 0,
        di,
        imports: Vec::new(),
        links: Vec::new(),
        exports: Vec::new(),
        profile_names: Vec::new(),
        module_asm: Vec::new(),
        stack: None,
        entry_chain: Vec::new(),
        vtables: HashMap::new(),
    };
    g.emit_entry(entry);
    // Exported functions are roots too.
    for (i, f) in p.fns.iter().enumerate() {
        if f.export.is_some() && matches!(f.kind, FnKind::User(_)) {
            g.instance(FnId(i as u32), vec![]);
        }
    }
    // Generate every reachable function and drop glue (they discover more work).
    loop {
        if let Some((f, targs)) = g.fn_queue.pop() {
            g.emit_fn(f, &targs);
            continue;
        }
        if let Some(t) = g.drop_queue.pop() {
            g.emit_drop_glue(t);
            continue;
        }
        break;
    }
    g.emit_builtins();
    g.finish()
}

impl<'a> Gen<'a> {
    pub fn fresh(&mut self) -> usize {
        self.counter += 1;
        self.counter
    }

    pub fn tcx(&self) -> &'a nxc_sema::ty::TyCtx {
        &self.p.tcx
    }

    /// `i64` or `i32`: the LLVM type of `usize`.
    pub fn usize_ty(&self) -> &'static str {
        if self.opts.target.ptr_bits == 64 {
            "i64"
        } else {
            "i32"
        }
    }

    pub fn int_bits(&self, i: IntTy) -> u32 {
        (self.lx.int_bytes(i) * 8) as u32
    }

    // -----------------------------------------------------------------------
    // Types

    /// Is a value of this type an SSA scalar (rather than an aggregate in memory)?
    pub fn is_scalar(&self, t: Ty) -> bool {
        matches!(&*self.tcx().kind(t), TyKind::Int(_) | TyKind::Float(_) | TyKind::Bool | TyKind::Char | TyKind::Ptr(_) | TyKind::Fn(..))
    }

    pub fn is_void(&self, t: Ty) -> bool {
        matches!(&*self.tcx().kind(t), TyKind::Unit | TyKind::Never | TyKind::Error)
    }

    /// The LLVM type of a value of type `t` in memory.
    pub fn llty(&mut self, t: Ty) -> String {
        let tcx = self.tcx();
        match &*tcx.kind(t) {
            TyKind::Int(i) => format!("i{}", self.int_bits(*i)),
            TyKind::Float(FloatTy::F32) => "float".into(),
            TyKind::Float(FloatTy::F64) => "double".into(),
            TyKind::Bool => "i1".into(),
            TyKind::Char => "i32".into(),
            TyKind::Unit | TyKind::Never | TyKind::Error => "{}".into(),
            TyKind::Ptr(_) | TyKind::Fn(..) => "ptr".into(),
            TyKind::Closure(..) | TyKind::Dyn(_) => "{ ptr, ptr }".into(),
            TyKind::Array(e, n) => {
                let et = self.llty(*e);
                format!("[{} x {}]", n, et)
            }
            TyKind::Tuple(_) | TyKind::Adt(..) => self.named(TypeKey::Ty(t)),
            TyKind::Param(_) | TyKind::Infer(_) | TyKind::IntVar(_) | TyKind::FloatVar(_) => {
                panic!("internal: non-concrete type in code generation: {}", self.p.ty_str(t))
            }
        }
    }

    /// The payload structure of variant `v` of the enumeration `t`.
    pub fn payload_ty(&mut self, t: Ty, v: u32) -> String {
        self.named(TypeKey::Payload(t, v))
    }

    fn named(&mut self, key: TypeKey) -> String {
        if let Some(n) = self.type_names.get(&key) {
            return n.clone();
        }
        let label = match key {
            TypeKey::Ty(t) => self.p.ty_str(t),
            TypeKey::Payload(t, v) => {
                let (id, _) = self.tcx().as_adt(t).unwrap();
                format!("{}.{}", self.p.ty_str(t), self.p.adt(id).variants()[v as usize].name)
            }
        };
        let id = self.fresh();
        let name = format!("%{}", quote_name(&format!("{}#{}", label, id)));
        self.type_names.insert(key, name.clone());
        let tcx = self.tcx();
        let body = match key {
            TypeKey::Payload(t, v) => {
                let fields = self.lx.variant_fields(t, v);
                let parts: Vec<String> = fields.iter().map(|&f| self.llty(f)).collect();
                format!("{{ {} }}", parts.join(", "))
            }
            TypeKey::Ty(t) => match &*tcx.kind(t) {
                TyKind::Tuple(elems) => {
                    let parts: Vec<String> = elems.clone().iter().map(|&e| self.llty(e)).collect();
                    format!("{{ {} }}", parts.join(", "))
                }
                TyKind::Adt(aid, args) => {
                    let adt = self.p.adt(*aid);
                    match &adt.kind {
                        AdtKind::Struct { fields } => {
                            let mut parts = Vec::new();
                            for f in fields {
                                if f.indirect {
                                    parts.push("ptr".to_string());
                                } else {
                                    let ft = tcx.subst(f.ty, args);
                                    parts.push(self.llty(ft));
                                }
                            }
                            format!("{{ {} }}", parts.join(", "))
                        }
                        AdtKind::Enum { .. } => {
                            let s = self.lx.enum_shape(t);
                            if !s.has_payload {
                                "{ i32 }".to_string()
                            } else {
                                let a = s.payload_align.min(8);
                                format!("{{ i32, [{} x i{}] }}", s.payload_size / a, a * 8)
                            }
                        }
                    }
                }
                _ => unreachable!(),
            },
        };
        let _ = writeln!(self.type_defs, "{} = type {}", name, body);
        name
    }

    pub fn size_of(&self, t: Ty) -> u64 {
        self.lx.layout(t).size
    }

    pub fn align_of(&self, t: Ty) -> u64 {
        self.lx.layout(t).align
    }

    // -----------------------------------------------------------------------
    // Signatures

    /// How a value of type `t` is passed.
    pub fn pass_kind(&mut self, t: Ty) -> PKind {
        if self.is_scalar(t) {
            PKind::Scalar(self.llty(t))
        } else {
            PKind::Ptr
        }
    }

    pub fn ret_kind(&mut self, t: Ty) -> RetKind {
        if self.is_void(t) {
            RetKind::Void
        } else if self.is_scalar(t) {
            RetKind::Scalar(self.llty(t))
        } else {
            RetKind::Sret(t)
        }
    }

    /// The signature of an instance of a NEXUL function (NEXUL calling convention).
    /// Parameters are passed by reference according to the declaration (the trait's
    /// for a trait method). Anonymous functions receive an environment pointer instead
    /// of their captured variables, and pass parameters according to their concrete
    /// types (like function values).
    pub fn fn_sig(&mut self, f: FnId, targs: &[Ty]) -> Sig {
        let fd = self.p.func(f);
        let tcx = self.tcx();
        let n_cap = fd.closure_captures();
        let is_closure = matches!(fd.kind, FnKind::Closure { .. });
        let mut params = Vec::new();
        let mut param_tys = Vec::new();
        for (i, prm) in fd.params.iter().enumerate() {
            if i < n_cap {
                continue;
            }
            let t = tcx.subst(prm.ty, targs);
            let by_ref = if is_closure { param_by_ref(self.p, prm.conv, t) } else { param_by_ref(self.p, prm.conv, fd.abi_param_ty(i)) };
            if by_ref {
                params.push(PKind::Ptr);
            } else {
                params.push(self.pass_kind(t));
            }
            param_tys.push(t);
        }
        let ret = tcx.subst(fd.ret, targs);
        Sig { ret: self.ret_kind(ret), params, track_caller: fd.track_caller, env: is_closure, param_tys }
    }

    /// The signature of a call through a function value of type `t`.
    pub fn fn_ty_sig(&mut self, t: Ty) -> Sig {
        let tcx = self.tcx();
        let (params, ret, env) = match &*tcx.kind(t) {
            TyKind::Fn(ps, r) => (ps.clone(), *r, false),
            TyKind::Closure(ps, r) => (ps.clone(), *r, true),
            _ => (vec![], tcx.unit, false),
        };
        let mut out = Vec::new();
        let mut param_tys = Vec::new();
        for (c, pt) in params {
            if param_by_ref(self.p, c, pt) {
                out.push(PKind::Ptr);
            } else {
                out.push(self.pass_kind(pt));
            }
            param_tys.push(pt);
        }
        Sig { ret: self.ret_kind(ret), params: out, track_caller: false, env, param_tys }
    }

    pub fn sig_param_list(&self, sig: &Sig) -> Vec<String> {
        let mut v = Vec::new();
        if let RetKind::Sret(_) = sig.ret {
            v.push("ptr".to_string());
        }
        if sig.env {
            v.push("ptr".to_string());
        }
        for p in &sig.params {
            v.push(match p {
                PKind::Scalar(t) => t.clone(),
                PKind::Ptr => "ptr".into(),
            });
        }
        if sig.track_caller {
            v.push("ptr".into());
        }
        v
    }

    pub fn sig_ret(&self, sig: &Sig) -> String {
        match &sig.ret {
            RetKind::Scalar(t) => t.clone(),
            _ => "void".into(),
        }
    }

    // -----------------------------------------------------------------------
    // Functions

    pub fn instance(&mut self, f: FnId, targs: Vec<Ty>) -> String {
        // A trait method is the implementation for its concrete `Self`.
        let (f, targs) = if self.p.func(f).is_trait_decl() {
            self.p.resolve_trait_method(f, &targs).unwrap_or_else(|| panic!("internal: no implementation of `{}` for `{}`", self.p.func(f).name, self.p.ty_str(targs[0])))
        } else {
            (f, targs)
        };
        let key = (f, targs);
        if let Some(n) = self.instances.get(&key) {
            return n.clone();
        }
        let fd = self.p.func(f);
        let name = match &fd.export {
            Some(sym) if key.1.is_empty() && !self.export_checks(f) => format!("@{}", quote_name(sym)),
            _ => {
                let module = &self.p.modules[fd.module.idx()].path;
                let mut label = format!("{}.{}", module, fd.qualified_name(self.p));
                if !key.1.is_empty() {
                    let a: Vec<String> = key.1.iter().map(|&t| self.p.ty_str(t)).collect();
                    label = format!("{}[{}]", label, a.join(", "));
                }
                let id = self.fresh();
                format!("@{}", quote_name(&format!("{}#{}", label, id)))
            }
        };
        self.instances.insert(key.clone(), name.clone());
        self.fn_queue.push(key);
        name
    }

    /// Does C pass to this `@export` function a parameter whose values are not all valid
    /// in NEXUL (`bool`, `char`, a constrained type)? Its symbol is then a wrapper that
    /// checks them before calling the function (ADR 0011 T4, ADR 0014 §2.7).
    pub fn export_checks(&self, f: FnId) -> bool {
        let fd = self.p.func(f);
        // The functions of std.builtins keep LLVM's convention: LLVM calls them.
        let user = !self.p.modules[fd.module.idx()].is_std;
        fd.export.is_some()
            && (user && cabi::arm_hard_float(&self.opts.target) || fd.params.iter().any(|p| match &*self.tcx().kind(p.ty) {
                TyKind::Bool | TyKind::Char => true,
                TyKind::Adt(id, _) => self.p.adt(*id).newtype.is_some() || self.p.adt(*id).repr_c,
                _ => false,
            }) || matches!(self.tcx().as_adt(fd.ret), Some((id, _)) if self.p.adt(id).repr_c))
    }

    /// A runtime function of the standard library.
    pub fn lang_fn(&mut self, name: &str) -> (String, Sig) {
        let f = self.p.lang_fn(name).unwrap_or_else(|| panic!("internal: the standard library has no `@lang(\"{}\")` function", name));
        let n = self.instance(f, vec![]);
        let sig = self.fn_sig(f, &[]);
        (n, sig)
    }

    pub fn lang_adt(&self, name: &str) -> AdtId {
        *self.p.lang_adts.get(name).unwrap_or_else(|| panic!("internal: the standard library has no `@lang(\"{}\")` type", name))
    }

    /// Declares an LLVM function once.
    pub fn declare(&mut self, line: &str) {
        if self.declared.insert(line.to_string()) {
            let _ = writeln!(self.decls, "{}", line);
        }
    }

    /// The symbol of an `extern "C"` function, declared with the C calling convention.
    pub fn extern_fn(&mut self, f: FnId) -> String {
        if let Some(n) = self.externs.get(&f) {
            return n.clone();
        }
        let fd = self.p.func(f);
        let name = format!("@{}", quote_name(&fd.name));
        let ptys: Vec<Ty> = fd.params.iter().map(|p| p.ty).collect();
        let ret_ty = fd.ret;
        let (ret, params) = self.c_signature(&ptys, ret_ty);
        let link = match &fd.kind {
            FnKind::Extern { link, .. } => link.clone(),
            _ => None,
        };
        let mut attrs = String::from("nounwind");
        if self.tcx().is_never(ret_ty) {
            attrs.push_str(" noreturn");
        }
        let target = self.opts.target.clone();
        match (target.os, &link) {
            (Os::Windows, l) => {
                // Without `@link`, a C function comes from the C runtime DLL of Windows.
                let dll = l.clone().unwrap_or_else(|| "ucrtbase".into());
                self.imports.push((dll, fd.name.clone()));
            }
            // macOS: the functions of the system library (libSystem), linked through a
            // text stub that lists them.
            (Os::MacOs, None) => self.imports.push(("System".into(), fd.name.clone())),
            (_, Some(l)) if target.arch == Arch::Wasm32 => {
                attrs.push_str(&format!(" \"wasm-import-module\"=\"{}\" \"wasm-import-name\"=\"{}\"", l, fd.name));
                self.imports.push((l.clone(), fd.name.clone()));
            }
            (_, Some(l)) => {
                if !self.links.contains(l) {
                    self.links.push(l.clone());
                }
            }
            _ => {}
        }
        let cc = self.c_cc();
        self.declare(&format!("declare {}{} {}({}) {}", cc, ret, name, params.join(", "), attrs));
        self.externs.insert(f, name.clone());
        name
    }

    /// The layout of a `@repr("C")` structure as the C calling conventions see it: its
    /// scalars flattened with their offsets.
    pub fn c_shape(&mut self, t: Ty) -> Option<cabi::Shape> {
        let tcx = self.tcx();
        let (id, _) = tcx.as_adt(t)?;
        if !self.p.adt(id).repr_c {
            return None;
        }
        let mut leaves = Vec::new();
        self.c_leaves(t, 0, &mut leaves);
        let l = self.lx.layout(t);
        Some(cabi::Shape { size: l.size, align: l.align, leaves, llty: self.llty(t) })
    }

    fn c_leaves(&mut self, t: Ty, base: u64, out: &mut Vec<(u64, cabi::Leaf)>) {
        let tcx = self.tcx();
        match &*tcx.kind(t) {
            TyKind::Float(nxc_sema::ty::FloatTy::F32) => out.push((base, cabi::Leaf::F32)),
            TyKind::Float(nxc_sema::ty::FloatTy::F64) => out.push((base, cabi::Leaf::F64)),
            TyKind::Int(i) => out.push((base, cabi::Leaf::Int(self.lx.int_bytes(*i)))),
            TyKind::Ptr(_) | TyKind::Fn(..) => out.push((base, cabi::Leaf::Ptr(self.lx.ptr_bytes))),
            TyKind::Array(e, n) => {
                let size = self.lx.layout(*e).size;
                for k in 0..*n {
                    self.c_leaves(*e, base + k * size, out);
                }
            }
            TyKind::Adt(id, args) => {
                let fields: Vec<Ty> = self.p.adt(*id).fields().iter().map(|f| tcx.subst(f.ty, args)).collect();
                let offsets = self.lx.struct_offsets(&fields);
                for (f, o) in fields.into_iter().zip(offsets) {
                    self.c_leaves(f, base + o, out);
                }
            }
            _ => unreachable!("internal: `{}` in a `@repr(\"C\")` structure", self.p.ty_str(t)),
        }
    }

    /// How the parameters and the result of a C signature are passed (spec §8.2).
    pub fn c_abi(&mut self, params: &[Ty], ret: Ty) -> (Vec<cabi::ArgAbi>, cabi::RetAbi) {
        let mut values = Vec::new();
        for &p in params {
            values.push(self.c_value(p));
        }
        let rv = if self.is_void(ret) || self.tcx().is_never(ret) { None } else { Some(self.c_value(ret)) };
        let target = self.opts.target.clone();
        cabi::classify(&target, &values, rv.as_ref())
    }

    fn c_value(&mut self, t: Ty) -> cabi::Value {
        match self.c_shape(t) {
            Some(s) => cabi::Value::Struct(s),
            None => cabi::Value::Scalar { float: matches!(&*self.tcx().kind(t), TyKind::Float(_)) },
        }
    }

    /// The calling convention written on what C calls or is called by. On ARM with a
    /// hard-float ABI (`thumbv7em`), LLVM's default convention passes floats in core
    /// registers (its float ABI is set by an option the C interface of LLVM does not
    /// have), while C passes them in VFP registers: `arm_aapcs_vfpcc` says so.
    pub fn c_cc(&self) -> &'static str {
        if cabi::arm_hard_float(&self.opts.target) {
            "arm_aapcs_vfpcc "
        } else {
            ""
        }
    }

    /// The LLVM result type and parameter list of a C signature (`sret` first).
    pub fn c_signature(&mut self, params: &[Ty], ret: Ty) -> (String, Vec<String>) {
        let (abis, rabi) = self.c_abi(params, ret);
        let mut out = Vec::new();
        let ret_s = match &rabi {
            cabi::RetAbi::Direct if self.is_void(ret) || self.tcx().is_never(ret) => "void".to_string(),
            // The result as it is: its extension is the callee's work, and the bits above
            // it are not read.
            cabi::RetAbi::Direct => self.llty(ret),
            cabi::RetAbi::Agg(ty) => ty.clone(),
            cabi::RetAbi::Sret { align } => {
                let lt = self.llty(ret);
                out.push(format!("ptr sret({}) align {}", lt, align));
                "void".to_string()
            }
        };
        for (&p, abi) in params.iter().zip(&abis) {
            match abi {
                cabi::ArgAbi::Direct => out.push(self.c_param(p)),
                cabi::ArgAbi::Pieces(ps) => out.extend(ps.iter().map(|(_, ty)| ty.clone())),
                cabi::ArgAbi::Agg(ty, attr) => out.push(format!("{}{}", ty, attr)),
                cabi::ArgAbi::Indirect { byval: true, align } => {
                    let lt = self.llty(p);
                    out.push(format!("ptr byval({}) align {}", lt, align));
                }
                cabi::ArgAbi::Indirect { byval: false, .. } => out.push("ptr".into()),
            }
        }
        (ret_s, out)
    }

    /// A parameter or return type with the C calling convention (narrow integers are
    /// extended by the caller, as C compilers expect).
    pub fn c_param(&mut self, t: Ty) -> String {
        let base = self.llty(t);
        let windows = self.opts.target.os == Os::Windows;
        match &*self.tcx().kind(t) {
            TyKind::Bool => "i1 zeroext".into(),
            TyKind::Int(i) if self.int_bits(*i) < 32 && !windows => {
                format!("{} {}", base, if i.signed() { "signext" } else { "zeroext" })
            }
            // RISC-V 64 keeps 32-bit integers sign-extended in its registers, unsigned
            // ones too (what clang emits).
            TyKind::Int(i) if self.int_bits(*i) == 32 && self.opts.target.arch == Arch::Riscv64 => format!("{} signext", base),
            _ => base,
        }
    }

    // -----------------------------------------------------------------------
    // Constants and globals

    /// A NUL-terminated byte array; returns its global name.
    pub fn bytes_global(&mut self, bytes: &[u8]) -> String {
        if let Some(n) = self.strings.get(bytes) {
            return n.clone();
        }
        let n = format!("@str.{}", self.fresh());
        let _ = writeln!(self.globals, "{} = private unnamed_addr constant [{} x i8] {}, align 1", n, bytes.len() + 1, c_string(bytes));
        self.strings.insert(bytes.to_vec(), n.clone());
        n
    }

    /// A constant `String` value (a literal: not owned, `cap == 0`).
    pub fn string_value(&mut self, s: &[u8]) -> String {
        if let Some(n) = self.string_values.get(s) {
            return n.clone();
        }
        let data = self.bytes_global(s);
        let sty = self.p.string_ty();
        let ty = self.llty(sty);
        let u = self.usize_ty();
        let n = format!("@strv.{}", self.fresh());
        let _ = writeln!(self.globals, "{} = private unnamed_addr constant {} {{ ptr {}, {} {}, {} 0 }}", n, ty, data, u, s.len(), u);
        self.string_values.insert(s.to_vec(), n.clone());
        n
    }

    /// A pointer to the constant `Location` of `span` (or `null`).
    pub fn location(&mut self, span: Span) -> String {
        if span.is_dummy() {
            return "null".into();
        }
        let l = self.sm.location(span);
        let key = (span.file.0 as usize, l.line, l.col);
        if let Some(n) = self.locs.get(&key) {
            return n.clone();
        }
        let file = self.bytes_global(l.file.as_bytes());
        let lid = self.lang_adt("location");
        let lt = self.tcx().adt(lid, vec![]);
        let ty = self.llty(lt);
        let u = self.usize_ty();
        let n = format!("@loc.{}", self.fresh());
        let _ = writeln!(self.globals, "{} = private unnamed_addr constant {} {{ ptr {}, {} {}, i32 {}, i32 {} }}", n, ty, file, u, l.file.len(), l.line, l.col);
        self.locs.insert(key, n.clone());
        n
    }

    /// A pointer to a constant `FormatSpec` (or `null`).
    pub fn spec(&mut self, spec: &Option<FormatSpec>) -> String {
        let Some(sp) = spec else { return "null".into() };
        let align = match sp.align {
            None => 0,
            Some(Align::Left) => 1,
            Some(Align::Center) => 2,
            Some(Align::Right) => 3,
        };
        let kind = match sp.kind {
            None => 0,
            Some(FormatKind::LowerHex) => 1,
            Some(FormatKind::UpperHex) => 2,
            Some(FormatKind::Octal) => 3,
            Some(FormatKind::Binary) => 4,
            Some(FormatKind::Exponent) => 5,
        };
        let sid = self.lang_adt("format_spec");
        let st = self.tcx().adt(sid, vec![]);
        let ty = self.llty(st);
        let init = format!(
            "{} {{ i32 {}, i8 {}, i1 {}, i1 {}, i32 {}, i32 {}, i8 {} }}",
            ty,
            sp.fill as u32,
            align,
            sp.plus,
            sp.zero,
            sp.width.map(|w| w as i64).unwrap_or(-1),
            sp.precision.map(|w| w as i64).unwrap_or(-1),
            kind
        );
        if let Some(n) = self.specs.get(&init) {
            return n.clone();
        }
        let n = format!("@spec.{}", self.fresh());
        let _ = writeln!(self.globals, "{} = private unnamed_addr constant {}", n, init);
        self.specs.insert(init, n.clone());
        n
    }

    /// The global variable of a `static`.
    pub fn static_global(&mut self, st: StaticId) -> String {
        if let Some(n) = self.statics.get(&st) {
            return n.clone();
        }
        let sd = &self.p.statics[st.idx()];
        let module = &self.p.modules[sd.module.idx()].path;
        let n = format!("@{}", quote_name(&format!("{}.{}#{}", module, sd.name, self.fresh())));
        self.statics.insert(st, n.clone());
        let ty = sd.ty;
        let llt = self.llty(ty);
        let init = match self.p.static_values.get(&st).cloned() {
            Some(v) => self.static_init(&v, ty),
            None => "zeroinitializer".into(),
        };
        let kind = if sd.mutable { "global" } else { "constant" };
        let align = self.align_of(ty);
        let mut dbg = String::new();
        if self.di.is_some() {
            let span = sd.span;
            let file = self.sm.try_get(span.file).map(|f| f.name.clone()).unwrap_or_default();
            let line = self.sm.location(span).line as u32;
            let root = self.opts.root_dir.clone();
            let name = sd.name.clone();
            let di = self.di.as_mut().unwrap();
            let f = di.file(&file, &root);
            let tyd = di.ty(&self.lx, self.p, ty);
            let g = di.global_variable(&name, f, line, tyd);
            dbg = format!(", !dbg !{}", g);
        }
        let _ = writeln!(self.globals, "{} = internal {} {} {}, align {}{}", n, kind, llt, init, align, dbg);
        n
    }

    fn static_init(&mut self, v: &StaticValue, t: Ty) -> String {
        let tcx = self.tcx();
        match v {
            StaticValue::Null => "null".into(),
            StaticValue::Scalar(c) => self.const_scalar(c, t),
            StaticValue::Repeat(inner, n) => {
                let et = match &*tcx.kind(t) {
                    TyKind::Array(e, _) => *e,
                    _ => return "zeroinitializer".into(),
                };
                if matches!(**inner, StaticValue::Null) || matches!(&**inner, StaticValue::Scalar(ConstValue::Int(0)) | StaticValue::Scalar(ConstValue::Bool(false))) {
                    return "zeroinitializer".into();
                }
                if let StaticValue::Scalar(ConstValue::Float(x)) = &**inner {
                    if *x == 0.0 && x.is_sign_positive() {
                        return "zeroinitializer".into();
                    }
                }
                let ety = self.llty(et);
                let e = self.static_init(inner, et);
                let parts: Vec<String> = (0..*n).map(|_| format!("{} {}", ety, e)).collect();
                format!("[{}]", parts.join(", "))
            }
            StaticValue::Aggregate(vals) => match &*tcx.kind(t) {
                TyKind::Array(e, _) => {
                    let ety = self.llty(*e);
                    let parts: Vec<String> = vals.iter().map(|v| format!("{} {}", ety, self.static_init(v, *e))).collect();
                    format!("[{}]", parts.join(", "))
                }
                TyKind::Tuple(ts) => {
                    let ts = ts.clone();
                    let parts: Vec<String> = vals.iter().zip(ts.iter()).map(|(v, &et)| format!("{} {}", self.llty(et), self.static_init(v, et))).collect();
                    format!("{{ {} }}", parts.join(", "))
                }
                TyKind::Adt(id, args) => {
                    let fts: Vec<Ty> = self.p.adt(*id).fields().iter().map(|f| tcx.subst(f.ty, args)).collect();
                    let parts: Vec<String> = vals.iter().zip(fts.iter()).map(|(v, &et)| format!("{} {}", self.llty(et), self.static_init(v, et))).collect();
                    format!("{{ {} }}", parts.join(", "))
                }
                _ => "zeroinitializer".into(),
            },
            StaticValue::Variant(i) => {
                let s = self.lx.enum_shape(t);
                if s.has_payload {
                    let a = s.payload_align.min(8);
                    format!("{{ i32 {}, [{} x i{}] zeroinitializer }}", i, s.payload_size / a, a * 8)
                } else {
                    format!("{{ i32 {} }}", i)
                }
            }
        }
    }

    /// A scalar constant of type `t`.
    pub fn const_scalar(&mut self, v: &ConstValue, t: Ty) -> String {
        let tcx = self.tcx();
        match (v, &*tcx.kind(t)) {
            (ConstValue::Int(n), TyKind::Int(i)) => int_const(*n, self.int_bits(*i)),
            (ConstValue::Int(n), TyKind::Float(f)) => float_const(*n as f64, *f),
            (ConstValue::Float(x), TyKind::Float(f)) => float_const(*x, *f),
            (ConstValue::Bool(b), _) => if *b { "true".into() } else { "false".into() },
            (ConstValue::Char(c), _) => (*c as u32).to_string(),
            (ConstValue::Int(n), TyKind::Char) => n.to_string(),
            (ConstValue::Int(n), _) => n.to_string(),
            (ConstValue::Float(x), _) => float_const(*x, FloatTy::F64),
            (ConstValue::Unit, _) => "zeroinitializer".into(),
            (ConstValue::Str(_), _) => panic!("internal: string constant used as a scalar"),
        }
    }

    // -----------------------------------------------------------------------
    // Drop glue

    /// The drop function of a type (`None` if dropping does nothing).
    pub fn drop_fn(&mut self, t: Ty) -> Option<String> {
        if !self.p.needs_drop(t) {
            return None;
        }
        if let Some(n) = self.drop_names.get(&t) {
            return Some(n.clone());
        }
        let n = format!("@{}", quote_name(&format!("drop.{}#{}", self.p.ty_str(t), self.fresh())));
        self.drop_names.insert(t, n.clone());
        self.drop_queue.push(t);
        Some(n)
    }

    fn emit_drop_glue(&mut self, t: Ty) {
        let tcx = self.tcx();
        let name = self.drop_names[&t].clone();
        let lt = self.llty(t);
        let mut b = String::new();
        let mut k = 0usize;
        let tmp = |k: &mut usize| {
            *k += 1;
            format!("%d{}", k)
        };
        let free = self.lang_fn("free").0;
        match &*tcx.kind(t) {
            TyKind::Tuple(elems) => {
                for (i, e) in elems.clone().iter().enumerate() {
                    if let Some(d) = self.drop_fn(*e) {
                        let a = tmp(&mut k);
                        let _ = writeln!(b, "  {} = getelementptr inbounds {}, ptr %p, i32 0, i32 {}", a, lt, i);
                        let _ = writeln!(b, "  call void {}(ptr {})", d, a);
                    }
                }
            }
            TyKind::Array(e, n) => {
                if let Some(d) = self.drop_fn(*e) {
                    let u = self.usize_ty();
                    // Elements are destroyed by increasing index (spec §5.5).
                    let _ = writeln!(b, "  br label %loop");
                    let _ = writeln!(b, "loop:");
                    let _ = writeln!(b, "  %i = phi {} [ 0, %start ], [ %next, %loop ]", u);
                    let _ = writeln!(b, "  %a = getelementptr inbounds {}, ptr %p, {} 0, {} %i", lt, u, u);
                    let _ = writeln!(b, "  call void {}(ptr %a)", d);
                    let _ = writeln!(b, "  %next = add {} %i, 1", u);
                    let _ = writeln!(b, "  %done = icmp eq {} %next, {}", u, n);
                    let _ = writeln!(b, "  br i1 %done, label %end, label %loop");
                    let _ = writeln!(b, "end:");
                }
            }
            TyKind::Adt(id, args) => {
                let adt = self.p.adt(*id);
                if let Some(df) = adt.drop_fn {
                    let inst = self.instance(df, args.clone());
                    let _ = writeln!(b, "  call void {}(ptr %p)", inst);
                }
                match &adt.kind {
                    AdtKind::Struct { fields } => {
                        for (i, f) in fields.iter().enumerate() {
                            let ft = tcx.subst(f.ty, args);
                            let d = self.drop_fn(ft);
                            let a = tmp(&mut k);
                            let _ = writeln!(b, "  {} = getelementptr inbounds {}, ptr %p, i32 0, i32 {}", a, lt, i);
                            if f.indirect {
                                let bx = tmp(&mut k);
                                let _ = writeln!(b, "  {} = load ptr, ptr {}", bx, a);
                                if let Some(d) = d {
                                    let _ = writeln!(b, "  call void {}(ptr {})", d, bx);
                                }
                                let _ = writeln!(b, "  call void {}(ptr {})", free, bx);
                            } else if let Some(d) = d {
                                let _ = writeln!(b, "  call void {}(ptr {})", d, a);
                            }
                        }
                    }
                    AdtKind::Enum { variants } => {
                        let tagp = tmp(&mut k);
                        let tag = tmp(&mut k);
                        let _ = writeln!(b, "  {} = getelementptr inbounds {}, ptr %p, i32 0, i32 0", tagp, lt);
                        let _ = writeln!(b, "  {} = load i32, ptr {}", tag, tagp);
                        let mut cases = String::new();
                        let mut blocks = String::new();
                        for (vi, v) in variants.iter().enumerate() {
                            if v.fields.is_empty() {
                                continue;
                            }
                            let pty = self.payload_ty(t, vi as u32);
                            let mut case = String::new();
                            let pay = tmp(&mut k);
                            let _ = writeln!(case, "  {} = getelementptr inbounds {}, ptr %p, i32 0, i32 1", pay, lt);
                            let base = if v.indirect {
                                let bx = tmp(&mut k);
                                let _ = writeln!(case, "  {} = load ptr, ptr {}", bx, pay);
                                bx
                            } else {
                                pay
                            };
                            let mut any = v.indirect;
                            for (fi, &f) in v.fields.iter().enumerate() {
                                let ft = tcx.subst(f, args);
                                if let Some(d) = self.drop_fn(ft) {
                                    any = true;
                                    let a = tmp(&mut k);
                                    let _ = writeln!(case, "  {} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}", a, pty, base, fi);
                                    let _ = writeln!(case, "  call void {}(ptr {})", d, a);
                                }
                            }
                            if v.indirect {
                                let _ = writeln!(case, "  call void {}(ptr {})", free, base);
                            }
                            if any {
                                let _ = writeln!(cases, "    i32 {}, label %v{}", vi, vi);
                                let _ = write!(blocks, "v{}:\n{}  br label %end\n", vi, case);
                            }
                        }
                        let _ = writeln!(b, "  switch i32 {}, label %end [\n{}  ]", tag, cases);
                        b.push_str(&blocks);
                        b.push_str("end:\n");
                    }
                }
            }
            TyKind::Dyn(_) => {
                // The concrete value's destructor (first entry of the vtable), then
                // its heap storage.
                let _ = writeln!(b, "  %data = load ptr, ptr %p");
                let _ = writeln!(b, "  %vp = getelementptr inbounds {{ ptr, ptr }}, ptr %p, i32 0, i32 1");
                let _ = writeln!(b, "  %vt = load ptr, ptr %vp");
                let _ = writeln!(b, "  %dropf = load ptr, ptr %vt");
                let _ = writeln!(b, "  %has = icmp ne ptr %dropf, null");
                let _ = writeln!(b, "  br i1 %has, label %drop, label %free");
                let _ = writeln!(b, "drop:");
                let _ = writeln!(b, "  call void %dropf(ptr %data)");
                let _ = writeln!(b, "  br label %free");
                let _ = writeln!(b, "free:");
                let _ = writeln!(b, "  call void {}(ptr %data)", free);
            }
            _ => {}
        }
        let _ = write!(self.fn_defs, "define internal void {}(ptr %p) nounwind {{\nstart:\n{}  ret void\n}}\n\n", name, b);
    }

    /// The methods of a `dyn Trait` vtable, in order: those of the trait, then of its
    /// supertraits.
    pub fn vtable_methods(&self, tr: TraitId) -> Vec<(TraitId, String, FnId)> {
        let mut v = Vec::new();
        for t in self.p.trait_closure(tr) {
            for m in &self.p.traits[t.idx()].methods {
                v.push((t, m.name.clone(), m.decl));
            }
        }
        v
    }

    /// The vtable of `ty` as a `dyn tr`: destructor, size, alignment, then the methods.
    pub fn vtable(&mut self, tr: TraitId, ty: Ty) -> String {
        if let Some(n) = self.vtables.get(&(tr, ty)) {
            return n.clone();
        }
        let name = format!("@{}", quote_name(&format!("vtable.{}.{}#{}", self.p.traits[tr.idx()].name, self.p.ty_str(ty), self.fresh())));
        self.vtables.insert((tr, ty), name.clone());
        let u = self.usize_ty();
        let mut entries = vec![match self.drop_fn(ty) {
            Some(d) => format!("ptr {}", d),
            None => "ptr null".to_string(),
        }];
        entries.push(format!("{} {}", u, self.size_of(ty)));
        entries.push(format!("{} {}", u, self.align_of(ty)));
        let mut types = vec!["ptr".to_string(), u.to_string(), u.to_string()];
        for (_, _, decl) in self.vtable_methods(tr) {
            let inst = self.instance(decl, vec![ty]);
            entries.push(format!("ptr {}", inst));
            types.push("ptr".into());
        }
        let _ = writeln!(self.globals, "{} = internal constant {{ {} }} {{ {} }}", name, types.join(", "), entries.join(", "));
        name
    }

    // -----------------------------------------------------------------------
    // Entry points

    fn emit_entry(&mut self, entry: &Entry) {
        let main_ptr = match entry {
            Entry::Library => return,
            Entry::Main(f) => self.instance(*f, vec![]),
            Entry::Tests(tests) => {
                let (index_fn, _) = self.lang_fn("test_index");
                let mut cases = String::new();
                let mut blocks = String::new();
                for (i, t) in tests.iter().enumerate() {
                    let name = self.instance(*t, vec![]);
                    let _ = writeln!(cases, "    i32 {}, label %t{}", i, i);
                    let _ = write!(blocks, "t{}:\n  call void {}()\n  ret void\n", i, name);
                }
                let _ = write!(
                    self.fn_defs,
                    "define internal void @nx.tests() nounwind {{\nstart:\n  %i = call i32 {}()\n  switch i32 %i, label %none [\n{}  ]\n{}none:\n  ret void\n}}\n\n",
                    index_fn, cases, blocks
                );
                "@nx.tests".to_string()
            }
        };
        let (entry_fn, _) = self.lang_fn("entry");
        let target = self.opts.target.clone();
        match target.os {
            Os::Windows => {
                let (set_handler, _) = self.lang_fn("sys_set_fault_handler");
                let (fault, _) = self.lang_fn("sys_fault");
                let _ = write!(self.fn_defs, "define i32 @nx_windows_fault(ptr %exception) nounwind {{\nstart:\n  %r = call i32 {}(ptr %exception)\n  ret i32 %r\n}}\n\n", fault);
                let _ = write!(self.fn_defs, "define void @mainCRTStartup() nounwind {{\nstart:\n  call void {}(ptr @nx_windows_fault)\n  call void {}(ptr {})\n  unreachable\n}}\n\n", set_handler, entry_fn, main_ptr);
            }
            Os::MacOs => {
                // The dynamic loader calls `main(argc, argv, envp)`; memory faults reach the
                // runtime through a C entry, on an alternate stack (ADR 0011, T2).
                let (set_args, _) = self.lang_fn("sys_set_args");
                let (set_handler, _) = self.lang_fn("sys_set_fault_handler");
                let (fault, _) = self.lang_fn("sys_fault");
                let _ = write!(
                    self.fn_defs,
                    "define void @nx_macos_fault(i32 %sig, ptr %info, ptr %ctx) nounwind {{\nstart:\n  call void {}(i32 %sig, ptr %info)\n  unreachable\n}}\n\n",
                    fault
                );
                let _ = write!(
                    self.fn_defs,
                    "define i32 @main(i32 %argc, ptr %argv, ptr %envp) nounwind {{\nstart:\n  call void {}(i32 %argc, ptr %argv, ptr %envp)\n  call void {}(ptr @nx_macos_fault)\n  call void {}(ptr {})\n  unreachable\n}}\n\n",
                    set_args, set_handler, entry_fn, main_ptr
                );
            }
            Os::Linux => {
                let (set_stack, _) = self.lang_fn("sys_set_stack");
                // Memory faults (SIGSEGV, SIGBUS) reach the runtime through a C entry, on
                // an alternate stack: a stack overflow ends like a panic (ADR 0011, T2).
                let (set_handler, _) = self.lang_fn("sys_set_fault_handler");
                let (fault, _) = self.lang_fn("sys_fault");
                let _ = write!(
                    self.fn_defs,
                    "define void @nx_linux_fault(i32 %sig, ptr %info, ptr %ctx) nounwind {{\nstart:\n  call void {}(i32 %sig, ptr %info)\n  unreachable\n}}\n\n",
                    fault
                );
                let _ = write!(
                    self.fn_defs,
                    "define void @nx_linux_start(ptr %sp) nounwind {{\nstart:\n  call void {}(ptr %sp)\n  call void {}(ptr @nx_linux_fault)\n  call void {}(ptr {})\n  unreachable\n}}\n\n",
                    set_stack, set_handler, entry_fn, main_ptr
                );
                let asm = match target.arch {
                    Arch::X86_64 => ".text\n.globl _start\n.type _start,@function\n_start:\n  xorl %ebp, %ebp\n  movq %rsp, %rdi\n  andq $-16, %rsp\n  callq nx_linux_start\n  hlt\n",
                    Arch::Aarch64 => ".text\n.globl _start\n.type _start,%function\n_start:\n  mov x29, #0\n  mov x30, #0\n  mov x0, sp\n  and x1, x0, #-16\n  mov sp, x1\n  bl nx_linux_start\n  brk #0\n",
                    _ => ".text\n.globl _start\n.type _start,@function\n_start:\n  mv a0, sp\n  andi sp, sp, -16\n  call nx_linux_start\n  ebreak\n",
                };
                self.module_asm.push(asm.to_string());
            }
            Os::Wasi | Os::Web => {
                let _ = write!(self.fn_defs, "define void @_start() nounwind {{\nstart:\n  call void {}(ptr {})\n  unreachable\n}}\n\n", entry_fn, main_ptr);
            }
            Os::None => {
                // Bare metal: the startup code prepares the memory, then the program runs.
                let (init, _) = self.lang_fn("sys_reset");
                // Paused in a call while the tasks run (ADR 0018 §6).
                self.entry_chain = ["@nx_vector_entry", "@nx_start_c", "@nx_reset", entry_fn.as_str(), main_ptr.as_str()].iter().map(|n| symbol_of(n)).collect();
                let _ = write!(
                    self.fn_defs,
                    "define void @nx_reset() nounwind noreturn {{\nstart:\n  call void {}()\n  call void {}(ptr {})\n  unreachable\n}}\n\n",
                    init, entry_fn, main_ptr
                );
                self.emit_bare_startup();
            }
        }
    }

    /// Startup code of bare-metal targets (spec §7.1): vector table (Cortex-M) or entry
    /// point, initialized data, zeroed `.bss`, floating-point unit.
    fn emit_bare_startup(&mut self) {
        let u = self.usize_ty();
        for sym in ["__stack_top", "__data_start", "__data_end", "__data_load", "__bss_start", "__bss_end"] {
            self.declare(&format!("@{} = external global i8", quote_name(sym)));
        }
        let mut fpu = String::new();
        if self.opts.target.arch == Arch::Arm && self.opts.target.features.contains("vfp") {
            // CPACR (0xE000ED88): full access to the coprocessors 10 and 11 (the FPU).
            fpu.push_str("  %cpacr = inttoptr i32 3758157192 to ptr\n  %c = load volatile i32, ptr %cpacr\n  %c2 = or i32 %c, 15728640\n  store volatile i32 %c2, ptr %cpacr\n  fence seq_cst\n");
        }
        // With real-time tasks, the stack is guarded by the MPU from the start (ADR 0018 §6).
        let guard = if self.p.rt.is_some() && self.opts.target.arch == Arch::Arm { "  call void @nx_rt_guard()\n" } else { "" };
        // Copies `.data` from the flash to the RAM and clears `.bss`.
        let _ = write!(
            self.fn_defs,
            "define void @nx_start_c() nounwind noreturn {{
start:
{fpu}  %ds = ptrtoint ptr @__data_start to {u}
  %de = ptrtoint ptr @__data_end to {u}
  %dn = sub {u} %de, %ds
  %r1 = call ptr @memcpy(ptr @__data_start, ptr @__data_load, {u} %dn)
  %bs = ptrtoint ptr @__bss_start to {u}
  %be = ptrtoint ptr @__bss_end to {u}
  %bn = sub {u} %be, %bs
  %r2 = call ptr @memset(ptr @__bss_start, i32 0, {u} %bn)
{guard}  call void @nx_reset()
  unreachable
}}

"
        );
        let exported: Vec<String> = self.p.fns.iter().filter_map(|f| f.export.clone()).collect();
        match self.opts.target.arch {
            Arch::Arm => {
                // Cortex-M: initial stack pointer, reset handler, the exceptions, then 32
                // interrupts. A program handles one with `@export("SysTick_Handler")`.
                let mut names: Vec<String> = ["NMI_Handler", "HardFault_Handler", "MemManage_Handler", "BusFault_Handler", "UsageFault_Handler", "", "", "", "", "SVC_Handler", "DebugMon_Handler", "", "PendSV_Handler", "SysTick_Handler"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                names.extend((0..32).map(|i| format!("IRQ{}_Handler", i)));
                // The scheduler of the real-time tasks takes SysTick and its lines (ADR 0018).
                let scheduler = self.emit_rt();
                let mut entries = vec!["ptr @__stack_top".to_string(), "ptr @nx_vector_entry".to_string()];
                let mut defaults = String::new();
                for n in &names {
                    if n.is_empty() {
                        entries.push("ptr null".into());
                        continue;
                    }
                    if let Some((_, h)) = scheduler.iter().find(|(v, _)| v == n) {
                        entries.push(format!("ptr @{}", h));
                        continue;
                    }
                    entries.push(format!("ptr @{}", n));
                    if !exported.contains(n) {
                        let _ = writeln!(defaults, "define internal void @{}() nounwind {{\nstart:\n  br label %spin\nspin:\n  br label %spin\n}}\n", n);
                    }
                }
                let _ = writeln!(self.globals, "@nx.vectors = internal constant [{} x ptr] [{}], section \".vectors\", align 4", entries.len(), entries.join(", "));
                let _ = writeln!(self.globals, "@llvm.used = appending global [1 x ptr] [ptr @nx.vectors], section \"llvm.metadata\"");
                self.fn_defs.push_str(&defaults);
                self.fn_defs.push_str("define void @nx_vector_entry() nounwind noreturn {\nstart:\n  call void @nx_start_c()\n  unreachable\n}\n\n");
            }
            Arch::Aarch64 => {
                self.module_asm.push(
                    ".section .vectors, \"ax\"\n.globl nx_vector_entry\nnx_vector_entry:\n  ldr x0, =__stack_top\n  mov sp, x0\n  mov x0, #(3 << 20)\n  msr cpacr_el1, x0\n  isb\n  bl nx_start_c\n1:\n  wfe\n  b 1b\n.ltorg\n.text\n".to_string(),
                );
            }
            _ => {
                // With the F/D extensions, the floating-point unit stays off until
                // `mstatus.FS` is set.
                let fpu = if self.opts.target.hard_float64 { "  li t0, 0x2000\n  csrs mstatus, t0\n" } else { "" };
                self.module_asm.push(format!(
                    ".section .vectors, \"ax\"\n.globl nx_vector_entry\nnx_vector_entry:\n  la sp, __stack_top\n{}  call nx_start_c\n1:\n  j 1b\n.text\n",
                    fpu
                ));
            }
        }
    }

    // -----------------------------------------------------------------------
    // Builtins: functions that LLVM itself may call (memory copies, stack probes).

    fn emit_builtins(&mut self) {
        let u = self.usize_ty();
        let attrs = "nounwind \"no-builtins\"";
        // memcpy / memmove / memset / memcmp / bcmp, written so that LLVM cannot turn
        // them back into calls to themselves ("no-builtins").
        let _ = write!(
            self.fn_defs,
            "define weak ptr @memcpy(ptr %d, ptr %s, {u} %n) {attrs} {{
start:
  %z = icmp eq {u} %n, 0
  br i1 %z, label %done, label %loop
loop:
  %i = phi {u} [ 0, %start ], [ %i1, %loop ]
  %sp = getelementptr i8, ptr %s, {u} %i
  %b = load i8, ptr %sp
  %dp = getelementptr i8, ptr %d, {u} %i
  store i8 %b, ptr %dp
  %i1 = add {u} %i, 1
  %c = icmp eq {u} %i1, %n
  br i1 %c, label %done, label %loop
done:
  ret ptr %d
}}

define weak ptr @memmove(ptr %d, ptr %s, {u} %n) {attrs} {{
start:
  %z = icmp eq {u} %n, 0
  br i1 %z, label %done, label %dir
dir:
  %di = ptrtoint ptr %d to {u}
  %si = ptrtoint ptr %s to {u}
  %fwd = icmp ule {u} %di, %si
  br i1 %fwd, label %floop, label %bloop
floop:
  %i = phi {u} [ 0, %dir ], [ %i1, %floop ]
  %sp = getelementptr i8, ptr %s, {u} %i
  %b = load i8, ptr %sp
  %dp = getelementptr i8, ptr %d, {u} %i
  store i8 %b, ptr %dp
  %i1 = add {u} %i, 1
  %c = icmp eq {u} %i1, %n
  br i1 %c, label %done, label %floop
bloop:
  %j = phi {u} [ %n, %dir ], [ %j1, %bloop ]
  %j1 = sub {u} %j, 1
  %sp2 = getelementptr i8, ptr %s, {u} %j1
  %b2 = load i8, ptr %sp2
  %dp2 = getelementptr i8, ptr %d, {u} %j1
  store i8 %b2, ptr %dp2
  %c2 = icmp eq {u} %j1, 0
  br i1 %c2, label %done, label %bloop
done:
  ret ptr %d
}}

define weak ptr @memset(ptr %d, i32 %v, {u} %n) {attrs} {{
start:
  %z = icmp eq {u} %n, 0
  %b = trunc i32 %v to i8
  br i1 %z, label %done, label %loop
loop:
  %i = phi {u} [ 0, %start ], [ %i1, %loop ]
  %dp = getelementptr i8, ptr %d, {u} %i
  store i8 %b, ptr %dp
  %i1 = add {u} %i, 1
  %c = icmp eq {u} %i1, %n
  br i1 %c, label %done, label %loop
done:
  ret ptr %d
}}

define weak i32 @memcmp(ptr %a, ptr %b, {u} %n) {attrs} {{
start:
  %z = icmp eq {u} %n, 0
  br i1 %z, label %equal, label %loop
loop:
  %i = phi {u} [ 0, %start ], [ %i1, %next ]
  %ap = getelementptr i8, ptr %a, {u} %i
  %x = load i8, ptr %ap
  %bp = getelementptr i8, ptr %b, {u} %i
  %y = load i8, ptr %bp
  %ne = icmp ne i8 %x, %y
  br i1 %ne, label %diff, label %next
next:
  %i1 = add {u} %i, 1
  %c = icmp eq {u} %i1, %n
  br i1 %c, label %equal, label %loop
diff:
  %xe = zext i8 %x to i32
  %ye = zext i8 %y to i32
  %r = sub i32 %xe, %ye
  ret i32 %r
equal:
  ret i32 0
}}

define weak i32 @bcmp(ptr %a, ptr %b, {u} %n) {attrs} {{
start:
  %r = call i32 @memcmp(ptr %a, ptr %b, {u} %n)
  ret i32 %r
}}

"
        );
        let target = self.opts.target.clone();
        if !target.native_div64 {
            self.emit_div64_builtins();
        }
        if target.os == Os::Windows {
            // Floating-point marker expected by MSVC-style objects, and the stack probe
            // called for frames larger than a page.
            let _ = writeln!(self.globals, "@_fltused = global i32 0");
            if target.arch == Arch::X86_64 {
                self.module_asm.push(
                    ".text\n.globl __chkstk\n__chkstk:\n  pushq %rcx\n  pushq %rax\n  cmpq $0x1000, %rax\n  leaq 24(%rsp), %rcx\n  jb 2f\n1:\n  subq $0x1000, %rcx\n  orq $0, (%rcx)\n  subq $0x1000, %rax\n  cmpq $0x1000, %rax\n  ja 1b\n2:\n  subq %rax, %rcx\n  orq $0, (%rcx)\n  popq %rax\n  popq %rcx\n  retq\n"
                        .to_string(),
                );
            }
        }
    }

    /// 64-bit division for 32-bit processors (the functions LLVM calls for `i64 / i64`),
    /// by shifts and subtractions on 64-bit values that the processor handles natively.
    /// The floating-point functions of the ARM run-time ABI (`__aeabi_dadd`…), which C
    /// compilers call for the operations the processor lacks: weak aliases of the
    /// functions of `std.builtins`. The run-time ABI passes their arguments in core
    /// registers even on a hard-float target (`arm_aapcscc`), so each converts.
    fn emit_aeabi_float(&mut self) {
        // (name, function of std.builtins, result, parameters, how the result is made)
        let d = "double";
        let f = "float";
        let table: &[(&str, &str, &str, &[&str], &str)] = &[
            ("dadd", "__adddf3", d, &[d, d], ""),
            ("dsub", "__subdf3", d, &[d, d], ""),
            ("drsub", "__subdf3", d, &[d, d], "swap"),
            ("dmul", "__muldf3", d, &[d, d], ""),
            ("ddiv", "__divdf3", d, &[d, d], ""),
            ("dneg", "__negdf2", d, &[d], ""),
            ("dcmpeq", "__eqdf2", "i32", &[d, d], "eq"),
            ("dcmplt", "__ltdf2", "i32", &[d, d], "slt"),
            ("dcmple", "__ledf2", "i32", &[d, d], "sle"),
            ("dcmpge", "__gedf2", "i32", &[d, d], "sge"),
            ("dcmpgt", "__gtdf2", "i32", &[d, d], "sgt"),
            ("dcmpun", "__unorddf2", "i32", &[d, d], "ne"),
            ("d2iz", "__fixdfsi", "i32", &[d], ""),
            ("d2uiz", "__fixunsdfsi", "i32", &[d], ""),
            ("d2lz", "__fixdfdi", "i64", &[d], ""),
            ("d2ulz", "__fixunsdfdi", "i64", &[d], ""),
            ("i2d", "__floatsidf", d, &["i32"], ""),
            ("ui2d", "__floatunsidf", d, &["i32"], ""),
            ("l2d", "__floatdidf", d, &["i64"], ""),
            ("ul2d", "__floatundidf", d, &["i64"], ""),
            ("f2d", "__extendsfdf2", d, &[f], ""),
            ("d2f", "__truncdfsf2", f, &[d], ""),
            ("fadd", "__addsf3", f, &[f, f], ""),
            ("fsub", "__subsf3", f, &[f, f], ""),
            ("frsub", "__subsf3", f, &[f, f], "swap"),
            ("fmul", "__mulsf3", f, &[f, f], ""),
            ("fdiv", "__divsf3", f, &[f, f], ""),
            ("fneg", "__negsf2", f, &[f], ""),
            ("fcmpeq", "__eqsf2", "i32", &[f, f], "eq"),
            ("fcmplt", "__ltsf2", "i32", &[f, f], "slt"),
            ("fcmple", "__lesf2", "i32", &[f, f], "sle"),
            ("fcmpge", "__gesf2", "i32", &[f, f], "sge"),
            ("fcmpgt", "__gtsf2", "i32", &[f, f], "sgt"),
            ("fcmpun", "__unordsf2", "i32", &[f, f], "ne"),
            ("f2iz", "__fixsfsi", "i32", &[f], ""),
            ("f2uiz", "__fixunssfsi", "i32", &[f], ""),
            ("f2lz", "__fixsfdi", "i64", &[f], ""),
            ("f2ulz", "__fixunssfdi", "i64", &[f], ""),
            ("i2f", "__floatsisf", f, &["i32"], ""),
            ("ui2f", "__floatunsisf", f, &["i32"], ""),
            ("l2f", "__floatdisf", f, &["i64"], ""),
            ("ul2f", "__floatundisf", f, &["i64"], ""),
        ];
        for &(name, gnu, ret, params, how) in table {
            if !self.exports.iter().any(|e| e == gnu) {
                continue;
            }
            let decl: Vec<String> = params.iter().enumerate().map(|(i, t)| format!("{} %a{}", t, i)).collect();
            let mut args = decl.clone();
            if how == "swap" {
                args.swap(0, 1);
            }
            // The functions of std.builtins return a comparison as a sign (`__ltdf2` < 0
            // when a < b); the run-time ABI as 0 or 1.
            let (call_ret, body) = match how {
                "eq" | "slt" | "sle" | "sge" | "sgt" | "ne" => {
                    let pred = if how == "eq" { "eq" } else if how == "ne" { "ne" } else { how };
                    ("i32", format!("  %c = icmp {} i32 %r, 0\n  %b = zext i1 %c to i32\n  ret i32 %b\n", pred))
                }
                _ => (ret, format!("  ret {} %r\n", ret)),
            };
            let _ = write!(
                self.fn_defs,
                "define weak arm_aapcscc {ret} @__aeabi_{name}({}) nounwind {{\nstart:\n  %r = call {call_ret} @{gnu}({})\n{body}}}\n\n",
                decl.join(", "),
                args.join(", ")
            );
        }
    }

    fn emit_div64_builtins(&mut self) {
        self.fn_defs.push_str(
            "define internal {i64, i64} @nx.udivmod64(i64 %n, i64 %d) nounwind \"no-builtins\" {
start:
  br label %loop
loop:
  %i = phi i32 [ 63, %start ], [ %i1, %loop ]
  %q = phi i64 [ 0, %start ], [ %q2, %loop ]
  %r = phi i64 [ 0, %start ], [ %r2, %loop ]
  %sh = zext i32 %i to i64
  %bit = lshr i64 %n, %sh
  %b1 = and i64 %bit, 1
  %rs = shl i64 %r, 1
  %r1 = or i64 %rs, %b1
  %ge = icmp uge i64 %r1, %d
  %rsub = sub i64 %r1, %d
  %qb = shl i64 1, %sh
  %qor = or i64 %q, %qb
  %r2 = select i1 %ge, i64 %rsub, i64 %r1
  %q2 = select i1 %ge, i64 %qor, i64 %q
  %i1 = sub i32 %i, 1
  %done = icmp slt i32 %i1, 0
  br i1 %done, label %end, label %loop
end:
  %o0 = insertvalue {i64, i64} undef, i64 %q2, 0
  %o1 = insertvalue {i64, i64} %o0, i64 %r2, 1
  ret {i64, i64} %o1
}

define internal {i64, i64} @nx.sdivmod64(i64 %n, i64 %d) nounwind \"no-builtins\" {
start:
  %nn = icmp slt i64 %n, 0
  %dn = icmp slt i64 %d, 0
  %na = sub i64 0, %n
  %da = sub i64 0, %d
  %un = select i1 %nn, i64 %na, i64 %n
  %ud = select i1 %dn, i64 %da, i64 %d
  %u = call {i64, i64} @nx.udivmod64(i64 %un, i64 %ud)
  %q = extractvalue {i64, i64} %u, 0
  %r = extractvalue {i64, i64} %u, 1
  %qs = xor i1 %nn, %dn
  %qneg = sub i64 0, %q
  %rneg = sub i64 0, %r
  %q2 = select i1 %qs, i64 %qneg, i64 %q
  %r2 = select i1 %nn, i64 %rneg, i64 %r
  %o0 = insertvalue {i64, i64} undef, i64 %q2, 0
  %o1 = insertvalue {i64, i64} %o0, i64 %r2, 1
  ret {i64, i64} %o1
}

",
        );
        if self.opts.target.arch == Arch::Arm {
            // The ARM run-time ABI returns the quotient in r0:r1 and the remainder in r2:r3.
            self.fn_defs.push_str(
                "define {i64, i64} @__aeabi_uldivmod(i64 %n, i64 %d) nounwind \"no-builtins\" {
start:
  %r = call {i64, i64} @nx.udivmod64(i64 %n, i64 %d)
  ret {i64, i64} %r
}

define {i64, i64} @__aeabi_ldivmod(i64 %n, i64 %d) nounwind \"no-builtins\" {
start:
  %r = call {i64, i64} @nx.sdivmod64(i64 %n, i64 %d)
  ret {i64, i64} %r
}

",
            );
        }
        if self.opts.target.arch == Arch::Arm {
            self.emit_aeabi_float();
        }
        {
            for (name, f, idx) in [("__udivdi3", "udivmod64", 0), ("__umoddi3", "udivmod64", 1), ("__divdi3", "sdivmod64", 0), ("__moddi3", "sdivmod64", 1)] {
                let _ = write!(
                    self.fn_defs,
                    "define i64 @{}(i64 %n, i64 %d) nounwind \"no-builtins\" {{\nstart:\n  %r = call {{i64, i64}} @nx.{}(i64 %n, i64 %d)\n  %v = extractvalue {{i64, i64}} %r, {}\n  ret i64 %v\n}}\n\n",
                    name, f, idx
                );
            }
        }
    }

    fn finish(mut self) -> Output {
        if let Some(spec) = &mut self.stack {
            let tcx = &self.p.tcx;
            for ((f, _), name) in &self.instances {
                if matches!(&*tcx.kind(self.p.func(*f).ret), TyKind::Never) {
                    spec.stops.push(symbol_of(name));
                }
            }
            spec.stops.sort();
        }
        let mut out = String::new();
        out.push_str("; Generated by nxc, the NEXUL compiler. Do not edit.\n");
        let _ = writeln!(out, "target triple = \"{}\"\n", self.opts.target.llvm_triple);
        for asm in &self.module_asm {
            for line in asm.lines() {
                let _ = writeln!(out, "module asm \"{}\"", line.replace('\\', "\\5C").replace('"', "\\22"));
            }
        }
        out.push('\n');
        out.push_str(&self.type_defs);
        out.push('\n');
        out.push_str(&self.globals);
        out.push('\n');
        out.push_str(&self.decls);
        out.push('\n');
        // Apply this to every definition, including drop glue, ABI shims and compiler
        // helpers: those can acquire large frames after inlining too. Declarations
        // and module assembly are deliberately excluded. Windows uses __chkstk.
        if matches!(self.opts.target.os, Os::Linux | Os::MacOs)
            && matches!(self.opts.target.arch, Arch::X86_64 | Arch::Aarch64)
        {
            for line in self.fn_defs.lines() {
                if line.starts_with("define ") {
                    // Function attributes precede debug metadata. All definitions
                    // have their complete signature on one line, ending in ` {`.
                    let at = line.find(" !dbg ").unwrap_or_else(|| line.rfind(" {").expect("function header"));
                    let _ = writeln!(out, "{} #0{}", &line[..at], &line[at..]);
                } else {
                    let _ = writeln!(out, "{}", line);
                }
            }
            out.push_str("\nattributes #0 = { \"probe-stack\"=\"inline-asm\" }\n");
        } else {
            out.push_str(&self.fn_defs);
        }
        if let Some(di) = self.di.take() {
            out.push_str(&di.module_tail(&[]));
            out.push_str(&di.defs);
        }
        Output { ir: out, imports: self.imports, links: self.links, exports: self.exports, profile_names: self.profile_names, stack: self.stack }
    }
}

#[allow(dead_code)]
fn conv_by_ref(c: Convention) -> bool {
    c != Convention::Owned
}

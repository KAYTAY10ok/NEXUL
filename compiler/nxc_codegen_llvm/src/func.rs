//! Translation of one function body (NIR) into LLVM IR.
//!
//! Every local lives in an `alloca` (LLVM's `mem2reg` turns them into registers when
//! optimizing). The checks required by the language (overflow, division, shifts,
//! conversions, bounds) are explicit branches to the runtime's panic functions.

use crate::{float_const, int_const, quote_name, Gen, PKind, RetKind, Sig};
use nxc_nir::nir::*;
use nxc_sema::defs::{AdtKind, ConstValue, FnId, FnKind};
use nxc_sema::target::{Arch, Os};
use nxc_sema::thir::PrimOp;
use nxc_sema::ty::{FloatTy, IntTy, Ty, TyKind};
use nxc_syntax::ast::{BinOp, UnOp};
use nxc_syntax::Span;
use std::fmt::Write;

/// A value produced by an operand.
#[derive(Clone, Debug)]
pub(crate) enum Val {
    /// An SSA scalar: (LLVM type, value).
    Scalar(String, String),
    /// An aggregate in memory, by address.
    Mem(String),
    /// A value of a zero-sized type.
    Unit,
}

impl Val {
    fn scalar(&self) -> &str {
        match self {
            Val::Scalar(_, v) => v,
            other => panic!("internal: expected a scalar value, found {:?}", other),
        }
    }
}

/// Panic kinds of the runtime's `panic_arith` (see `std.rt`).
mod kind {
    pub const ADD: u32 = 0;
    pub const SUB: u32 = 1;
    pub const MUL: u32 = 2;
    pub const DIV_ZERO: u32 = 3;
    pub const DIV_OVERFLOW: u32 = 4;
    pub const REM_ZERO: u32 = 5;
    pub const REM_OVERFLOW: u32 = 6;
    pub const NEG: u32 = 7;
    pub const SHL: u32 = 8;
    pub const SHR: u32 = 9;
    pub const ABS: u32 = 10;
    pub const CAST: u32 = 11;
    pub const CAST_NAN: u32 = 12;
    pub const CHAR: u32 = 13;
    pub const POW: u32 = 14;
}

pub(crate) struct FnCx {
    targs: Vec<Ty>,
    tys: Vec<Ty>,
    addr: Vec<String>,
    out: String,
    allocas: String,
    n: usize,
    ret: RetKind,
    track_caller: bool,
    scope: Option<usize>,
    dbg: String,
}

impl<'a> Gen<'a> {
    // -----------------------------------------------------------------------
    // Instruction helpers

    fn ins(&mut self, cx: &mut FnCx, s: impl AsRef<str>) {
        let _ = writeln!(cx.out, "  {}{}", s.as_ref(), cx.dbg);
    }

    fn val(&mut self, cx: &mut FnCx, s: impl AsRef<str>) -> String {
        cx.n += 1;
        let t = format!("%t{}", cx.n);
        let _ = writeln!(cx.out, "  {} = {}{}", t, s.as_ref(), cx.dbg);
        t
    }

    fn new_label(&mut self, cx: &mut FnCx) -> String {
        cx.n += 1;
        format!("x{}", cx.n)
    }

    fn put_label(&mut self, cx: &mut FnCx, l: &str) {
        let _ = writeln!(cx.out, "{}:", l);
    }

    pub(crate) fn alloca(&mut self, cx: &mut FnCx, t: Ty) -> String {
        let lt = self.llty(t);
        let align = self.align_of(t);
        cx.n += 1;
        let name = format!("%s{}", cx.n);
        let _ = writeln!(cx.allocas, "  {} = alloca {}, align {}", name, lt, align);
        name
    }

    fn alloca_raw(&mut self, cx: &mut FnCx, lt: &str) -> String {
        cx.n += 1;
        let name = format!("%s{}", cx.n);
        let _ = writeln!(cx.allocas, "  {} = alloca {}", name, lt);
        name
    }

    fn set_loc(&mut self, cx: &mut FnCx, span: Span) {
        let Some(scope) = cx.scope else { return };
        if span.is_dummy() {
            return;
        }
        let l = self.sm.location(span);
        let id = self.di.as_mut().unwrap().location(scope, l.line as u32, l.col as u32);
        cx.dbg = format!(", !dbg !{}", id);
    }

    /// The location argument of a check: the caller's for `@track_caller` functions.
    fn loc(&mut self, cx: &FnCx, span: Span) -> String {
        if cx.track_caller {
            "%caller".into()
        } else {
            self.location(span)
        }
    }

    fn memcpy(&mut self, cx: &mut FnCx, dst: &str, src: &str, size: u64) {
        if size == 0 {
            return;
        }
        let u = self.usize_ty();
        self.declare(&format!("declare void @llvm.memcpy.p0.p0.{u}(ptr, ptr, {u}, i1)"));
        self.ins(cx, format!("call void @llvm.memcpy.p0.p0.{u}(ptr {}, ptr {}, {u} {}, i1 false)", dst, src, size));
    }

    /// Calls a runtime panic function and ends the current block.
    fn panic_arith(&mut self, cx: &mut FnCx, k: u32, span: Span) {
        let (f, _) = self.lang_fn("panic_arith");
        let loc = self.loc(cx, span);
        self.ins(cx, format!("call void {}(i32 {}, ptr {})", f, k, loc));
        self.ins(cx, "unreachable");
    }

    /// Branches to a panic when `cond` is true; continues in a new block otherwise.
    fn check(&mut self, cx: &mut FnCx, cond: &str, k: u32, span: Span) {
        let bad = self.new_label(cx);
        let ok = self.new_label(cx);
        self.ins(cx, format!("br i1 {}, label %{}, label %{}", cond, bad, ok));
        self.put_label(cx, &bad);
        self.panic_arith(cx, k, span);
        self.put_label(cx, &ok);
    }

    // -----------------------------------------------------------------------
    // Functions

    pub(crate) fn emit_fn(&mut self, f: FnId, targs: &[Ty]) {
        let name = self.instances[&(f, targs.to_vec())].clone();
        let fd = self.p.func(f);
        let Some(body) = self.bodies.get(&f) else {
            panic!("internal: no body for {}", fd.name);
        };
        let tcx = self.tcx();
        let sig = self.fn_sig(f, targs);
        let tys: Vec<Ty> = body.locals.iter().map(|l| tcx.subst(l.ty, targs)).collect();
        let mut cx = FnCx {
            targs: targs.to_vec(),
            tys: tys.clone(),
            addr: vec![String::new(); tys.len()],
            out: String::new(),
            allocas: String::new(),
            n: 0,
            ret: sig.ret.clone(),
            track_caller: sig.track_caller,
            scope: None,
            dbg: String::new(),
        };
        // Debug information.
        let fn_line = self.sm.location(fd.span).line as u32;
        let mut sp_suffix = String::new();
        let mut file_md = 0;
        if self.di.is_some() {
            let file = self.sm.try_get(fd.span.file).map(|f| f.name.clone()).unwrap_or_default();
            let root = self.opts.root_dir.clone();
            let module = self.p.modules[fd.module.idx()].path.clone();
            let label = format!("{}.{}", module, fd.qualified_name(self.p));
            let linkage = name.trim_start_matches('@').trim_matches('"').to_string();
            let opt = self.opts.optimize;
            let di = self.di.as_mut().unwrap();
            file_md = di.file(&file, &root);
            let sp = di.subprogram(&label, &linkage, file_md, fn_line, opt);
            cx.scope = Some(sp);
            sp_suffix = format!(" !dbg !{}", sp);
            self.set_loc(&mut cx, fd.span);
        }
        // Parameters and locals.
        let mut params = Vec::new();
        let mut entry_code = String::new();
        if let RetKind::Sret(_) = sig.ret {
            params.push("ptr %l0".to_string());
            cx.addr[0] = "%l0".into();
        }
        if sig.env {
            params.push("ptr %env".to_string());
        }
        let n_cap = fd.closure_captures();
        let mut k = 0usize;
        for i in 1..=body.arg_count {
            let t = tys[i];
            if i <= n_cap {
                // A variable captured by an anonymous function: its address is in the
                // environment.
                let _ = writeln!(cx.allocas, "  %l{} = alloca ptr", i);
                let _ = writeln!(entry_code, "  %c{} = getelementptr inbounds ptr, ptr %env, i32 {}", i, i - 1);
                let _ = writeln!(entry_code, "  %cv{} = load ptr, ptr %c{}", i, i);
                let _ = writeln!(entry_code, "  store ptr %cv{}, ptr %l{}", i, i);
                cx.addr[i] = format!("%l{}", i);
                continue;
            }
            let pk = sig.params.get(k).cloned();
            k += 1;
            // Received by value (concrete type) although the generic body reads it
            // through a reference: keep the value in memory and pass its address.
            if let (Some(PKind::Scalar(slt)), TyKind::Ptr(inner), LocalKind::Arg { by_ref: true }) = (&pk, &*tcx.kind(t), body.locals[i].kind) {
                if self.is_scalar(*inner) && self.llty(*inner) == *slt {
                    let align = self.align_of(*inner);
                    params.push(format!("{} %a{}", slt, i));
                    let _ = writeln!(cx.allocas, "  %v{} = alloca {}, align {}", i, slt, align);
                    let _ = writeln!(cx.allocas, "  %l{} = alloca ptr", i);
                    let _ = writeln!(entry_code, "  store {} %a{}, ptr %v{}", slt, i, i);
                    let _ = writeln!(entry_code, "  store ptr %v{}, ptr %l{}", i, i);
                    cx.addr[i] = format!("%l{}", i);
                    continue;
                }
            }
            if self.is_scalar(t) {
                let lt = self.llty(t);
                params.push(format!("{} %a{}", lt, i));
                let align = self.align_of(t);
                let _ = writeln!(cx.allocas, "  %l{} = alloca {}, align {}", i, lt, align);
                let _ = writeln!(entry_code, "  store {} %a{}, ptr %l{}", lt, i, i);
                cx.addr[i] = format!("%l{}", i);
            } else {
                params.push(format!("ptr %a{}", i));
                cx.addr[i] = format!("%a{}", i);
            }
        }
        if sig.track_caller {
            params.push("ptr %caller".into());
        }
        for i in 0..tys.len() {
            if !cx.addr[i].is_empty() {
                continue;
            }
            let lt = self.llty(tys[i]);
            let align = self.align_of(tys[i]);
            let _ = writeln!(cx.allocas, "  %l{} = alloca {}, align {}", i, lt, align);
            cx.addr[i] = format!("%l{}", i);
        }
        // Variables for the debugger.
        if let Some(scope) = cx.scope {
            self.declare("declare void @llvm.dbg.declare(metadata, metadata, metadata)");
            for (i, l) in body.locals.iter().enumerate() {
                let Some(vname) = &l.name else { continue };
                if !matches!(l.kind, LocalKind::User | LocalKind::Arg { .. }) || l.span.is_dummy() {
                    continue;
                }
                let line = self.sm.location(l.span).line as u32;
                let col = self.sm.location(l.span).col as u32;
                let arg = match l.kind {
                    LocalKind::Arg { .. } => Some(i),
                    _ => None,
                };
                let di = self.di.as_mut().unwrap();
                let ty = di.ty(&self.lx, self.p, tys[i]);
                let var = di.local_variable(vname, arg, scope, file_md, line, ty);
                let loc = di.location(scope, line, col);
                let _ = writeln!(entry_code, "    call void @llvm.dbg.declare(metadata ptr {}, metadata !{}, metadata !DIExpression()), !dbg !{}", cx.addr[i], var, loc);
            }
        }
        // Blocks. A block that cannot be reached from the entry follows an expression of
        // type `Never` (`f(x, panic("…"))`, `1 + return`): it may use the "value" of that
        // expression, which has no type the machine knows, and never runs (contract K3.10).
        let mut reachable = vec![false; body.blocks.len()];
        let mut work = vec![0 as BlockId];
        while let Some(b) = work.pop() {
            if (b as usize) < reachable.len() && !std::mem::replace(&mut reachable[b as usize], true) {
                work.extend(body.successors(b));
            }
        }
        for (bi, b) in body.blocks.iter().enumerate() {
            let _ = writeln!(cx.out, "bb{}:", bi);
            if !reachable[bi] {
                let _ = writeln!(cx.out, "  unreachable");
                continue;
            }
            for s in &b.stmts {
                self.set_loc(&mut cx, s.span);
                self.emit_stmt(&mut cx, s);
            }
            self.set_loc(&mut cx, b.term_span);
            self.emit_term(&mut cx, b);
        }
        // Signature and attributes.
        let ret = self.sig_ret(&sig);
        let fd = self.p.func(f);
        let checked = fd.export.is_some() && targs.is_empty() && self.export_checks(f);
        let exported = fd.export.is_some() && targs.is_empty() && !checked;
        let linkage = if exported {
            if self.opts.target.os == Os::Windows {
                "dllexport "
            } else {
                ""
            }
        } else {
            "internal "
        };
        if exported {
            let sym = fd.export.clone().unwrap();
            if !self.exports.contains(&sym) {
                self.exports.push(sym);
            }
        }
        let mut attrs = String::from("nounwind");
        if self.tcx().is_never(tcx.subst(fd.ret, targs)) {
            attrs.push_str(" noreturn");
        }
        if fd.hints.iter().any(|h| h == "cold") {
            attrs.push_str(" cold noinline");
        } else if fd.hints.iter().any(|h| h == "inline") {
            attrs.push_str(" inlinehint");
        }
        if fd.hints.iter().any(|h| h == "no_builtins") {
            attrs.push_str(" \"no-builtins\"");
        }
        if self.opts.target.os == Os::Windows {
            attrs.push_str(" uwtable");
        }
        if !self.opts.optimize {
            attrs.push_str(" \"frame-pointer\"=\"all\"");
        }
        let header = format!("define {}{} {}({}) {}{} {{\nstart:\n", linkage, ret, name, params.join(", "), attrs, sp_suffix);
        let mut text = header;
        text.push_str(&cx.allocas);
        text.push_str(&entry_code);
        let _ = writeln!(text, "  br label %bb0");
        text.push_str(&cx.out);
        text.push_str("}\n\n");
        self.fn_defs.push_str(&text);
        if checked {
            self.export_wrapper(f, &name, &sig);
        }
    }

    /// Checks a scalar received through the host ABI and returns its parameter type,
    /// normalized value, and NEXUL value type.
    fn checked_export_scalar(
        &self,
        ty: Ty,
        a: &str,
        i: usize,
        code: &mut String,
        panic_fn: &str,
        loc: &str,
    ) -> Option<(String, String, String)> {
        match &*self.tcx().kind(ty) {
            TyKind::Bool => {
                let _ = writeln!(
                    code,
                    "  %bad.c{i} = icmp ugt i8 {a}, 1\n  br i1 %bad.c{i}, label %bad{i}, label %ok{i}\nbad{i}:\n  call void {panic_fn}(i32 15, ptr {loc})\n  unreachable\nok{i}:\n  %v{i} = trunc i8 {a} to i1"
                );
                Some(("i8".into(), format!("%v{i}"), "i1".into()))
            }
            TyKind::Char => {
                let _ = writeln!(
                    code,
                    "  %s{i} = sub i32 {a}, 55296\n  %sur{i} = icmp ult i32 %s{i}, 2048\n  %big{i} = icmp ugt i32 {a}, 1114111\n  %bad.c{i} = or i1 %sur{i}, %big{i}\n  br i1 %bad.c{i}, label %bad{i}, label %ok{i}\nbad{i}:\n  call void {panic_fn}(i32 13, ptr {loc})\n  unreachable\nok{i}:"
                );
                Some(("i32".into(), a.into(), "i32".into()))
            }
            _ => None,
        }
    }

    /// The exported symbol of a function that receives from C values to check (ADR 0011
    /// T4, ADR 0014 §2.7): a `bool` must be the byte 0 or 1, a `char` a Unicode scalar
    /// value, a constrained type must satisfy its condition. Otherwise the program panics
    /// at the parameter; else the wrapper calls the function with the checked values.
    fn export_wrapper(&mut self, f: FnId, internal: &str, sig: &Sig) {
        let tcx = self.tcx();
        let fd = self.p.func(f);
        let sym = fd.export.clone().unwrap();
        if !self.exports.contains(&sym) {
            self.exports.push(sym.clone());
        }
        let (panic_fn, _) = self.lang_fn("panic_arith");
        let mut params = Vec::new();
        let mut allocas = String::new();
        let mut code = String::new();
        let mut args = Vec::new();
        let ptys: Vec<Ty> = fd.params.iter().map(|p| p.ty).collect();
        let ret_ty = fd.ret;
        let (c_abis, c_ret) = self.c_abi(&ptys, ret_ty);
        // A `@repr("C")` result: where the function writes it (the memory C gives, or a
        // buffer returned in registers).
        let ret_buf = match &c_ret {
            crate::cabi::RetAbi::Sret { align } => {
                let lt = self.llty(ret_ty);
                params.push(format!("ptr sret({lt}) align {align} %sret"));
                Some("%sret".to_string())
            }
            crate::cabi::RetAbi::Agg(_) => {
                let _ = writeln!(allocas, "  %rb = alloca [{} x i8], align 16", crate::layout::round_up(self.size_of(ret_ty).max(16), 16));
                Some("%rb".to_string())
            }
            crate::cabi::RetAbi::Direct => None,
        };
        if let Some(b) = &ret_buf {
            args.push(format!("ptr {b}"));
        }
        for (i, p) in fd.params.iter().enumerate() {
            let t = p.ty;
            let loc = self.location(p.span);
            let a = format!("%a{}", i);
            // The value in the type the function receives.
            let (value, lt) = match &*tcx.kind(t) {
                TyKind::Bool | TyKind::Char => {
                    let (param_ty, value, lt) = self
                        .checked_export_scalar(t, &a, i, &mut code, &panic_fn, &loc)
                        .unwrap();
                    params.push(format!("{param_ty} {a}"));
                    (value, lt)
                }
                TyKind::Adt(id, _) if self.p.adt(*id).repr_c => {
                    // A `@repr("C")` structure, received as the target passes it, rebuilt
                    // in memory for the function (spec §8.2).
                    let size = self.size_of(t);
                    let buf = format!("%cb{i}");
                    let _ = writeln!(allocas, "  {buf} = alloca [{} x i8], align 16", crate::layout::round_up(size.max(16), 16));
                    match &c_abis[i] {
                        crate::cabi::ArgAbi::Pieces(ps) => {
                            for (k, (off, ty)) in ps.iter().enumerate() {
                                params.push(format!("{ty} {a}.{k}"));
                                let _ = writeln!(code, "  %cp{i}.{k} = getelementptr inbounds i8, ptr {buf}, i64 {off}\n  store {ty} {a}.{k}, ptr %cp{i}.{k}");
                            }
                            (buf, "ptr".to_string())
                        }
                        crate::cabi::ArgAbi::Agg(ty, attr) => {
                            params.push(format!("{ty}{attr} {a}"));
                            let _ = writeln!(code, "  store {ty} {a}, ptr {buf}");
                            (buf, "ptr".to_string())
                        }
                        crate::cabi::ArgAbi::Indirect { byval, align } => {
                            let lt = self.llty(t);
                            params.push(if *byval { format!("ptr byval({lt}) align {align} {a}") } else { format!("ptr {a}") });
                            (a.clone(), "ptr".to_string())
                        }
                        crate::cabi::ArgAbi::Direct => unreachable!(),
                    }
                }
                TyKind::Adt(id, _) => {
                    // A distinct or constrained type crosses the boundary as its base.
                    let adt = self.p.adt(*id);
                    let base = adt.fields()[0].ty;
                    let validate = adt.methods.get(nxc_sema::newtype::VALIDATE_FN).copied();
                    let blt = self.llty(base);
                    let (param_ty, base_value) =
                        match self.checked_export_scalar(base, &a, i, &mut code, &panic_fn, &loc) {
                            Some((param_ty, value, _)) => (param_ty, value),
                            None => (blt.clone(), a.clone()),
                        };
                    params.push(format!("{param_ty} {a}"));
                    let v = match validate {
                        Some(vf) => {
                            let vname = self.instance(vf, vec![]);
                            let _ = writeln!(
                                code,
                                "  %b{i} = call {blt} {vname}({blt} {base_value}, ptr {loc})"
                            );
                            format!("%b{i}")
                        }
                        None => base_value,
                    };
                    let slt = self.llty(t);
                    let _ = writeln!(allocas, "  %n{i} = alloca {slt}");
                    let _ = writeln!(code, "  store {blt} {v}, ptr %n{i}");
                    (format!("%n{i}"), "ptr".to_string())
                }
                _ => {
                    let lt = self.llty(t);
                    // With the extension attribute C puts on it (`i32 signext` on RISC-V).
                    let ct = self.c_param(t);
                    params.push(format!("{ct} {a}"));
                    (a.clone(), lt)
                }
            };
            // Passed by value, or by address as the function expects.
            match (sig.params.get(i), lt.as_str()) {
                (Some(PKind::Ptr), "ptr") | (Some(PKind::Scalar(_)), _) => args.push(format!("{lt} {value}")),
                (Some(PKind::Ptr), _) => {
                    let _ = writeln!(allocas, "  %p{i} = alloca {lt}");
                    let _ = writeln!(code, "  store {lt} {value}, ptr %p{i}");
                    args.push(format!("ptr %p{i}"));
                }
                (None, _) => args.push(format!("{lt} {value}")),
            }
        }
        let mut ret = self.sig_ret(sig);
        let call = format!("{}({})", internal, args.join(", "));
        match &c_ret {
            crate::cabi::RetAbi::Sret { .. } => {
                let _ = writeln!(code, "  call void {call}\n  ret void");
            }
            crate::cabi::RetAbi::Agg(ty) => {
                let _ = writeln!(code, "  call void {call}\n  %r = load {ty}, ptr %rb\n  ret {ty} %r");
                ret = ty.clone();
            }
            crate::cabi::RetAbi::Direct if ret == "void" => {
                let _ = writeln!(code, "  call void {call}\n  ret void");
            }
            crate::cabi::RetAbi::Direct => {
                let _ = writeln!(code, "  %r = call {ret} {call}\n  ret {ret} %r");
            }
        }
        let linkage = if self.opts.target.os == Os::Windows { "dllexport " } else { "" };
        let mut attrs = String::from("nounwind");
        if self.opts.target.os == Os::Windows {
            attrs.push_str(" uwtable");
        }
        let text = format!("define {}{}{} @{}({}) {} {{\nstart:\n{}{}}}\n\n", linkage, self.c_cc(), ret, quote_name(&sym), params.join(", "), attrs, allocas, code);
        self.fn_defs.push_str(&text);
    }

    // -----------------------------------------------------------------------
    // Places and operands

    /// The address of a place and its type.
    fn place_addr(&mut self, cx: &mut FnCx, place: &Place) -> (String, Ty) {
        let tcx = self.tcx();
        let mut a = cx.addr[place.local as usize].clone();
        let mut t = cx.tys[place.local as usize];
        for pr in &place.proj {
            let k = tcx.kind(t);
            match (pr, &*k) {
                (Proj::Deref, TyKind::Ptr(e)) => {
                    a = self.val(cx, format!("load ptr, ptr {}", a));
                    t = *e;
                }
                (Proj::Field(i), TyKind::Tuple(ts)) => {
                    let lt = self.llty(t);
                    a = self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 {}", lt, a, i));
                    t = ts[*i as usize];
                }
                (Proj::Field(i), TyKind::Adt(id, args)) => {
                    let lt = self.llty(t);
                    let f = &self.p.adt(*id).fields()[*i as usize];
                    let indirect = f.indirect;
                    let ft = tcx.subst(f.ty, args);
                    a = self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 {}", lt, a, i));
                    if indirect {
                        a = self.val(cx, format!("load ptr, ptr {}", a));
                    }
                    t = ft;
                }
                (Proj::VariantField(v, i), TyKind::Adt(id, args)) => {
                    let lt = self.llty(t);
                    let var = &self.p.adt(*id).variants()[*v as usize];
                    let indirect = var.indirect;
                    let ft = tcx.subst(var.fields[*i as usize], args);
                    let pay = self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 1", lt, a));
                    let base = if indirect { self.val(cx, format!("load ptr, ptr {}", pay)) } else { pay };
                    let pt = self.payload_ty(t, *v);
                    a = self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 {}", pt, base, i));
                    t = ft;
                }
                (Proj::Index(l), TyKind::Array(e, _)) => {
                    let u = self.usize_ty();
                    let lt = self.llty(t);
                    let ia = cx.addr[*l as usize].clone();
                    let idx = self.val(cx, format!("load {}, ptr {}", u, ia));
                    a = self.val(cx, format!("getelementptr inbounds {}, ptr {}, {} 0, {} {}", lt, a, u, u, idx));
                    t = *e;
                }
                _ => panic!("internal: bad projection {:?} on {}", pr, self.p.ty_str(t)),
            }
        }
        (a, t)
    }

    fn load(&mut self, cx: &mut FnCx, addr: &str, t: Ty) -> Val {
        if self.is_void(t) {
            Val::Unit
        } else if self.is_scalar(t) {
            let lt = self.llty(t);
            let v = self.val(cx, format!("load {}, ptr {}", lt, addr));
            Val::Scalar(lt, v)
        } else {
            Val::Mem(addr.to_string())
        }
    }

    fn store(&mut self, cx: &mut FnCx, dst: &str, v: &Val, t: Ty) {
        match v {
            Val::Scalar(lt, x) => self.ins(cx, format!("store {} {}, ptr {}", lt, x, dst)),
            Val::Mem(src) => {
                let size = self.size_of(t);
                if src != dst {
                    self.memcpy(cx, dst, src, size);
                }
            }
            Val::Unit => {}
        }
    }

    fn operand_ty(&mut self, cx: &FnCx, o: &Operand) -> Ty {
        let tcx = self.tcx();
        match o {
            Operand::Copy(p) | Operand::Move(p) => {
                let mut t = cx.tys[p.local as usize];
                for pr in &p.proj {
                    t = match (pr, &*tcx.kind(t)) {
                        (Proj::Deref, TyKind::Ptr(e)) => *e,
                        (Proj::Field(i), TyKind::Tuple(ts)) => ts[*i as usize],
                        (Proj::Field(i), TyKind::Adt(id, args)) => tcx.subst(self.p.adt(*id).fields()[*i as usize].ty, args),
                        (Proj::VariantField(v, i), TyKind::Adt(id, args)) => tcx.subst(self.p.adt(*id).variants()[*v as usize].fields[*i as usize], args),
                        (Proj::Index(_), TyKind::Array(e, _)) => *e,
                        _ => tcx.error,
                    };
                }
                t
            }
            Operand::Const(_, t) => tcx.subst(*t, &cx.targs),
            Operand::FnRef(f) => {
                let fd = self.p.func(*f);
                let params = fd.params.iter().map(|p| (p.conv, p.ty)).collect();
                tcx.mk(TyKind::Fn(params, fd.ret))
            }
        }
    }

    fn operand(&mut self, cx: &mut FnCx, o: &Operand) -> (Val, Ty) {
        match o {
            Operand::Copy(p) | Operand::Move(p) => {
                let (a, t) = self.place_addr(cx, p);
                (self.load(cx, &a, t), t)
            }
            Operand::Const(v, t) => {
                let t = self.tcx().subst(*t, &cx.targs);
                let val = match v {
                    ConstValue::Str(s) => Val::Mem(self.string_value(s.as_bytes())),
                    _ if self.is_void(t) => Val::Unit,
                    _ => {
                        let lt = self.llty(t);
                        Val::Scalar(lt, self.const_scalar(v, t))
                    }
                };
                (val, t)
            }
            Operand::FnRef(f) => {
                let t = self.operand_ty(cx, o);
                let name = self.fn_value(*f);
                (Val::Scalar("ptr".into(), name), t)
            }
        }
    }

    /// The address of a function used as a value. A `@track_caller` function is wrapped
    /// so that indirect callers need not pass a location.
    fn fn_value(&mut self, f: FnId) -> String {
        let name = self.instance(f, vec![]);
        if !self.p.func(f).track_caller {
            return name;
        }
        let shim = format!("@{}", crate::quote_name(&format!("{}.value", name.trim_start_matches('@').trim_matches('"'))));
        if self.helpers.insert(shim.clone()) {
            let mut sig = self.fn_sig(f, &[]);
            let inner = self.sig_param_list(&sig);
            sig.track_caller = false;
            let outer = self.sig_param_list(&sig);
            let ret = self.sig_ret(&sig);
            let params: Vec<String> = outer.iter().enumerate().map(|(i, t)| format!("{} %p{}", t, i)).collect();
            let mut args: Vec<String> = outer.iter().enumerate().map(|(i, t)| format!("{} %p{}", t, i)).collect();
            args.push("ptr null".into());
            debug_assert_eq!(inner.len(), args.len());
            let call = format!("call {} {}({})", ret, name, args.join(", "));
            let body = if ret == "void" { format!("  {}\n  ret void\n", call) } else { format!("  %r = {}\n  ret {} %r\n", call, ret) };
            let _ = write!(self.fn_defs, "define internal {} {}({}) nounwind {{\nstart:\n{}}}\n\n", ret, shim, params.join(", "), body);
        }
        shim
    }

    // -----------------------------------------------------------------------
    // Statements

    fn emit_stmt(&mut self, cx: &mut FnCx, s: &Statement) {
        match &s.kind {
            StmtKind::Assign(place, rv) => self.emit_assign(cx, place, rv, s.span),
            StmtKind::Drop(place) => {
                let (a, t) = self.place_addr(cx, place);
                if let Some(d) = self.drop_fn(t) {
                    self.ins(cx, format!("call void {}(ptr {})", d, a));
                }
            }
            StmtKind::DropIfInit(l) => {
                // Drop elaboration replaces every `DropIfInit` (contract K3.2). Dropping
                // unconditionally here could destroy a moved value twice: stop instead.
                panic!("internal: `DropIfInit(_{})` reached code generation; drop elaboration did not run", l);
            }
            StmtKind::MarkMoved(_) | StmtKind::Nop => {}
            StmtKind::BoundsCheck { index, len, dest } => {
                let (iv, it) = self.operand(cx, index);
                let (lv, _) = self.operand(cx, len);
                let u = self.usize_ty();
                let ii = self.tcx().as_int(it).unwrap_or(IntTy::Usize);
                let ib = self.int_bits(ii);
                let ub = self.opts.target.ptr_bits;
                // Compare in 64 bits: the index may be wider or narrower than `usize`.
                let i64v = if ib == 64 { iv.scalar().to_string() } else { self.val(cx, format!("{} i{} {} to i64", if ii.signed() { "sext" } else { "zext" }, ib, iv.scalar())) };
                let l64 = if ub == 64 { lv.scalar().to_string() } else { self.val(cx, format!("zext {} {} to i64", u, lv.scalar())) };
                if ii.signed() {
                    let neg = self.val(cx, format!("icmp slt i64 {}, 0", i64v));
                    let bad = self.new_label(cx);
                    let ok = self.new_label(cx);
                    self.ins(cx, format!("br i1 {}, label %{}, label %{}", neg, bad, ok));
                    self.put_label(cx, &bad);
                    let (f, _) = self.lang_fn("panic_bounds_neg");
                    let loc = self.loc(cx, s.span);
                    self.ins(cx, format!("call void {}(i64 {}, i64 {}, ptr {})", f, i64v, l64, loc));
                    self.ins(cx, "unreachable");
                    self.put_label(cx, &ok);
                }
                let out = self.val(cx, format!("icmp uge i64 {}, {}", i64v, l64));
                let bad = self.new_label(cx);
                let ok = self.new_label(cx);
                self.ins(cx, format!("br i1 {}, label %{}, label %{}", out, bad, ok));
                self.put_label(cx, &bad);
                let (f, _) = self.lang_fn("panic_bounds");
                let loc = self.loc(cx, s.span);
                self.ins(cx, format!("call void {}(i64 {}, i64 {}, ptr {})", f, i64v, l64, loc));
                self.ins(cx, "unreachable");
                self.put_label(cx, &ok);
                let idx = if ub == 64 { i64v } else { self.val(cx, format!("trunc i64 {} to {}", i64v, u)) };
                let da = cx.addr[*dest as usize].clone();
                self.ins(cx, format!("store {} {}, ptr {}", u, idx, da));
            }
            StmtKind::FreeBox(place, b) => {
                let (a, t) = self.place_addr(cx, place);
                let lt = self.llty(t);
                let slot = match b {
                    BoxRef::Field(i) => self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 {}", lt, a, i)),
                    BoxRef::Variant(_) => self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 1", lt, a)),
                };
                let bx = self.val(cx, format!("load ptr, ptr {}", slot));
                let (free, _) = self.lang_fn("free");
                self.ins(cx, format!("call void {}(ptr {})", free, bx));
            }
        }
    }

    fn emit_assign(&mut self, cx: &mut FnCx, place: &Place, rv: &Rvalue, span: Span) {
        match rv {
            Rvalue::Use(o) => {
                let (v, t) = self.operand(cx, o);
                let (dst, _) = self.place_addr(cx, place);
                self.store(cx, &dst, &v, t);
            }
            Rvalue::Ref(p) => {
                let (a, _) = self.place_addr(cx, p);
                let (dst, _) = self.place_addr(cx, place);
                self.ins(cx, format!("store ptr {}, ptr {}", a, dst));
            }
            Rvalue::StaticAddr(st) => {
                let g = self.static_global(*st);
                let (dst, _) = self.place_addr(cx, place);
                self.ins(cx, format!("store ptr {}, ptr {}", g, dst));
            }
            Rvalue::Closure { f, env, heap } => {
                let code = self.instance(*f, cx.targs.clone());
                let envp = if env.is_empty() {
                    "null".to_string()
                } else {
                    let arr_t = format!("[{} x ptr]", env.len());
                    let arr = if *heap {
                        // Freed by the task that runs the function (`std.tache`).
                        let (alloc, _) = self.lang_fn("alloc");
                        let u = self.usize_ty();
                        let size = env.len() as u64 * self.lx.ptr_bytes;
                        self.val(cx, format!("call ptr {}({} {})", alloc, u, size))
                    } else {
                        self.alloca_raw(cx, &arr_t)
                    };
                    for (i, o) in env.iter().enumerate() {
                        let (v, _) = self.operand(cx, o);
                        let slot = self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 {}", arr_t, arr, i));
                        self.ins(cx, format!("store ptr {}, ptr {}", v.scalar(), slot));
                    }
                    arr
                };
                let (dst, _) = self.place_addr(cx, place);
                self.store_pair(cx, &dst, &code, &envp);
            }
            Rvalue::FnToClosure(o) => {
                let (v, t) = self.operand(cx, o);
                let thunk = self.closure_thunk(t);
                let (dst, _) = self.place_addr(cx, place);
                let fv = v.scalar().to_string();
                self.store_pair(cx, &dst, &thunk, &fv);
            }
            Rvalue::ToDyn(o, src) => {
                let (v, t) = self.operand(cx, o);
                let _ = src;
                let (dst, dt) = self.place_addr(cx, place);
                let tr = match &*self.tcx().kind(dt) {
                    TyKind::Dyn(tr) => *tr,
                    _ => panic!("internal: conversion to a non-dyn type"),
                };
                let size = self.size_of(t).max(1);
                let data = self.heap_alloc_bytes(cx, size);
                self.store(cx, &data, &v, t);
                let vt = self.vtable(tr, t);
                self.store_pair(cx, &dst, &data, &vt);
            }
            Rvalue::Discriminant(p) => {
                let (a, t) = self.place_addr(cx, p);
                let lt = self.llty(t);
                let tp = self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 0", lt, a));
                let tag = self.val(cx, format!("load i32, ptr {}", tp));
                let (dst, _) = self.place_addr(cx, place);
                self.ins(cx, format!("store i32 {}, ptr {}", tag, dst));
            }
            Rvalue::Unary(op, o) => {
                let (v, t) = self.operand(cx, o);
                let r = self.unary(cx, *op, &v, t, span);
                let (dst, dt) = self.place_addr(cx, place);
                let lt = self.llty(dt);
                self.ins(cx, format!("store {} {}, ptr {}", lt, r, dst));
            }
            Rvalue::Binary(op, a, b) => {
                let (va, ta) = self.operand(cx, a);
                let (vb, tb) = self.operand(cx, b);
                let r = self.binary(cx, *op, &va, ta, &vb, tb, span);
                let (dst, dt) = self.place_addr(cx, place);
                if !self.is_void(dt) {
                    let lt = self.llty(dt);
                    self.ins(cx, format!("store {} {}, ptr {}", lt, r, dst));
                }
            }
            Rvalue::Cast(o, to) => {
                let to = self.tcx().subst(*to, &cx.targs);
                let (v, from) = self.operand(cx, o);
                let r = self.cast(cx, &v, from, to, span);
                let (dst, _) = self.place_addr(cx, place);
                let lt = self.llty(to);
                self.ins(cx, format!("store {} {}, ptr {}", lt, r, dst));
            }
            Rvalue::Aggregate(kind, ops) => {
                let vals: Vec<(Val, Ty)> = ops.iter().map(|o| self.operand(cx, o)).collect();
                let (dst, dt) = self.place_addr(cx, place);
                self.aggregate(cx, kind, &vals, &dst, dt);
            }
            Rvalue::Repeat(o, n) => {
                let (v, et) = self.operand(cx, o);
                let (dst, dt) = self.place_addr(cx, place);
                if *n == 0 || self.size_of(et) == 0 {
                    return;
                }
                let u = self.usize_ty();
                let lt = self.llty(dt);
                let ctr = self.alloca_raw(cx, u);
                self.ins(cx, format!("store {} 0, ptr {}", u, ctr));
                let head = self.new_label(cx);
                let body = self.new_label(cx);
                let end = self.new_label(cx);
                self.ins(cx, format!("br label %{}", head));
                self.put_label(cx, &head);
                let i = self.val(cx, format!("load {}, ptr {}", u, ctr));
                let c = self.val(cx, format!("icmp ult {} {}, {}", u, i, n));
                self.ins(cx, format!("br i1 {}, label %{}, label %{}", c, body, end));
                self.put_label(cx, &body);
                let ea = self.val(cx, format!("getelementptr inbounds {}, ptr {}, {} 0, {} {}", lt, dst, u, u, i));
                self.store(cx, &ea, &v, et);
                let i1 = self.val(cx, format!("add {} {}, 1", u, i));
                self.ins(cx, format!("store {} {}, ptr {}", u, i1, ctr));
                self.ins(cx, format!("br label %{}", head));
                self.put_label(cx, &end);
            }
        }
    }

    fn aggregate(&mut self, cx: &mut FnCx, kind: &AggKind, vals: &[(Val, Ty)], dst: &str, dt: Ty) {
        let tcx = self.tcx();
        let lt = self.llty(dt);
        match kind {
            AggKind::Tuple => {
                for (i, (v, t)) in vals.iter().enumerate() {
                    let fa = self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 {}", lt, dst, i));
                    self.store(cx, &fa, v, *t);
                }
            }
            AggKind::Array => {
                let u = self.usize_ty();
                for (i, (v, t)) in vals.iter().enumerate() {
                    let fa = self.val(cx, format!("getelementptr inbounds {}, ptr {}, {} 0, {} {}", lt, dst, u, u, i));
                    self.store(cx, &fa, v, *t);
                }
            }
            AggKind::Adt { adt, variant, .. } => {
                let (_, args) = tcx.as_adt(dt).unwrap_or((*adt, vec![]));
                let a = self.p.adt(*adt);
                match variant {
                    None => {
                        let fields: Vec<(bool, Ty)> = a.fields().iter().map(|f| (f.indirect, tcx.subst(f.ty, &args))).collect();
                        for (i, ((v, t), (indirect, ft))) in vals.iter().zip(fields.iter()).enumerate() {
                            let fa = self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 {}", lt, dst, i));
                            if *indirect {
                                let bx = self.heap_box(cx, *ft);
                                self.store(cx, &bx, v, *t);
                                self.ins(cx, format!("store ptr {}, ptr {}", bx, fa));
                            } else {
                                self.store(cx, &fa, v, *t);
                            }
                        }
                    }
                    Some(vi) => {
                        let indirect = a.variants()[*vi as usize].indirect;
                        let tp = self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 0", lt, dst));
                        self.ins(cx, format!("store i32 {}, ptr {}", vi, tp));
                        if vals.is_empty() {
                            return;
                        }
                        let pa = self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 1", lt, dst));
                        let pt = self.payload_ty(dt, *vi);
                        let base = if indirect {
                            let fields = self.lx.variant_fields(dt, *vi);
                            let size = self.lx.struct_layout(&fields).size;
                            let bx = self.heap_alloc_bytes(cx, size);
                            self.ins(cx, format!("store ptr {}, ptr {}", bx, pa));
                            bx
                        } else {
                            pa
                        };
                        for (i, (v, t)) in vals.iter().enumerate() {
                            let fa = self.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 {}", pt, base, i));
                            self.store(cx, &fa, v, *t);
                        }
                    }
                }
            }
        }
    }

    /// Stores a `{ ptr, ptr }` value (anonymous function or `dyn`).
    fn store_pair(&mut self, cx: &mut FnCx, dst: &str, a: &str, b: &str) {
        self.ins(cx, format!("store ptr {}, ptr {}", a, dst));
        let p1 = self.val(cx, format!("getelementptr inbounds {{ ptr, ptr }}, ptr {}, i32 0, i32 1", dst));
        self.ins(cx, format!("store ptr {}, ptr {}", b, p1));
    }

    /// The code of an anonymous function that calls the function pointer stored as its
    /// environment (one per signature).
    fn closure_thunk(&mut self, fn_ty: Ty) -> String {
        let sig = self.fn_ty_sig(fn_ty);
        let inner = self.sig_param_list(&sig);
        let ret = self.sig_ret(&sig);
        let name = format!("@{}", crate::quote_name(&format!("nx.thunk({}) -> {}", inner.join(", "), ret)));
        if self.helpers.insert(name.clone()) {
            let sret = matches!(sig.ret, RetKind::Sret(_));
            let mut params = Vec::new();
            let mut args = Vec::new();
            for (i, t) in inner.iter().enumerate() {
                if i == 0 && sret {
                    params.push(format!("{} %p{}", t, i));
                    args.push(format!("{} %p{}", t, i));
                    params.push("ptr %env".to_string());
                    continue;
                }
                params.push(format!("{} %p{}", t, i));
                args.push(format!("{} %p{}", t, i));
            }
            if !sret {
                params.insert(0, "ptr %env".to_string());
            }
            let call = format!("call {} %env({})", ret, args.join(", "));
            let body = if ret == "void" { format!("  {}\n  ret void\n", call) } else { format!("  %r = {}\n  ret {} %r\n", call, ret) };
            let _ = write!(self.fn_defs, "define internal {} {}({}) nounwind {{\nstart:\n{}}}\n\n", ret, name, params.join(", "), body);
        }
        name
    }

    fn heap_alloc_bytes(&mut self, cx: &mut FnCx, size: u64) -> String {
        let (f, _) = self.lang_fn("alloc");
        let u = self.usize_ty();
        self.val(cx, format!("call ptr {}({} {})", f, u, size))
    }

    fn heap_box(&mut self, cx: &mut FnCx, t: Ty) -> String {
        let size = self.size_of(t);
        self.heap_alloc_bytes(cx, size)
    }

    // -----------------------------------------------------------------------
    // Operators

    fn unary(&mut self, cx: &mut FnCx, op: UnOp, v: &Val, t: Ty, span: Span) -> String {
        let tcx = self.tcx();
        let x = v.scalar().to_string();
        match (op, &*tcx.kind(t)) {
            (UnOp::Not, _) => self.val(cx, format!("xor i1 {}, true", x)),
            (UnOp::Neg, TyKind::Int(i)) => {
                let bits = self.int_bits(*i);
                if i.signed() {
                    self.declare(&format!("declare {{i{b}, i1}} @llvm.ssub.with.overflow.i{b}(i{b}, i{b})", b = bits));
                    let r = self.val(cx, format!("call {{i{b}, i1}} @llvm.ssub.with.overflow.i{b}(i{b} 0, i{b} {})", x, b = bits));
                    let o = self.val(cx, format!("extractvalue {{i{}, i1}} {}, 1", bits, r));
                    self.check(cx, &o, kind::NEG, span);
                    self.val(cx, format!("extractvalue {{i{}, i1}} {}, 0", bits, r))
                } else {
                    let nz = self.val(cx, format!("icmp ne i{} {}, 0", bits, x));
                    self.check(cx, &nz, kind::NEG, span);
                    self.val(cx, format!("sub i{} 0, {}", bits, x))
                }
            }
            (UnOp::Neg, TyKind::Float(_)) => {
                let lt = self.llty(t);
                self.val(cx, format!("fneg {} {}", lt, x))
            }
            (UnOp::BitNot, TyKind::Int(i)) => {
                let bits = self.int_bits(*i);
                self.val(cx, format!("xor i{} {}, -1", bits, x))
            }
            _ => panic!("internal: unary {:?} on {}", op, self.p.ty_str(t)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn binary(&mut self, cx: &mut FnCx, op: BinOp, a: &Val, t: Ty, b: &Val, bt: Ty, span: Span) -> String {
        let tcx = self.tcx();
        let k = tcx.kind(t);
        match &*k {
            TyKind::Int(i) => self.int_binary(cx, op, *i, a.scalar(), b.scalar(), bt, span),
            TyKind::Float(f) => {
                let lt = self.llty(t);
                let (x, y) = (a.scalar(), b.scalar());
                match op {
                    BinOp::Add => self.val(cx, format!("fadd {} {}, {}", lt, x, y)),
                    BinOp::Sub => self.val(cx, format!("fsub {} {}, {}", lt, x, y)),
                    BinOp::Mul => self.val(cx, format!("fmul {} {}, {}", lt, x, y)),
                    BinOp::Div => self.val(cx, format!("fdiv {} {}, {}", lt, x, y)),
                    BinOp::Rem => self.float_call2(cx, "math.fmod", *f, x, y),
                    _ => {
                        let pred = match op {
                            BinOp::Eq => "oeq",
                            BinOp::Ne => "une",
                            BinOp::Lt => "olt",
                            BinOp::Le => "ole",
                            BinOp::Gt => "ogt",
                            _ => "oge",
                        };
                        self.val(cx, format!("fcmp {} {} {}, {}", pred, lt, x, y))
                    }
                }
            }
            TyKind::Bool => {
                let (x, y) = (a.scalar(), b.scalar());
                let i = match op {
                    BinOp::And | BinOp::BitAnd => "and",
                    BinOp::Or | BinOp::BitOr => "or",
                    BinOp::BitXor | BinOp::Ne => "xor",
                    BinOp::Eq => return self.val(cx, format!("icmp eq i1 {}, {}", x, y)),
                    _ => return self.val(cx, format!("icmp {} i1 {}, {}", cmp_pred(op, false), x, y)),
                };
                self.val(cx, format!("{} i1 {}, {}", i, x, y))
            }
            TyKind::Char => self.val(cx, format!("icmp {} i32 {}, {}", cmp_pred(op, false), a.scalar(), b.scalar())),
            TyKind::Ptr(_) | TyKind::Fn(..) => self.val(cx, format!("icmp {} ptr {}, {}", cmp_pred(op, false), a.scalar(), b.scalar())),
            TyKind::Adt(..) => {
                // Fieldless enumerations: compare the tags.
                let lt = self.llty(t);
                let tag = |g: &mut Self, cx: &mut FnCx, v: &Val| -> String {
                    match v {
                        Val::Mem(p) => {
                            let tp = g.val(cx, format!("getelementptr inbounds {}, ptr {}, i32 0, i32 0", lt, p));
                            g.val(cx, format!("load i32, ptr {}", tp))
                        }
                        _ => "0".into(),
                    }
                };
                let x = tag(self, cx, a);
                let y = tag(self, cx, b);
                self.val(cx, format!("icmp {} i32 {}, {}", cmp_pred(op, false), x, y))
            }
            TyKind::Unit | TyKind::Never | TyKind::Error => {
                if op == BinOp::Eq || op == BinOp::Le || op == BinOp::Ge {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            _ => panic!("internal: binary {:?} on {}", op, self.p.ty_str(t)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn int_binary(&mut self, cx: &mut FnCx, op: BinOp, i: IntTy, x: &str, y: &str, bt: Ty, span: Span) -> String {
        let bits = self.int_bits(i);
        let ty = format!("i{}", bits);
        let s = i.signed();
        let checked = |g: &mut Self, cx: &mut FnCx, name: &str, k: u32| -> String {
            // On 32-bit targets LLVM would call a 128-bit multiplication of the C runtime
            // (`__multi3`, `__mulodi4`): NEXUL checks 64-bit products itself.
            let intr = if name == "mul" && bits == 64 && g.opts.target.ptr_bits == 32 {
                g.mul64_helper(s)
            } else {
                let i = format!("@llvm.{}{}.with.overflow.{}", if s { "s" } else { "u" }, name, ty);
                g.declare(&format!("declare {{{t}, i1}} {}({t}, {t})", i, t = ty));
                i
            };
            let r = g.val(cx, format!("call {{{t}, i1}} {}({t} {}, {t} {})", intr, x, y, t = ty));
            let o = g.val(cx, format!("extractvalue {{{}, i1}} {}, 1", ty, r));
            g.check(cx, &o, k, span);
            g.val(cx, format!("extractvalue {{{}, i1}} {}, 0", ty, r))
        };
        match op {
            BinOp::Add => checked(self, cx, "add", kind::ADD),
            BinOp::Sub => checked(self, cx, "sub", kind::SUB),
            BinOp::Mul => checked(self, cx, "mul", kind::MUL),
            BinOp::Div | BinOp::Rem => {
                let is_div = op == BinOp::Div;
                let z = self.val(cx, format!("icmp eq {} {}, 0", ty, y));
                self.check(cx, &z, if is_div { kind::DIV_ZERO } else { kind::REM_ZERO }, span);
                if s {
                    let min = int_const(i.min().max(-(1i128 << (bits - 1))), bits);
                    let m1 = self.val(cx, format!("icmp eq {} {}, -1", ty, y));
                    let mn = self.val(cx, format!("icmp eq {} {}, {}", ty, x, min));
                    let ov = self.val(cx, format!("and i1 {}, {}", m1, mn));
                    self.check(cx, &ov, if is_div { kind::DIV_OVERFLOW } else { kind::REM_OVERFLOW }, span);
                }
                let instr = match (is_div, s) {
                    (true, true) => "sdiv",
                    (true, false) => "udiv",
                    (false, true) => "srem",
                    (false, false) => "urem",
                };
                self.val(cx, format!("{} {} {}, {}", instr, ty, x, y))
            }
            BinOp::AddWrap => self.val(cx, format!("add {} {}, {}", ty, x, y)),
            BinOp::SubWrap => self.val(cx, format!("sub {} {}, {}", ty, x, y)),
            BinOp::MulWrap => self.val(cx, format!("mul {} {}, {}", ty, x, y)),
            BinOp::BitAnd => self.val(cx, format!("and {} {}, {}", ty, x, y)),
            BinOp::BitOr => self.val(cx, format!("or {} {}, {}", ty, x, y)),
            BinOp::BitXor => self.val(cx, format!("xor {} {}, {}", ty, x, y)),
            BinOp::Shl | BinOp::Shr => {
                // The amount may have any integer type: check it in 64 bits.
                let ai = self.tcx().as_int(bt).unwrap_or(IntTy::U32);
                let ab = self.int_bits(ai);
                let amt = if ab == 64 { y.to_string() } else { self.val(cx, format!("{} i{} {} to i64", if ai.signed() { "sext" } else { "zext" }, ab, y)) };
                let bad = if ai.signed() {
                    let neg = self.val(cx, format!("icmp slt i64 {}, 0", amt));
                    let big = self.val(cx, format!("icmp sge i64 {}, {}", amt, bits));
                    self.val(cx, format!("or i1 {}, {}", neg, big))
                } else {
                    self.val(cx, format!("icmp uge i64 {}, {}", amt, bits))
                };
                self.check(cx, &bad, if op == BinOp::Shl { kind::SHL } else { kind::SHR }, span);
                let n = if bits == 64 { amt } else { self.val(cx, format!("trunc i64 {} to {}", amt, ty)) };
                let instr = match (op, s) {
                    (BinOp::Shl, _) => "shl",
                    (_, true) => "ashr",
                    (_, false) => "lshr",
                };
                self.val(cx, format!("{} {} {}, {}", instr, ty, x, n))
            }
            _ => self.val(cx, format!("icmp {} {} {}, {}", cmp_pred(op, s), ty, x, y)),
        }
    }

    /// Calls a binary runtime math function (in `f64`).
    fn float_call2(&mut self, cx: &mut FnCx, name: &str, f: FloatTy, x: &str, y: &str) -> String {
        let (func, _) = self.lang_fn(name);
        if f == FloatTy::F64 {
            return self.val(cx, format!("call double {}(double {}, double {})", func, x, y));
        }
        let xe = self.val(cx, format!("fpext float {} to double", x));
        let ye = self.val(cx, format!("fpext float {} to double", y));
        let r = self.val(cx, format!("call double {}(double {}, double {})", func, xe, ye));
        self.val(cx, format!("fptrunc double {} to float", r))
    }

    fn float_call1(&mut self, cx: &mut FnCx, name: &str, f: FloatTy, x: &str) -> String {
        let (func, _) = self.lang_fn(name);
        if f == FloatTy::F64 {
            return self.val(cx, format!("call double {}(double {})", func, x));
        }
        let xe = self.val(cx, format!("fpext float {} to double", x));
        let r = self.val(cx, format!("call double {}(double {})", func, xe));
        self.val(cx, format!("fptrunc double {} to float", r))
    }

    // -----------------------------------------------------------------------
    // Conversions (spec §3.6)

    fn cast(&mut self, cx: &mut FnCx, v: &Val, from: Ty, to: Ty, span: Span) -> String {
        let tcx = self.tcx();
        let x = v.scalar().to_string();
        let (kf, kt) = (tcx.kind(from), tcx.kind(to));
        match (&*kf, &*kt) {
            (TyKind::Int(a), TyKind::Int(b)) => self.int_cast(cx, &x, *a, *b, span),
            (TyKind::Int(a), TyKind::Float(_)) => {
                let lt = self.llty(to);
                let ab = self.int_bits(*a);
                self.val(cx, format!("{} i{} {} to {}", if a.signed() { "sitofp" } else { "uitofp" }, ab, x, lt))
            }
            (TyKind::Float(a), TyKind::Float(b)) => {
                if a == b {
                    return x;
                }
                if *a == FloatTy::F32 {
                    self.val(cx, format!("fpext float {} to double", x))
                } else {
                    self.val(cx, format!("fptrunc double {} to float", x))
                }
            }
            (TyKind::Float(a), TyKind::Int(b)) => {
                let fl = self.llty(from);
                let nan = self.val(cx, format!("fcmp uno {} {}, {}", fl, x, x));
                self.check(cx, &nan, kind::CAST_NAN, span);
                let d = if *a == FloatTy::F32 { self.val(cx, format!("fpext float {} to double", x)) } else { x.clone() };
                let bits = self.int_bits(*b);
                let (lo, lo_incl, hi) = if b.signed() {
                    let h = 2f64.powi(bits as i32 - 1);
                    if bits == 64 {
                        (-h, true, h)
                    } else {
                        (-h - 1.0, false, h)
                    }
                } else {
                    (-1.0, false, 2f64.powi(bits as i32))
                };
                let c1 = self.val(cx, format!("fcmp {} double {}, {}", if lo_incl { "olt" } else { "ole" }, d, float_const(lo, FloatTy::F64)));
                let c2 = self.val(cx, format!("fcmp oge double {}, {}", d, float_const(hi, FloatTy::F64)));
                let bad = self.val(cx, format!("or i1 {}, {}", c1, c2));
                self.check(cx, &bad, kind::CAST, span);
                self.val(cx, format!("{} {} {} to i{}", if b.signed() { "fptosi" } else { "fptoui" }, fl, x, bits))
            }
            (TyKind::Bool, TyKind::Int(b)) => {
                let bits = self.int_bits(*b);
                self.val(cx, format!("zext i1 {} to i{}", x, bits))
            }
            (TyKind::Char, TyKind::Int(b)) => self.int_cast(cx, &x, IntTy::U32, *b, span),
            (TyKind::Int(a), TyKind::Char) => {
                let ab = self.int_bits(*a);
                let w = if ab < 64 { self.val(cx, format!("{} i{} {} to i64", if a.signed() { "sext" } else { "zext" }, ab, x)) } else { x.clone() };
                // Valid: 0 <= v <= 0x10FFFF and not a surrogate.
                let big = self.val(cx, format!("icmp ugt i64 {}, 1114111", w));
                let lo = self.val(cx, format!("icmp uge i64 {}, 55296", w));
                let hi = self.val(cx, format!("icmp ule i64 {}, 57343", w));
                let sur = self.val(cx, format!("and i1 {}, {}", lo, hi));
                let bad = self.val(cx, format!("or i1 {}, {}", big, sur));
                self.check(cx, &bad, kind::CHAR, span);
                self.val(cx, format!("trunc i64 {} to i32", w))
            }
            (TyKind::Ptr(_), TyKind::Ptr(_)) | (TyKind::Fn(..), TyKind::Fn(..)) => x,
            (TyKind::Ptr(_), TyKind::Int(b)) => {
                let bits = self.int_bits(*b);
                self.val(cx, format!("ptrtoint ptr {} to i{}", x, bits))
            }
            (TyKind::Int(a), TyKind::Ptr(_)) => {
                let bits = self.int_bits(*a);
                self.val(cx, format!("inttoptr i{} {} to ptr", bits, x))
            }
            _ if from == to => x,
            _ => panic!("internal: cast from {} to {}", self.p.ty_str(from), self.p.ty_str(to)),
        }
    }

    /// Checked integer conversion: panics unless the value is representable.
    fn int_cast(&mut self, cx: &mut FnCx, x: &str, a: IntTy, b: IntTy, span: Span) -> String {
        let (ab, bb) = (self.int_bits(a), self.int_bits(b));
        let (sa, sb) = (a.signed(), b.signed());
        let max_b: i128 = if sb { (1i128 << (bb - 1)) - 1 } else { (1i128 << bb) - 1 };
        let min_b: i128 = if sb { -(1i128 << (bb - 1)) } else { 0 };
        let max_a: i128 = if sa { (1i128 << (ab - 1)) - 1 } else { (1i128 << ab) - 1 };
        let min_a: i128 = if sa { -(1i128 << (ab - 1)) } else { 0 };
        let ty_a = format!("i{}", ab);
        let mut conds = Vec::new();
        if min_a < min_b {
            // Only possible for a signed source.
            let c = if min_b == 0 { self.val(cx, format!("icmp slt {} {}, 0", ty_a, x)) } else { self.val(cx, format!("icmp slt {} {}, {}", ty_a, x, int_const(min_b, ab))) };
            conds.push(c);
        }
        if max_a > max_b {
            let pred = if sa { "sgt" } else { "ugt" };
            let c = self.val(cx, format!("icmp {} {} {}, {}", pred, ty_a, x, int_const(max_b, ab)));
            conds.push(c);
        }
        if !conds.is_empty() {
            let mut bad = conds[0].clone();
            for c in &conds[1..] {
                bad = self.val(cx, format!("or i1 {}, {}", bad, c));
            }
            self.check(cx, &bad, kind::CAST, span);
        }
        if ab == bb {
            x.to_string()
        } else if ab > bb {
            self.val(cx, format!("trunc {} {} to i{}", ty_a, x, bb))
        } else {
            self.val(cx, format!("{} {} {} to i{}", if sa { "sext" } else { "zext" }, ty_a, x, bb))
        }
    }

    // -----------------------------------------------------------------------
    // Terminators and calls

    fn emit_term(&mut self, cx: &mut FnCx, b: &BasicBlock) {
        match &b.term {
            Terminator::Goto(t) => self.ins(cx, format!("br label %bb{}", t)),
            Terminator::Switch { discr, targets, otherwise } => {
                let (v, t) = self.operand(cx, discr);
                let x = v.scalar().to_string();
                if t == self.tcx().bool && targets.len() == 1 {
                    let (val, tb) = targets[0];
                    if val == 0 {
                        self.ins(cx, format!("br i1 {}, label %bb{}, label %bb{}", x, otherwise, tb));
                    } else {
                        self.ins(cx, format!("br i1 {}, label %bb{}, label %bb{}", x, tb, otherwise));
                    }
                } else {
                    let lt = self.llty(t);
                    let bits: u32 = lt.trim_start_matches('i').parse().unwrap_or(32);
                    let mut s = format!("switch {} {}, label %bb{} [", lt, x, otherwise);
                    for (val, tb) in targets {
                        let _ = write!(s, " {} {}, label %bb{}", lt, int_const(*val as i128, bits), tb);
                    }
                    s.push_str(" ]");
                    self.ins(cx, s);
                }
            }
            Terminator::Return => match cx.ret.clone() {
                RetKind::Scalar(lt) => {
                    let a = cx.addr[0].clone();
                    let v = self.val(cx, format!("load {}, ptr {}", lt, a));
                    self.ins(cx, format!("ret {} {}", lt, v));
                }
                _ => self.ins(cx, "ret void"),
            },
            Terminator::Unreachable => self.ins(cx, "unreachable"),
            Terminator::Call { callee, args, dest, target } => {
                self.emit_call(cx, callee, args, dest.as_ref(), b.term_span);
                match target {
                    Some(t) => self.ins(cx, format!("br label %bb{}", t)),
                    None => self.ins(cx, "unreachable"),
                }
            }
        }
    }

    /// A buffer large enough for a `@repr("C")` structure and every coerced type it is
    /// loaded or stored as (at most 16 bytes past its end, aligned to 16).
    fn c_buffer(&mut self, cx: &mut FnCx, size: u64) -> String {
        let n = crate::layout::round_up(size.max(16), 16);
        let name = format!("%cb{}", self.fresh());
        let _ = writeln!(cx.allocas, "  {} = alloca [{} x i8], align 16", name, n);
        name
    }

    /// A call of a C function (spec §8.2): scalars as they are, `@repr("C")` structures
    /// with the calling convention of the target (`cabi`).
    fn emit_c_call(&mut self, cx: &mut FnCx, name: &str, ptys: &[Ty], ret_ty: Ty, args: &[Operand], dest: Option<&Place>) {
        let (abis, rabi) = self.c_abi(ptys, ret_ty);
        let mut argv = Vec::new();
        // The result: in memory given to the callee, or in registers stored into a buffer.
        let ret_buf = match &rabi {
            crate::cabi::RetAbi::Sret { align } => {
                let lt = self.llty(ret_ty);
                let b = self.c_buffer(cx, self.size_of(ret_ty));
                argv.push(format!("ptr sret({}) align {} {}", lt, align, b));
                Some(b)
            }
            crate::cabi::RetAbi::Agg(_) => Some(self.c_buffer(cx, self.size_of(ret_ty))),
            crate::cabi::RetAbi::Direct => None,
        };
        for ((a, &pt), abi) in args.iter().zip(ptys).zip(&abis) {
            let (v, _) = self.operand(cx, a);
            match abi {
                crate::cabi::ArgAbi::Direct => {
                    let ct = self.c_param(pt);
                    argv.push(format!("{} {}", ct, v.scalar()));
                }
                _ => {
                    // A structure, in memory: copied into a buffer the coerced loads may
                    // read past its end, then loaded piece by piece or passed by address.
                    let src = match &v {
                        Val::Mem(m) => m.clone(),
                        other => panic!("internal: a structure in a register: {:?}", other),
                    };
                    let size = self.size_of(pt);
                    let buf = self.c_buffer(cx, size);
                    self.memcpy(cx, &buf, &src, size);
                    match abi {
                        crate::cabi::ArgAbi::Pieces(ps) => {
                            for (off, ty) in ps {
                                let at = self.val(cx, format!("getelementptr inbounds i8, ptr {}, i64 {}", buf, off));
                                let x = self.val(cx, format!("load {}, ptr {}", ty, at));
                                argv.push(format!("{} {}", ty, x));
                            }
                        }
                        crate::cabi::ArgAbi::Agg(ty, attr) => {
                            let x = self.val(cx, format!("load {}, ptr {}", ty, buf));
                            argv.push(format!("{}{} {}", ty, attr, x));
                        }
                        crate::cabi::ArgAbi::Indirect { byval: true, align } => {
                            let lt = self.llty(pt);
                            argv.push(format!("ptr byval({}) align {} {}", lt, align, buf));
                        }
                        crate::cabi::ArgAbi::Indirect { byval: false, .. } => argv.push(format!("ptr {}", buf)),
                        crate::cabi::ArgAbi::Direct => unreachable!(),
                    }
                }
            }
        }
        let call = format!("{}({})", name, argv.join(", "));
        let cc = self.c_cc();
        match (&rabi, ret_buf) {
            (crate::cabi::RetAbi::Direct, _) if self.is_void(ret_ty) || self.tcx().is_never(ret_ty) => {
                self.ins(cx, format!("call {}void {}", cc, call));
            }
            (crate::cabi::RetAbi::Direct, _) => {
                let lt = self.llty(ret_ty);
                let r = self.val(cx, format!("call {}{} {}", cc, lt, call));
                if let Some(d) = dest {
                    let (a, _) = self.place_addr(cx, d);
                    self.ins(cx, format!("store {} {}, ptr {}", lt, r, a));
                }
            }
            (crate::cabi::RetAbi::Agg(ty), Some(buf)) => {
                let r = self.val(cx, format!("call {}{} {}", cc, ty, call));
                self.ins(cx, format!("store {} {}, ptr {}", ty, r, buf));
                self.copy_c_result(cx, &buf, ret_ty, dest);
            }
            (crate::cabi::RetAbi::Sret { .. }, Some(buf)) => {
                self.ins(cx, format!("call {}void {}", cc, call));
                self.copy_c_result(cx, &buf, ret_ty, dest);
            }
            _ => unreachable!(),
        }
    }

    fn copy_c_result(&mut self, cx: &mut FnCx, buf: &str, t: Ty, dest: Option<&Place>) {
        if let Some(d) = dest {
            let (a, _) = self.place_addr(cx, d);
            let size = self.size_of(t);
            self.memcpy(cx, &a, buf, size);
        }
    }

    fn emit_call(&mut self, cx: &mut FnCx, callee: &Callee, args: &[Operand], dest: Option<&Place>, span: Span) {
        let tcx = self.tcx();
        match callee {
            Callee::Fn(f, targs) => {
                let targs: Vec<Ty> = targs.iter().map(|&t| tcx.subst(t, &cx.targs)).collect();
                let fd = self.p.func(*f);
                if let Some(name) = fd.intrinsic() {
                    let name = name.to_string();
                    self.emit_intrinsic(cx, &name, &targs, args, dest, span);
                    return;
                }
                if let FnKind::Extern { .. } = fd.kind {
                    let name = self.extern_fn(*f);
                    let ret_ty = fd.ret;
                    let ptys: Vec<Ty> = fd.params.iter().map(|p| p.ty).collect();
                    self.emit_c_call(cx, &name, &ptys, ret_ty, args, dest);
                    return;
                }
                if fd.is_trait_decl() {
                    match self.p.resolve_trait_method(*f, &targs) {
                        Some((g, gargs)) => {
                            let name = self.instance(g, gargs.clone());
                            let sig = self.fn_sig(g, &gargs);
                            self.call_with_sig(cx, &name, &sig, args, dest, span);
                        }
                        None => self.emit_dyn_call(cx, *f, &targs, args, dest, span),
                    }
                    return;
                }
                let name = self.instance(*f, targs.clone());
                let sig = self.fn_sig(*f, &targs);
                self.call_with_sig(cx, &name, &sig, args, dest, span);
            }
            Callee::Indirect(fop) => {
                let (fv, ft) = self.operand(cx, fop);
                let sig = self.fn_ty_sig(ft);
                if let Val::Mem(fat) = &fv {
                    // An anonymous function: code and environment.
                    let fat = fat.clone();
                    let code = self.val(cx, format!("load ptr, ptr {}", fat));
                    let ep = self.val(cx, format!("getelementptr inbounds {{ ptr, ptr }}, ptr {}, i32 0, i32 1", fat));
                    let env = self.val(cx, format!("load ptr, ptr {}", ep));
                    self.call_with_sig_ext(cx, &code, &sig, args, dest, span, Some(&env), None);
                    return;
                }
                let target = fv.scalar().to_string();
                self.call_with_sig(cx, &target, &sig, args, dest, span);
            }
            Callee::Builtin(_) => unreachable!("builtins are lowered to runtime calls"),
            Callee::Prim(op) => self.emit_prim(cx, *op, args, dest, span),
            Callee::Runtime(rt) => self.emit_runtime(cx, rt, args, dest, span),
        }
    }

    /// A method call on a `dyn Trait` value: through the vtable of the concrete type,
    /// with the address of the value as `self`.
    fn emit_dyn_call(&mut self, cx: &mut FnCx, decl: FnId, targs: &[Ty], args: &[Operand], dest: Option<&Place>, span: Span) {
        let tr0 = match &*self.tcx().kind(targs[0]) {
            TyKind::Dyn(t) => *t,
            _ => panic!("internal: dynamic call on a non-dyn type"),
        };
        let fd = self.p.func(decl);
        let (mtrait, mname) = (fd.trait_id, fd.name.clone());
        let slot = self
            .vtable_methods(tr0)
            .iter()
            .position(|(t, n, _)| Some(*t) == mtrait && *n == mname)
            .unwrap_or_else(|| panic!("internal: `{}` is not in the vtable", mname));
        let (rv, _) = self.operand(cx, &args[0]);
        let fat = rv.scalar().to_string();
        let data = self.val(cx, format!("load ptr, ptr {}", fat));
        let vtp = self.val(cx, format!("getelementptr inbounds {{ ptr, ptr }}, ptr {}, i32 0, i32 1", fat));
        let vt = self.val(cx, format!("load ptr, ptr {}", vtp));
        let ep = self.val(cx, format!("getelementptr inbounds ptr, ptr {}, i32 {}", vt, 3 + slot));
        let fptr = self.val(cx, format!("load ptr, ptr {}", ep));
        let sig = self.fn_sig(decl, targs);
        self.call_with_sig_ext(cx, &fptr, &sig, args, dest, span, None, Some(&data));
    }

    /// Passes the operands according to `sig` and stores the result in `dest`.
    fn call_with_sig(&mut self, cx: &mut FnCx, target: &str, sig: &Sig, args: &[Operand], dest: Option<&Place>, span: Span) {
        self.call_with_sig_ext(cx, target, sig, args, dest, span, None, None);
    }

    /// `env`: the environment of an anonymous function; `first`: the pointer to pass
    /// as first argument instead of the first operand (`dyn` receivers).
    #[allow(clippy::too_many_arguments)]
    fn call_with_sig_ext(&mut self, cx: &mut FnCx, target: &str, sig: &Sig, args: &[Operand], dest: Option<&Place>, span: Span, env: Option<&str>, first: Option<&str>) {
        let tcx = self.tcx();
        let mut argv = Vec::new();
        if let RetKind::Sret(t) = &sig.ret {
            let slot = match dest {
                Some(d) => self.place_addr(cx, d).0,
                None => self.alloca(cx, *t),
            };
            argv.push(format!("ptr {}", slot));
        }
        if sig.env {
            argv.push(format!("ptr {}", env.unwrap_or("null")));
        }
        for (i, (a, pk)) in args.iter().zip(sig.params.iter()).enumerate() {
            if let (0, Some(f)) = (i, first) {
                argv.push(format!("ptr {}", f));
                continue;
            }
            let (mut v, t) = self.operand(cx, a);
            // Generic code passes a reference where the concrete type is passed by value.
            if let (PKind::Scalar(lt), Some(&pt)) = (pk, sig.param_tys.get(i)) {
                if t == tcx.ptr(pt) {
                    let x = self.val(cx, format!("load {}, ptr {}", lt, v.scalar()));
                    v = Val::Scalar(lt.clone(), x);
                }
            }
            match (pk, &v) {
                (PKind::Scalar(lt), Val::Scalar(_, x)) => argv.push(format!("{} {}", lt, x)),
                (PKind::Ptr, Val::Scalar(_, x)) => argv.push(format!("ptr {}", x)),
                (PKind::Ptr, Val::Mem(src)) => {
                    // The callee owns its copy of an aggregate argument.
                    let tmp = self.alloca(cx, t);
                    let size = self.size_of(t);
                    let src = src.clone();
                    self.memcpy(cx, &tmp, &src, size);
                    argv.push(format!("ptr {}", tmp));
                }
                (PKind::Ptr, Val::Unit) => {
                    let tmp = self.alloca(cx, t);
                    argv.push(format!("ptr {}", tmp));
                }
                (pk, v) => panic!("internal: argument {:?} does not match parameter {:?} in call to {}", v, pk, target),
            }
        }
        if sig.track_caller {
            let loc = self.loc(cx, span);
            argv.push(format!("ptr {}", loc));
        }
        match &sig.ret {
            RetKind::Scalar(lt) => {
                let r = self.val(cx, format!("call {} {}({})", lt, target, argv.join(", ")));
                if let Some(d) = dest {
                    let (a, _) = self.place_addr(cx, d);
                    self.ins(cx, format!("store {} {}, ptr {}", lt, r, a));
                }
            }
            _ => self.ins(cx, format!("call void {}({})", target, argv.join(", "))),
        }
    }

    /// Calls a runtime function with already formatted arguments.
    fn call_lang(&mut self, cx: &mut FnCx, name: &str, args: Vec<String>, dest: Option<&Place>) -> Option<String> {
        let (f, sig) = self.lang_fn(name);
        match &sig.ret {
            RetKind::Void => {
                self.ins(cx, format!("call void {}({})", f, args.join(", ")));
                None
            }
            RetKind::Scalar(lt) => {
                let lt = lt.clone();
                let r = self.val(cx, format!("call {} {}({})", lt, f, args.join(", ")));
                if let Some(d) = dest {
                    let (a, _) = self.place_addr(cx, d);
                    self.ins(cx, format!("store {} {}, ptr {}", lt, r, a));
                }
                Some(r)
            }
            RetKind::Sret(t) => {
                let t = *t;
                let slot = match dest {
                    Some(d) => self.place_addr(cx, d).0,
                    None => self.alloca(cx, t),
                };
                let mut all = vec![format!("ptr {}", slot)];
                all.extend(args);
                self.ins(cx, format!("call void {}({})", f, all.join(", ")));
                None
            }
        }
    }

    fn emit_runtime(&mut self, cx: &mut FnCx, rt: &RtFn, args: &[Operand], dest: Option<&Place>, span: Span) {
        let tcx = self.tcx();
        let vals: Vec<(Val, Ty)> = args.iter().map(|a| self.operand(cx, a)).collect();
        let p = |i: usize| -> String { format!("ptr {}", vals[i].0.scalar()) };
        match rt {
            RtFn::StrEmpty => {
                let s = self.string_value(b"");
                if let Some(d) = dest {
                    let (a, t) = self.place_addr(cx, d);
                    self.store(cx, &a, &Val::Mem(s), t);
                }
            }
            RtFn::StrPushStr => {
                self.call_lang(cx, "str_push_str", vec![p(0), p(1)], None);
            }
            RtFn::StrConcat => {
                self.call_lang(cx, "str_concat", vec![p(0), p(1)], dest);
            }
            RtFn::StrEq => {
                self.call_lang(cx, "str_eq", vec![p(0), p(1)], dest);
            }
            RtFn::StrCmp => {
                self.call_lang(cx, "str_cmp", vec![p(0), p(1)], dest);
            }
            RtFn::StrDecodeChar => {
                self.call_lang(cx, "str_decode", vec![p(0), p(1)], dest);
            }
            RtFn::Print { newline, stderr } => {
                self.call_lang(cx, "print", vec![p(0), format!("i1 {}", newline), format!("i1 {}", stderr)], None);
            }
            RtFn::PanicStr => {
                let loc = self.loc(cx, span);
                self.call_lang(cx, "panic", vec![p(0), format!("ptr {}", loc)], None);
            }
            RtFn::PanicMsg(m) => {
                let s = self.string_value(m.as_bytes());
                let loc = self.loc(cx, span);
                self.call_lang(cx, "panic", vec![format!("ptr {}", s), format!("ptr {}", loc)], None);
            }
            RtFn::Fmt { spec } => {
                let sp = self.spec(spec);
                let (v, vt) = &vals[1];
                let d = p(0);
                match &*tcx.kind(*vt) {
                    TyKind::Int(i) => {
                        let bits = self.int_bits(*i);
                        let x = v.scalar().to_string();
                        let w = if bits == 64 { x } else { self.val(cx, format!("{} i{} {} to i64", if i.signed() { "sext" } else { "zext" }, bits, x)) };
                        let name = if i.signed() { "fmt_i64" } else { "fmt_u64" };
                        self.call_lang(cx, name, vec![d, format!("i64 {}", w), format!("ptr {}", sp)], None);
                    }
                    TyKind::Float(f) => {
                        let x = v.scalar().to_string();
                        let w = if *f == FloatTy::F32 { self.val(cx, format!("fpext float {} to double", x)) } else { x };
                        self.call_lang(cx, "fmt_f64", vec![d, format!("double {}", w), format!("i1 {}", *f == FloatTy::F32), format!("ptr {}", sp)], None);
                    }
                    TyKind::Bool => {
                        self.call_lang(cx, "fmt_bool", vec![d, format!("i1 {}", v.scalar()), format!("ptr {}", sp)], None);
                    }
                    TyKind::Char => {
                        self.call_lang(cx, "fmt_char", vec![d, format!("i32 {}", v.scalar()), format!("ptr {}", sp)], None);
                    }
                    TyKind::Ptr(_) => {
                        self.call_lang(cx, "fmt_str", vec![d, format!("ptr {}", v.scalar()), format!("ptr {}", sp)], None);
                    }
                    _ => panic!("internal: unformattable value of type {}", self.p.ty_str(*vt)),
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Primitive operations

    fn store_result(&mut self, cx: &mut FnCx, dest: Option<&Place>, lt: &str, v: &str) {
        if let Some(d) = dest {
            let (a, _) = self.place_addr(cx, d);
            self.ins(cx, format!("store {} {}, ptr {}", lt, v, a));
        }
    }

    fn emit_prim(&mut self, cx: &mut FnCx, op: PrimOp, args: &[Operand], dest: Option<&Place>, span: Span) {
        let tcx = self.tcx();
        let vals: Vec<(Val, Ty)> = args.iter().map(|a| self.operand(cx, a)).collect();
        let recv_ty = vals.first().map(|v| v.1).unwrap_or(tcx.unit);
        let int = tcx.as_int(recv_ty);
        let float = tcx.as_float(recv_ty);
        let x = || vals[0].0.scalar().to_string();
        let y = || vals[1].0.scalar().to_string();
        match op {
            PrimOp::IntAbs => {
                let i = int.unwrap();
                let bits = self.int_bits(i);
                let min = int_const(-(1i128 << (bits - 1)), bits);
                let is_min = self.val(cx, format!("icmp eq i{} {}, {}", bits, x(), min));
                self.check(cx, &is_min, kind::ABS, span);
                let neg = self.val(cx, format!("icmp slt i{} {}, 0", bits, x()));
                let m = self.val(cx, format!("sub i{} 0, {}", bits, x()));
                let r = self.val(cx, format!("select i1 {}, i{} {}, i{} {}", neg, bits, m, bits, x()));
                self.store_result(cx, dest, &format!("i{}", bits), &r);
            }
            PrimOp::IntMin | PrimOp::IntMax => {
                let i = int.unwrap();
                let bits = self.int_bits(i);
                let pred = match (op == PrimOp::IntMin, i.signed()) {
                    (true, true) => "slt",
                    (true, false) => "ult",
                    (false, true) => "sgt",
                    (false, false) => "ugt",
                };
                let c = self.val(cx, format!("icmp {} i{} {}, {}", pred, bits, x(), y()));
                let r = self.val(cx, format!("select i1 {}, i{} {}, i{} {}", c, bits, x(), bits, y()));
                self.store_result(cx, dest, &format!("i{}", bits), &r);
            }
            PrimOp::IntPow => {
                let i = int.unwrap();
                let helper = self.pow_helper(i);
                let bits = self.int_bits(i);
                let loc = self.loc(cx, span);
                let r = self.val(cx, format!("call i{b} {}(i{b} {}, i32 {}, ptr {})", helper, x(), y(), loc, b = bits));
                self.store_result(cx, dest, &format!("i{}", bits), &r);
            }
            PrimOp::IntLeadingZeros | PrimOp::IntTrailingZeros | PrimOp::IntCountOnes => {
                let bits = self.int_bits(int.unwrap());
                let name = match op {
                    PrimOp::IntLeadingZeros => "ctlz",
                    PrimOp::IntTrailingZeros => "cttz",
                    _ => "ctpop",
                };
                let r = if op == PrimOp::IntCountOnes {
                    self.declare(&format!("declare i{b} @llvm.ctpop.i{b}(i{b})", b = bits));
                    self.val(cx, format!("call i{b} @llvm.ctpop.i{b}(i{b} {})", x(), b = bits))
                } else {
                    self.declare(&format!("declare i{b} @llvm.{n}.i{b}(i{b}, i1)", n = name, b = bits));
                    self.val(cx, format!("call i{b} @llvm.{n}.i{b}(i{b} {}, i1 false)", x(), n = name, b = bits))
                };
                let r32 = match bits.cmp(&32) {
                    std::cmp::Ordering::Equal => r,
                    std::cmp::Ordering::Less => self.val(cx, format!("zext i{} {} to i32", bits, r)),
                    std::cmp::Ordering::Greater => self.val(cx, format!("trunc i{} {} to i32", bits, r)),
                };
                self.store_result(cx, dest, "i32", &r32);
            }
            PrimOp::IntRotateLeft | PrimOp::IntRotateRight => {
                let bits = self.int_bits(int.unwrap());
                let m = self.val(cx, format!("urem i32 {}, {}", y(), bits));
                let amt = match bits.cmp(&32) {
                    std::cmp::Ordering::Equal => m,
                    std::cmp::Ordering::Less => self.val(cx, format!("trunc i32 {} to i{}", m, bits)),
                    std::cmp::Ordering::Greater => self.val(cx, format!("zext i32 {} to i{}", m, bits)),
                };
                let name = if op == PrimOp::IntRotateLeft { "fshl" } else { "fshr" };
                self.declare(&format!("declare i{b} @llvm.{n}.i{b}(i{b}, i{b}, i{b})", n = name, b = bits));
                let r = self.val(cx, format!("call i{b} @llvm.{n}.i{b}(i{b} {x}, i{b} {x}, i{b} {a})", n = name, b = bits, x = x(), a = amt));
                self.store_result(cx, dest, &format!("i{}", bits), &r);
            }
            PrimOp::IntSwapBytes => {
                let bits = self.int_bits(int.unwrap());
                let r = if bits == 8 {
                    x()
                } else {
                    self.declare(&format!("declare i{b} @llvm.bswap.i{b}(i{b})", b = bits));
                    self.val(cx, format!("call i{b} @llvm.bswap.i{b}(i{b} {})", x(), b = bits))
                };
                self.store_result(cx, dest, &format!("i{}", bits), &r);
            }
            PrimOp::FloatUnary(name) => {
                let f = float.unwrap();
                let lt = self.llty(recv_ty);
                let sfx = if f == FloatTy::F32 { "f32" } else { "f64" };
                let r = match name {
                    "sqrt" | "abs" => {
                        let intr = if name == "sqrt" { "sqrt" } else { "fabs" };
                        self.declare(&format!("declare {lt} @llvm.{i}.{s}({lt})", lt = lt, i = intr, s = sfx));
                        self.val(cx, format!("call {lt} @llvm.{i}.{s}({lt} {x})", lt = lt, i = intr, s = sfx, x = x()))
                    }
                    other => self.float_call1(cx, &format!("math.{}", other), f, &x()),
                };
                self.store_result(cx, dest, &lt, &r);
            }
            PrimOp::FloatBinary(name) => {
                let f = float.unwrap();
                let lt = self.llty(recv_ty);
                let sfx = if f == FloatTy::F32 { "f32" } else { "f64" };
                let r = match name {
                    "min" | "max" | "copysign" => {
                        let intr = match name {
                            "min" => "minnum",
                            "max" => "maxnum",
                            _ => "copysign",
                        };
                        self.declare(&format!("declare {lt} @llvm.{i}.{s}({lt}, {lt})", lt = lt, i = intr, s = sfx));
                        self.val(cx, format!("call {lt} @llvm.{i}.{s}({lt} {x}, {lt} {y})", lt = lt, i = intr, s = sfx, x = x(), y = y()))
                    }
                    other => self.float_call2(cx, &format!("math.{}", other), f, &x(), &y()),
                };
                self.store_result(cx, dest, &lt, &r);
            }
            PrimOp::FloatIsNan => {
                let lt = self.llty(recv_ty);
                let r = self.val(cx, format!("fcmp uno {} {}, {}", lt, x(), x()));
                self.store_result(cx, dest, "i1", &r);
            }
            PrimOp::FloatIsInfinite | PrimOp::FloatIsFinite => {
                let f = float.unwrap();
                let lt = self.llty(recv_ty);
                let sfx = if f == FloatTy::F32 { "f32" } else { "f64" };
                self.declare(&format!("declare {lt} @llvm.fabs.{s}({lt})", lt = lt, s = sfx));
                let a = self.val(cx, format!("call {lt} @llvm.fabs.{s}({lt} {x})", lt = lt, s = sfx, x = x()));
                let inf = float_const(f64::INFINITY, f);
                let r = if op == PrimOp::FloatIsInfinite {
                    self.val(cx, format!("fcmp oeq {} {}, {}", lt, a, inf))
                } else {
                    self.val(cx, format!("fcmp olt {} {}, {}", lt, a, inf))
                };
                self.store_result(cx, dest, "i1", &r);
            }
            PrimOp::FloatToBits => {
                let (lt, it) = if float == Some(FloatTy::F32) { ("float", "i32") } else { ("double", "i64") };
                let r = self.val(cx, format!("bitcast {} {} to {}", lt, x(), it));
                self.store_result(cx, dest, it, &r);
            }
            PrimOp::CharPredicate(name) => {
                let c = x();
                let r = match name {
                    "is_whitespace" => {
                        let (f, _) = self.lang_fn("char_is_whitespace");
                        self.val(cx, format!("call i1 {}(i32 {})", f, c))
                    }
                    "is_ascii" => self.val(cx, format!("icmp ult i32 {}, 128", c)),
                    "is_ascii_digit" => self.char_range(cx, &c, '0', '9'),
                    "is_ascii_uppercase" => self.char_range(cx, &c, 'A', 'Z'),
                    "is_ascii_lowercase" => self.char_range(cx, &c, 'a', 'z'),
                    "is_ascii_alphabetic" => {
                        let u = self.char_range(cx, &c, 'A', 'Z');
                        let l = self.char_range(cx, &c, 'a', 'z');
                        self.val(cx, format!("or i1 {}, {}", u, l))
                    }
                    _ => {
                        let u = self.char_range(cx, &c, 'A', 'Z');
                        let l = self.char_range(cx, &c, 'a', 'z');
                        let d = self.char_range(cx, &c, '0', '9');
                        let a = self.val(cx, format!("or i1 {}, {}", u, l));
                        self.val(cx, format!("or i1 {}, {}", a, d))
                    }
                };
                self.store_result(cx, dest, "i1", &r);
            }
            PrimOp::CharToUpper | PrimOp::CharToLower => {
                let c = x();
                let (lo, hi, delta) = if op == PrimOp::CharToUpper { ('a', 'z', "-32") } else { ('A', 'Z', "32") };
                let inr = self.char_range(cx, &c, lo, hi);
                let m = self.val(cx, format!("add i32 {}, {}", c, delta));
                let r = self.val(cx, format!("select i1 {}, i32 {}, i32 {}", inr, m, c));
                self.store_result(cx, dest, "i32", &r);
            }
            PrimOp::CharLenUtf8 => {
                let c = x();
                let u = self.usize_ty();
                let a = self.val(cx, format!("icmp ult i32 {}, 128", c));
                let b = self.val(cx, format!("icmp ult i32 {}, 2048", c));
                let d = self.val(cx, format!("icmp ult i32 {}, 65536", c));
                let s3 = self.val(cx, format!("select i1 {}, {u} 3, {u} 4", d, u = u));
                let s2 = self.val(cx, format!("select i1 {}, {u} 2, {u} {}", b, s3, u = u));
                let r = self.val(cx, format!("select i1 {}, {u} 1, {u} {}", a, s2, u = u));
                self.store_result(cx, dest, u, &r);
            }
            PrimOp::ToString => {
                let (v, t) = vals[0].clone();
                match &*tcx.kind(t) {
                    TyKind::Int(i) => {
                        let bits = self.int_bits(*i);
                        let w = if bits == 64 { v.scalar().to_string() } else { self.val(cx, format!("{} i{} {} to i64", if i.signed() { "sext" } else { "zext" }, bits, v.scalar())) };
                        self.call_lang(cx, if i.signed() { "to_string_i64" } else { "to_string_u64" }, vec![format!("i64 {}", w)], dest);
                    }
                    TyKind::Float(f) => {
                        let w = if *f == FloatTy::F32 { self.val(cx, format!("fpext float {} to double", v.scalar())) } else { v.scalar().to_string() };
                        self.call_lang(cx, "to_string_f64", vec![format!("double {}", w), format!("i1 {}", *f == FloatTy::F32)], dest);
                    }
                    TyKind::Bool => {
                        self.call_lang(cx, "to_string_bool", vec![format!("i1 {}", v.scalar())], dest);
                    }
                    TyKind::Char => {
                        self.call_lang(cx, "to_string_char", vec![format!("i32 {}", v.scalar())], dest);
                    }
                    _ => {
                        if let Some(d) = dest {
                            let s = self.string_value(b"");
                            let (a, dt) = self.place_addr(cx, d);
                            self.store(cx, &a, &Val::Mem(s), dt);
                        }
                    }
                }
            }
            PrimOp::PtrNull => self.store_result(cx, dest, "ptr", "null"),
            PrimOp::PtrIsNull => {
                let r = self.val(cx, format!("icmp eq ptr {}, null", x()));
                self.store_result(cx, dest, "i1", &r);
            }
            PrimOp::PtrAddr => {
                let u = self.usize_ty();
                let r = self.val(cx, format!("ptrtoint ptr {} to {}", x(), u));
                self.store_result(cx, dest, u, &r);
            }
            PrimOp::PtrOffset => {
                let elem = match &*tcx.kind(recv_ty) {
                    TyKind::Ptr(e) => *e,
                    _ => tcx.u8,
                };
                let et = self.llty(elem);
                let u = self.usize_ty();
                let r = self.val(cx, format!("getelementptr {}, ptr {}, {} {}", et, x(), u, y()));
                self.store_result(cx, dest, "ptr", &r);
            }
            PrimOp::PtrSub => {
                let elem = match &*tcx.kind(recv_ty) {
                    TyKind::Ptr(e) => *e,
                    _ => tcx.u8,
                };
                let et = self.llty(elem);
                let u = self.usize_ty();
                let n = self.val(cx, format!("sub {} 0, {}", u, y()));
                let r = self.val(cx, format!("getelementptr {}, ptr {}, {} {}", et, x(), u, n));
                self.store_result(cx, dest, "ptr", &r);
            }
            PrimOp::PtrFromAddr => {
                let u = self.usize_ty();
                let r = self.val(cx, format!("inttoptr {} {} to ptr", u, x()));
                self.store_result(cx, dest, "ptr", &r);
            }
            PrimOp::PtrRead => {
                let elem = match &*tcx.kind(recv_ty) {
                    TyKind::Ptr(e) => *e,
                    _ => tcx.unit,
                };
                if let Some(d) = dest {
                    let src = x();
                    let v = self.load(cx, &src, elem);
                    let (a, _) = self.place_addr(cx, d);
                    self.store(cx, &a, &v, elem);
                }
            }
            PrimOp::PtrWrite => {
                let (v, t) = vals[1].clone();
                let p = x();
                self.store(cx, &p, &v, t);
            }
            PrimOp::PtrCast => self.store_result(cx, dest, "ptr", &x()),
        }
    }

    fn char_range(&mut self, cx: &mut FnCx, c: &str, lo: char, hi: char) -> String {
        let a = self.val(cx, format!("sub i32 {}, {}", c, lo as u32));
        self.val(cx, format!("icmp ule i32 {}, {}", a, hi as u32 - lo as u32))
    }

    /// Overflow-checked 64-bit multiplication from 32-bit pieces, for 32-bit targets:
    /// returns `{product, overflow}` like `llvm.{s,u}mul.with.overflow.i64`.
    fn mul64_helper(&mut self, signed: bool) -> String {
        let name = if signed { "@nx.smul.ov.i64" } else { "@nx.umul.ov.i64" };
        if !self.helpers.insert(name.to_string()) {
            return name.to_string();
        }
        if signed && !self.helpers.insert("@nx.umul.ov.i64".to_string()) {
            // The unsigned helper, used by the signed one, already exists.
        } else {
        // Unsigned: overflow if both high halves are nonzero, or if the cross products
        // plus the carry of the low product exceed 32 bits.
        let _ = write!(
            self.fn_defs,
            "define internal {{i64, i1}} @nx.umul.ov.i64(i64 %a, i64 %b) nounwind {{
start:
  %ah = lshr i64 %a, 32
  %bh = lshr i64 %b, 32
  %al = and i64 %a, 4294967295
  %bl = and i64 %b, 4294967295
  %ahz = icmp ne i64 %ah, 0
  %bhz = icmp ne i64 %bh, 0
  %both = and i1 %ahz, %bhz
  %c1 = mul i64 %ah, %bl
  %c2 = mul i64 %al, %bh
  %cross = add i64 %c1, %c2
  %low = mul i64 %al, %bl
  %lowh = lshr i64 %low, 32
  %t = add i64 %cross, %lowh
  %big = icmp uge i64 %t, 4294967296
  %ov = or i1 %both, %big
  %p = mul i64 %a, %b
  %r0 = insertvalue {{i64, i1}} undef, i64 %p, 0
  %r1 = insertvalue {{i64, i1}} %r0, i1 %ov, 1
  ret {{i64, i1}} %r1
}}

"
        );
        }
        if signed {
            let _ = write!(
                self.fn_defs,
                "define internal {{i64, i1}} @nx.smul.ov.i64(i64 %a, i64 %b) nounwind {{
start:
  %an = icmp slt i64 %a, 0
  %bn = icmp slt i64 %b, 0
  %na = sub i64 0, %a
  %nb = sub i64 0, %b
  %ma = select i1 %an, i64 %na, i64 %a
  %mb = select i1 %bn, i64 %nb, i64 %b
  %u = call {{i64, i1}} @nx.umul.ov.i64(i64 %ma, i64 %mb)
  %uov = extractvalue {{i64, i1}} %u, 1
  %m = extractvalue {{i64, i1}} %u, 0
  %neg = xor i1 %an, %bn
  %lim = select i1 %neg, i64 -9223372036854775808, i64 9223372036854775807
  %over = icmp ugt i64 %m, %lim
  %ov = or i1 %uov, %over
  %p = mul i64 %a, %b
  %r0 = insertvalue {{i64, i1}} undef, i64 %p, 0
  %r1 = insertvalue {{i64, i1}} %r0, i1 %ov, 1
  ret {{i64, i1}} %r1
}}

"
            );
        }
        name.to_string()
    }

    /// `x.pow(n)` for an integer type: checked multiplications.
    fn pow_helper(&mut self, i: IntTy) -> String {
        let bits = self.int_bits(i);
        let name = format!("@nx.pow.{}{}", if i.signed() { "i" } else { "u" }, bits);
        if !self.helpers.insert(name.clone()) {
            return name;
        }
        let s = if i.signed() { "s" } else { "u" };
        let (pf, _) = self.lang_fn("panic_arith");
        let mul = if bits == 64 && self.opts.target.ptr_bits == 32 {
            self.mul64_helper(i.signed())
        } else {
            self.declare(&format!("declare {{i{b}, i1}} @llvm.{s}mul.with.overflow.i{b}(i{b}, i{b})", b = bits, s = s));
            format!("@llvm.{}mul.with.overflow.i{}", s, bits)
        };
        let _ = write!(
            self.fn_defs,
            "define internal i{b} {name}(i{b} %base, i32 %exp, ptr %loc) nounwind {{
start:
  br label %loop
loop:
  %r = phi i{b} [ 1, %start ], [ %r2, %next ]
  %bs = phi i{b} [ %base, %start ], [ %b2, %next ]
  %e = phi i32 [ %exp, %start ], [ %e2, %next ]
  %done = icmp eq i32 %e, 0
  br i1 %done, label %end, label %step
step:
  %odd = and i32 %e, 1
  %isodd = icmp ne i32 %odd, 0
  br i1 %isodd, label %mul, label %sq
mul:
  %m = call {{i{b}, i1}} {mul}(i{b} %r, i{b} %bs)
  %mo = extractvalue {{i{b}, i1}} %m, 1
  br i1 %mo, label %fail, label %mulok
mulok:
  %mv = extractvalue {{i{b}, i1}} %m, 0
  br label %sq
sq:
  %r1 = phi i{b} [ %r, %step ], [ %mv, %mulok ]
  %e2 = lshr i32 %e, 1
  %more = icmp ne i32 %e2, 0
  br i1 %more, label %square, label %last
square:
  %q = call {{i{b}, i1}} {mul}(i{b} %bs, i{b} %bs)
  %qo = extractvalue {{i{b}, i1}} %q, 1
  br i1 %qo, label %fail, label %sqok
sqok:
  %qv = extractvalue {{i{b}, i1}} %q, 0
  br label %next
last:
  br label %next
next:
  %r2 = phi i{b} [ %r1, %sqok ], [ %r1, %last ]
  %b2 = phi i{b} [ %qv, %sqok ], [ %bs, %last ]
  br label %loop
end:
  ret i{b} %r
fail:
  call void {pf}(i32 {k}, ptr %loc)
  unreachable
}}

",
            b = bits,
            name = name,
            mul = mul,
            pf = pf,
            k = kind::POW
        );
        name
    }

    // -----------------------------------------------------------------------
    // Intrinsics of the standard library

    fn emit_intrinsic(&mut self, cx: &mut FnCx, name: &str, targs: &[Ty], args: &[Operand], dest: Option<&Place>, span: Span) {
        let _ = span;
        let u = self.usize_ty();
        let vals: Vec<(Val, Ty)> = args.iter().map(|a| self.operand(cx, a)).collect();
        let s = |i: usize| -> String { vals[i].0.scalar().to_string() };
        match name {
            "size_of" | "align_of" => {
                let t = targs[0];
                let v = if name == "size_of" { self.size_of(t) } else { self.align_of(t) };
                self.store_result(cx, dest, u, &v.to_string());
            }
            "copy" => {
                let size = self.size_of(targs[0]);
                if size > 0 {
                    let bytes = self.val(cx, format!("mul {} {}, {}", u, s(2), size));
                    self.declare(&format!("declare void @llvm.memmove.p0.p0.{u}(ptr, ptr, {u}, i1)"));
                    self.ins(cx, format!("call void @llvm.memmove.p0.p0.{u}(ptr {}, ptr {}, {u} {}, i1 false)", s(1), s(0), bytes));
                }
            }
            "set_bytes" => {
                self.declare(&format!("declare void @llvm.memset.p0.{u}(ptr, i8, {u}, i1)"));
                self.ins(cx, format!("call void @llvm.memset.p0.{u}(ptr {}, i8 {}, {u} {}, i1 false)", s(0), s(1), s(2)));
            }
            "drop_in_place" => {
                if let Some(d) = self.drop_fn(targs[0]) {
                    self.ins(cx, format!("call void {}(ptr {})", d, s(0)));
                }
            }
            "forget" => {}
            "addr_of" => self.store_result(cx, dest, "ptr", &s(0)),
            "swap" => {
                let t = targs[0];
                let (a, b) = (s(0), s(1));
                if self.is_scalar(t) {
                    let lt = self.llty(t);
                    let va = self.val(cx, format!("load {}, ptr {}", lt, a));
                    let vb = self.val(cx, format!("load {}, ptr {}", lt, b));
                    self.ins(cx, format!("store {} {}, ptr {}", lt, vb, a));
                    self.ins(cx, format!("store {} {}, ptr {}", lt, va, b));
                } else {
                    let size = self.size_of(t);
                    let tmp = self.alloca(cx, t);
                    self.memcpy(cx, &tmp, &a, size);
                    self.memcpy(cx, &a, &b, size);
                    self.memcpy(cx, &b, &tmp, size);
                }
            }
            "replace" => {
                let t = targs[0];
                let p = s(0);
                let (v, vt) = vals[1].clone();
                match dest {
                    Some(d) => {
                        let old = self.load(cx, &p, t);
                        let (a, _) = self.place_addr(cx, d);
                        self.store(cx, &a, &old, t);
                    }
                    None => {
                        if let Some(df) = self.drop_fn(t) {
                            self.ins(cx, format!("call void {}(ptr {})", df, p));
                        }
                    }
                }
                self.store(cx, &p, &v, vt);
            }
            "read_volatile" => {
                let t = targs[0];
                if let Some(d) = dest {
                    let (a, _) = self.place_addr(cx, d);
                    if self.is_scalar(t) {
                        let lt = self.llty(t);
                        let v = self.val(cx, format!("load volatile {}, ptr {}", lt, s(0)));
                        self.ins(cx, format!("store {} {}, ptr {}", lt, v, a));
                    } else {
                        let size = self.size_of(t);
                        self.declare(&format!("declare void @llvm.memcpy.p0.p0.{u}(ptr, ptr, {u}, i1)"));
                        self.ins(cx, format!("call void @llvm.memcpy.p0.p0.{u}(ptr {}, ptr {}, {u} {}, i1 true)", a, s(0), size));
                    }
                }
            }
            "write_volatile" => {
                let t = targs[0];
                let (v, _) = vals[1].clone();
                match v {
                    Val::Scalar(lt, x) => self.ins(cx, format!("store volatile {} {}, ptr {}", lt, x, s(0))),
                    Val::Mem(src) => {
                        let size = self.size_of(t);
                        self.declare(&format!("declare void @llvm.memcpy.p0.p0.{u}(ptr, ptr, {u}, i1)"));
                        self.ins(cx, format!("call void @llvm.memcpy.p0.p0.{u}(ptr {}, ptr {}, {u} {}, i1 true)", s(0), src, size));
                    }
                    Val::Unit => {}
                }
            }
            "zeroed" => {
                let t = targs[0];
                if let Some(d) = dest {
                    let (a, _) = self.place_addr(cx, d);
                    let size = self.size_of(t);
                    if size > 0 {
                        self.declare(&format!("declare void @llvm.memset.p0.{u}(ptr, i8, {u}, i1)"));
                        self.ins(cx, format!("call void @llvm.memset.p0.{u}(ptr {}, i8 0, {u} {}, i1 false)", a, size));
                    }
                }
            }
            "f64_from_bits" => {
                let r = self.val(cx, format!("bitcast i64 {} to double", s(0)));
                self.store_result(cx, dest, "double", &r);
            }
            "f32_from_bits" => {
                let r = self.val(cx, format!("bitcast i32 {} to float", s(0)));
                self.store_result(cx, dest, "float", &r);
            }
            "syscall6" => {
                let w = u;
                let (insn, regs) = match self.opts.target.arch {
                    Arch::X86_64 => ("syscall", "={rax},{rax},{rdi},{rsi},{rdx},{r10},{r8},{r9},~{rcx},~{r11},~{memory}"),
                    Arch::Aarch64 => ("svc #0", "={x0},{x8},{x0},{x1},{x2},{x3},{x4},{x5},~{memory}"),
                    _ => ("ecall", "={x10},{x17},{x10},{x11},{x12},{x13},{x14},{x15},~{memory}"),
                };
                let argv: Vec<String> = (0..7).map(|i| format!("{} {}", w, s(i))).collect();
                let r = self.val(cx, format!("call {} asm sideeffect \"{}\", \"{}\"({})", w, insn, regs, argv.join(", ")));
                self.store_result(cx, dest, w, &r);
            }
            // Atomic operations on integers, sequentially consistent (ADR 0013): the lock of
            // the runtime, the end of threads, `std.tache`.
            "atomic_load" | "atomic_store" | "atomic_swap" | "atomic_add" | "atomic_cas" => {
                let t = targs[0];
                let lt = self.llty(t);
                let al = self.align_of(t);
                // The integer operands, by value (loaded if one was passed by reference).
                let ints: Vec<String> = vals
                    .iter()
                    .skip(1)
                    .map(|(v, _)| match v {
                        Val::Scalar(vt, x) if *vt == lt => x.clone(),
                        Val::Scalar(_, x) | Val::Mem(x) => self.val(cx, format!("load {lt}, ptr {x}, align {al}")),
                        Val::Unit => panic!("internal: atomic operand without a value"),
                    })
                    .collect();
                let s = |i: usize| -> String { if i == 0 { vals[0].0.scalar().to_string() } else { ints[i - 1].clone() } };
                match name {
                    "atomic_load" => {
                        let r = self.val(cx, format!("load atomic {lt}, ptr {} seq_cst, align {al}", s(0)));
                        self.store_result(cx, dest, &lt, &r);
                    }
                    "atomic_store" => self.ins(cx, format!("store atomic {lt} {}, ptr {} seq_cst, align {al}", s(1), s(0))),
                    "atomic_cas" => {
                        let pair = self.val(cx, format!("cmpxchg ptr {}, {lt} {}, {lt} {} seq_cst seq_cst, align {al}", s(0), s(1), s(2)));
                        let r = self.val(cx, format!("extractvalue {{ {lt}, i1 }} {}, 0", pair));
                        self.store_result(cx, dest, &lt, &r);
                    }
                    _ => {
                        let op = if name == "atomic_swap" { "xchg" } else { "add" };
                        let r = self.val(cx, format!("atomicrmw {op} ptr {}, {lt} {} seq_cst, align {al}", s(0), s(1)));
                        self.store_result(cx, dest, &lt, &r);
                    }
                }
            }
            "atomic_fence" => self.ins(cx, "fence seq_cst"),
            // The code and the environment of an anonymous function, and a call of them
            // (`std.tache` runs the function in another thread).
            "closure_code" | "closure_env" => {
                // The pair `{ code, environment }` in memory, or its address when the function
                // was received by reference.
                let fat = match &vals[0].0 {
                    Val::Mem(a) | Val::Scalar(_, a) => a.clone(),
                    Val::Unit => panic!("internal: `{}` of a value that is not an anonymous function", name),
                };
                let at = if name == "closure_code" { fat } else { self.val(cx, format!("getelementptr inbounds {{ ptr, ptr }}, ptr {}, i32 0, i32 1", fat)) };
                let r = self.val(cx, format!("load ptr, ptr {}", at));
                self.store_result(cx, dest, "ptr", &r);
            }
            "call_closure" => self.ins(cx, format!("call void {}(ptr {})", s(0), s(1))),
            // A thread of Linux (ADR 0013): `clone` with a new stack; the child calls
            // `entry(arg)`, then ends the thread. The kernel writes the thread id to `tid` and
            // clears it (waking a futex) when the thread ends.
            "linux_clone" => {
                let w = u;
                let (insn, regs) = match self.opts.target.arch {
                    Arch::X86_64 => (
                        "syscall\\0Atest %rax, %rax\\0Ajnz 1f\\0Axor %ebp, %ebp\\0Amov %r12, %rdi\\0Acall *%r13\\0Amov $$60, %eax\\0Axor %edi, %edi\\0Asyscall\\0Aud2\\0A1:",
                        // clone(flags, stack, parent_tid, child_tid, tls)
                        "={rax},{rax},{rdi},{rsi},{rdx},{r10},{r8},{r12},{r13},~{rcx},~{r11},~{memory}",
                    ),
                    Arch::Aarch64 => (
                        "svc #0\\0Acbnz x0, 1f\\0Amov x29, xzr\\0Amov x0, x20\\0Ablr x19\\0Amov x8, #93\\0Amov x0, xzr\\0Asvc #0\\0Abrk #0\\0A1:",
                        // clone(flags, stack, parent_tid, tls, child_tid)
                        "={x0},{x8},{x0},{x1},{x2},{x4},{x3},{x20},{x19},~{memory}",
                    ),
                    Arch::Riscv64 => (
                        "ecall\\0Abnez a0, 1f\\0Amv s0, zero\\0Amv a0, s2\\0Ajalr s1\\0Ali a7, 93\\0Ali a0, 0\\0Aecall\\0Aunimp\\0A1:",
                        // clone(flags, stack, parent_tid, tls, child_tid)
                        "={x10},{x17},{x10},{x11},{x12},{x14},{x13},{x18},{x9},~{memory}",
                    ),
                    _ => panic!("internal: linux_clone on an architecture without Linux threads"),
                };
                let number = if self.opts.target.arch == Arch::X86_64 { 56 } else { 220 };
                // number, flags, stack, parent tid, child tid (the same word), tls, arg, entry
                let argv = format!("{w} {number}, {w} {}, ptr {}, ptr {}, ptr {}, {w} 0, ptr {}, ptr {}", s(0), s(1), s(2), s(2), s(4), s(3));
                let r = self.val(cx, format!("call {w} asm sideeffect \"{insn}\", \"{regs}\"({argv})"));
                self.store_result(cx, dest, w, &r);
            }
            "wasm_memory_grow" => {
                self.declare("declare i32 @llvm.wasm.memory.grow.i32(i32, i32)");
                let r = self.val(cx, format!("call i32 @llvm.wasm.memory.grow.i32(i32 0, i32 {})", s(0)));
                self.store_result(cx, dest, "i32", &r);
            }
            "wasm_memory_size" => {
                self.declare("declare i32 @llvm.wasm.memory.size.i32(i32)");
                let r = self.val(cx, "call i32 @llvm.wasm.memory.size.i32(i32 0)");
                self.store_result(cx, dest, "i32", &r);
            }
            "irq_save" => {
                let (insn, regs) = match self.opts.target.arch {
                    Arch::Arm => ("mrs $0, primask\\0Acpsid i", "=r,~{memory}"),
                    Arch::Aarch64 => ("mrs $0, daif\\0Amsr daifset, #3", "=r,~{memory}"),
                    Arch::Riscv32 | Arch::Riscv64 => ("csrrci $0, mstatus, 8", "=r,~{memory}"),
                    _ => panic!("internal: irq_save on a non-bare architecture"),
                };
                let r = self.val(cx, format!("call {u} asm sideeffect \"{}\", \"{}\"()", insn, regs));
                self.store_result(cx, dest, u, &r);
            }
            "irq_restore" => {
                match self.opts.target.arch {
                    Arch::Arm => self.ins(cx, format!("call void asm sideeffect \"msr primask, $0\", \"r,~{{memory}}\"({u} {})", s(0))),
                    Arch::Aarch64 => self.ins(cx, format!("call void asm sideeffect \"msr daif, $0\", \"r,~{{memory}}\"({u} {})", s(0))),
                    Arch::Riscv32 | Arch::Riscv64 => {
                        // Only restore MIE, not unrelated status bits (notably MPP).
                        let enabled = self.val(cx, format!("and {u} {}, 8", s(0)));
                        self.ins(cx, format!("call void asm sideeffect \"csrs mstatus, $0\", \"r,~{{memory}}\"({u} {})", enabled));
                    }
                    _ => panic!("internal: irq_restore on a non-bare architecture"),
                }
            }
            "semihost" => {
                let (insn, regs) = match self.opts.target.arch {
                    Arch::Arm => ("bkpt #0xab", "={r0},{r0},{r1},~{memory}"),
                    Arch::Aarch64 => ("hlt #0xf000", "={x0},{x0},{x1},~{memory}"),
                    _ => (".balign 16\\0A.option push\\0A.option norvc\\0Aslli x0, x0, 0x1f\\0Aebreak\\0Asrai x0, x0, 7\\0A.option pop", "={x10},{x10},{x11},~{memory}"),
                };
                let r = self.val(cx, format!("call {u} asm sideeffect \"{}\", \"{}\"({u} {}, {u} {})", insn, regs, s(0), s(1), u = u));
                self.store_result(cx, dest, u, &r);
            }
            "heap_start" | "stack_top" => {
                let g = if name == "heap_start" { "@__heap_start" } else { "@__stack_top" };
                self.declare(&format!("@{} = external global i8", crate::quote_name(&g[1..])));
                self.store_result(cx, dest, "ptr", g);
            }
            "trap" => {
                self.declare("declare void @llvm.trap()");
                self.ins(cx, "call void @llvm.trap()");
            }
            "breakpoint" => {
                self.declare("declare void @llvm.debugtrap()");
                self.ins(cx, "call void @llvm.debugtrap()");
            }
            other => panic!("internal: unknown intrinsic `{}`", other),
        }
    }
}

fn cmp_pred(op: BinOp, signed: bool) -> &'static str {
    match (op, signed) {
        (BinOp::Eq, _) => "eq",
        (BinOp::Ne, _) => "ne",
        (BinOp::Lt, true) => "slt",
        (BinOp::Lt, false) => "ult",
        (BinOp::Le, true) => "sle",
        (BinOp::Le, false) => "ule",
        (BinOp::Gt, true) => "sgt",
        (BinOp::Gt, false) => "ugt",
        (BinOp::Ge, true) => "sge",
        (BinOp::Ge, false) => "uge",
        _ => "eq",
    }
}

#[allow(dead_code)]
fn unused(_: &Os, _: AdtKind) {}

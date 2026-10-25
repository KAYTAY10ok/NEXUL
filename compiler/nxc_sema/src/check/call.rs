//! Paths, calls, constructors and methods.

use super::{Deferred, Expect, FnCtxt};
use crate::collect::{lookup_in_module, lookup_item, primitive_type};
use crate::consts::prim_assoc_const;
use crate::defs::{AdtId, Builtin, ConstId, ConstValue, FnId, ItemRef, ModId, StaticId, TraitId};
use crate::thir::{self, CallArg, Callee, ExprKind as T, PrimOp};
use crate::ty::{Ty, TyKind};
use crate::util::{best_match, PRIMITIVE_TYPES};
use nxc_syntax::ast::{self, Convention, ExprKind as A, TypeExpr, TypeExprKind};
use nxc_syntax::{Diagnostic, Span};

/// What a path expression designates.
#[derive(Clone, Debug)]
pub(crate) enum PathRes {
    Local(thir::LocalId),
    Module(ModId),
    /// A struct or enum, with explicit generic arguments if written.
    Adt(AdtId, Option<Vec<Ty>>),
    /// A primitive type or `Ptr[T]` used as a namespace.
    Prim(Ty),
    Fn(FnId, Option<Vec<Ty>>),
    AssocFn { f: FnId, owner_args: Vec<Ty>, explicit: Option<Vec<Ty>> },
    Variant(AdtId, u32, Option<Vec<Ty>>),
    Const(ConstId),
    Static(StaticId),
    PrimConst(ConstValue, Ty),
    PrimFn(PrimOp, Ty),
    Builtin(Builtin),
    Trait(TraitId),
}

pub(crate) enum PathLookup {
    /// The expression is not a path: evaluate it as a value.
    NotPath,
    Res(PathRes),
    /// Resolution failed and an error was reported.
    Err,
}

impl<'a> FnCtxt<'a> {
    fn item_to_res(&mut self, item: ItemRef) -> PathRes {
        match item {
            ItemRef::Module(m) => PathRes::Module(m),
            ItemRef::Adt(a) => PathRes::Adt(a, None),
            ItemRef::Fn(f) => PathRes::Fn(f, None),
            ItemRef::Const(c) => PathRes::Const(c),
            ItemRef::Variant(a, v) => PathRes::Variant(a, v, None),
            ItemRef::Builtin(b) => PathRes::Builtin(b),
            ItemRef::Static(st) => PathRes::Static(st),
            ItemRef::Trait(t) => PathRes::Trait(t),
            ItemRef::Alias(al) => {
                let t = self.p.aliases[al.idx()].ty;
                match self.tcx().as_adt(t) {
                    Some((id, args)) => PathRes::Adt(id, Some(args)),
                    None => PathRes::Prim(t),
                }
            }
        }
    }

    fn not_found_value(&self, name: &str, span: Span) {
        let mut d = Diagnostic::error("E0202", format!("cannot find `{}` in this scope", name)).primary(span, "not found");
        let mut cands: Vec<String> = Vec::new();
        for s in &self.scopes {
            for (n, _) in s {
                cands.push(n.clone());
            }
        }
        let m = &self.p.modules[self.module.idx()];
        cands.extend(m.items.keys().cloned());
        cands.extend(m.imports.keys().cloned());
        cands.extend(self.p.prelude.keys().cloned());
        if let Some(s) = best_match(name, cands.iter().map(|s| s.as_str())) {
            d = d.help(format!("did you mean `{}`?", s));
        } else if matches!(name, "null" | "nil" | "None_" | "NULL") {
            d = d.help("NEXUL has no null value; use `Option[T]` and `None`");
        } else if self.p.modules.iter().any(|m| m.path == format!("std.{}", name)) || crate::load::public_std_modules().iter().any(|p| *p == format!("std.{}", name)) {
            d = d.help(format!("add `import std.{}` at the top of the file", name));
        }
        self.err(d);
    }

    /// Resolves a path-like expression (`a`, `a.b.c`, `Type[args]`, `Self`).
    pub(crate) fn resolve_path(&mut self, e: &ast::Expr) -> PathLookup {
        match &e.kind {
            A::Ident(name) => {
                if let Some(l) = self.lookup_local(name) {
                    return PathLookup::Res(PathRes::Local(l));
                }
                if let Some(i) = self.generics.iter().position(|g| g == name) {
                    // A type parameter as a namespace: `T.default()`.
                    return PathLookup::Res(PathRes::Prim(self.tcx().param(i as u32)));
                }
                if let Some(t) = primitive_type(self.p, name) {
                    return PathLookup::Res(PathRes::Prim(t));
                }
                if name == "Ptr" {
                    let v = self.new_var(e.span);
                    return PathLookup::Res(PathRes::Prim(self.tcx().ptr(v)));
                }
                match lookup_item(self.p, self.module, name) {
                    Some(item) => PathLookup::Res(self.item_to_res(item)),
                    None => {
                        self.not_found_value(name, e.span);
                        PathLookup::Err
                    }
                }
            }
            A::SelfValue => match self.lookup_local("self") {
                Some(l) => PathLookup::Res(PathRes::Local(l)),
                None => {
                    self.err(
                        Diagnostic::error("E0203", "`self` is only available in methods")
                            .primary(e.span, "no `self` here")
                            .help("declare the function with a `self` parameter inside `impl`"),
                    );
                    PathLookup::Err
                }
            },
            A::SelfType => match self.self_ty {
                Some(t) => match self.tcx().as_adt(t) {
                    Some((id, args)) => PathLookup::Res(PathRes::Adt(id, Some(args))),
                    None => PathLookup::Res(PathRes::Prim(t)),
                },
                None => {
                    self.err(Diagnostic::error("E0202", "`Self` is only available inside an `impl` block").primary(e.span, "no `Self` here"));
                    PathLookup::Err
                }
            },
            A::Field(base, name) => {
                let b = self.resolve_path(base);
                match b {
                    PathLookup::NotPath | PathLookup::Res(PathRes::Local(_)) => PathLookup::NotPath,
                    PathLookup::Err => PathLookup::Err,
                    PathLookup::Res(PathRes::Module(m)) => match lookup_in_module(self.p, self.module, m, &name.name, name.span, self.diags) {
                        Some(item) => PathLookup::Res(self.item_to_res(item)),
                        None => PathLookup::Err,
                    },
                    PathLookup::Res(PathRes::Adt(id, args)) => self.resolve_type_member(id, args, name),
                    PathLookup::Res(PathRes::Trait(t)) => match self.p.traits[t.idx()].method(&name.name) {
                        // `Default.default()`: `Self` is inferred from the context.
                        Some(m) => {
                            let v = self.new_var(name.span);
                            PathLookup::Res(PathRes::AssocFn { f: m.decl, owner_args: vec![v], explicit: None })
                        }
                        None => {
                            self.err(Diagnostic::error("E0202", format!("trait `{}` has no function `{}`", self.p.traits[t.idx()].name, name.name)).primary(name.span, "not found"));
                            PathLookup::Err
                        }
                    },
                    PathLookup::Res(PathRes::Prim(t)) => {
                        if let Some(v) = prim_assoc_const(self.p, t, &name.name) {
                            return PathLookup::Res(PathRes::PrimConst(v, t));
                        }
                        if let TyKind::Ptr(_) = &*self.tcx().kind(t) {
                            if name.name == "null" {
                                return PathLookup::Res(PathRes::PrimFn(PrimOp::PtrNull, t));
                            }
                            if name.name == "from_addr" {
                                return PathLookup::Res(PathRes::PrimFn(PrimOp::PtrFromAddr, t));
                            }
                        }
                        let cands = self.trait_methods_for(t, &name.name);
                        if let Some(f) = self.pick_trait_method(&cands, &name.name, name.span) {
                            return PathLookup::Res(PathRes::AssocFn { f, owner_args: vec![t], explicit: None });
                        }
                        if matches!(&*self.tcx().kind(t), TyKind::Param(_)) {
                            self.err(
                                Diagnostic::error("E0202", format!("`{}` has no associated function `{}`", self.ty_str(t), name.name))
                                    .primary(name.span, "unknown function")
                                    .help(format!("add a bound declaring it, for example `[{}: Default]`", self.ty_str(t))),
                            );
                            return PathLookup::Err;
                        }
                        self.err(
                            Diagnostic::error("E0202", format!("`{}` has no associated item `{}`", self.ty_str(t), name.name))
                                .primary(name.span, "unknown item"),
                        );
                        PathLookup::Err
                    }
                    // `CONST.field`, `f.x`: the base is a value.
                    PathLookup::Res(_) => PathLookup::NotPath,
                }
            }
            A::Index(base, args) => {
                // `Ptr[T]` as a namespace (`Ptr[u8].null()`).
                if let A::Ident(n) = &base.kind {
                    if n == "Ptr" && self.lookup_local("Ptr").is_none() {
                        let tys = self.type_args(args);
                        if tys.len() != 1 {
                            self.err(Diagnostic::error("E0315", "`Ptr` takes exactly one generic argument").primary(e.span, "write `Ptr[T]`"));
                            return PathLookup::Err;
                        }
                        return PathLookup::Res(PathRes::Prim(self.tcx().ptr(tys[0])));
                    }
                }
                let b = self.resolve_path(base);
                match b {
                    PathLookup::NotPath | PathLookup::Res(PathRes::Local(_)) => PathLookup::NotPath,
                    PathLookup::Err => PathLookup::Err,
                    PathLookup::Res(PathRes::Adt(id, None)) => {
                        let tys = self.type_args(args);
                        let n = self.p.adt(id).generics.len();
                        if tys.len() != n {
                            self.err(
                                Diagnostic::error("E0315", format!("`{}` takes {} generic argument(s)", self.p.adt(id).name, n))
                                    .primary(e.span, format!("{} given", tys.len())),
                            );
                            return PathLookup::Err;
                        }
                        PathLookup::Res(PathRes::Adt(id, Some(tys)))
                    }
                    PathLookup::Res(PathRes::Fn(f, None)) => {
                        let tys = self.type_args(args);
                        PathLookup::Res(PathRes::Fn(f, Some(tys)))
                    }
                    PathLookup::Res(PathRes::AssocFn { f, owner_args, explicit: None }) => {
                        let tys = self.type_args(args);
                        PathLookup::Res(PathRes::AssocFn { f, owner_args, explicit: Some(tys) })
                    }
                    PathLookup::Res(PathRes::Prim(t)) if matches!(&*self.tcx().kind(t), TyKind::Ptr(_)) => {
                        let tys = self.type_args(args);
                        if tys.len() != 1 {
                            self.err(Diagnostic::error("E0315", "`Ptr` takes exactly one generic argument").primary(e.span, "write `Ptr[T]`"));
                            return PathLookup::Err;
                        }
                        PathLookup::Res(PathRes::Prim(self.tcx().ptr(tys[0])))
                    }
                    PathLookup::Res(PathRes::Adt(..)) | PathLookup::Res(PathRes::Fn(..)) | PathLookup::Res(PathRes::AssocFn { .. }) => {
                        self.err(Diagnostic::error("E0315", "generic arguments given twice").primary(e.span, "remove one list of `[...]`"));
                        PathLookup::Err
                    }
                    PathLookup::Res(_) => PathLookup::NotPath,
                }
            }
            _ => PathLookup::NotPath,
        }
    }

    fn resolve_type_member(&mut self, id: AdtId, args: Option<Vec<Ty>>, name: &ast::Ident) -> PathLookup {
        let adt = self.p.adt(id);
        if let Some(v) = adt.variant_index(&name.name) {
            return PathLookup::Res(PathRes::Variant(id, v, args));
        }
        if let Some(&f) = adt.methods.get(&name.name) {
            let fd = self.p.func(f);
            if !fd.is_pub && fd.module != self.module {
                self.err(
                    Diagnostic::error("E0227", format!("`{}.{}` is private", adt.name, name.name))
                        .primary(name.span, "private function")
                        .secondary(fd.span, "declared here without `pub`"),
                );
            }
            let owner_args = match args {
                Some(a) => a,
                None => self.fresh_args(adt.generics.len(), name.span),
            };
            return PathLookup::Res(PathRes::AssocFn { f, owner_args, explicit: None });
        }
        let self_args = match &args {
            Some(a) => a.clone(),
            None => self.fresh_args(adt.generics.len(), name.span),
        };
        let self_ty = self.tcx().adt(id, self_args);
        let cands = self.trait_methods_for(self_ty, &name.name);
        if let Some(f) = self.pick_trait_method(&cands, &name.name, name.span) {
            return PathLookup::Res(PathRes::AssocFn { f, owner_args: vec![self_ty], explicit: None });
        }
        let mut d = Diagnostic::error("E0202", format!("`{}` has no variant or associated function `{}`", adt.name, name.name)).primary(name.span, "not found");
        let mut cands: Vec<&str> = adt.variants().iter().map(|v| v.name.as_str()).collect();
        cands.extend(adt.methods.keys().map(|s| s.as_str()));
        if let Some(s) = best_match(&name.name, cands) {
            d = d.help(format!("did you mean `{}`?", s));
        }
        self.err(d);
        PathLookup::Err
    }

    /// Converts generic arguments written in expression syntax into types.
    pub(crate) fn type_args(&mut self, args: &[ast::Expr]) -> Vec<Ty> {
        args.iter()
            .map(|a| match expr_to_type_expr(a) {
                Some(t) => self.resolve_type(&t),
                None => {
                    self.err(Diagnostic::error("E0106", "expected a type").primary(a.span, "not a type"));
                    self.tcx().error
                }
            })
            .collect()
    }

    /// A variant without values used as a value: `Couleur.Rouge`, or `.Rouge` in edition 1.
    pub(crate) fn fieldless_variant(&mut self, id: AdtId, v: u32, args: Option<Vec<Ty>>, expect: Expect, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let adt = self.p.adt(id);
        let variant = &adt.variants()[v as usize];
        if !variant.fields.is_empty() {
            self.err(
                Diagnostic::error("E0306", format!("variant `{}.{}` needs {} value(s)", adt.name, variant.name, variant.fields.len()))
                    .primary(span, "missing values")
                    .help(format!("write `{}({})`", variant.name, vec!["..."; variant.fields.len()].join(", "))),
            );
            return self.error_expr(span);
        }
        let targs = match args {
            Some(a) => a,
            None => self.fresh_args(adt.generics.len(), span),
        };
        let ty = tcx.adt(id, targs.clone());
        if let Some(t) = expect.ty() {
            self.unify(ty, t);
        }
        self.mk(T::Adt { adt: id, variant: Some(v), type_args: targs, fields: vec![] }, ty, span)
    }

    /// Edition 1, `.Name`: the variant `Name` of the type the context expects.
    pub(crate) fn dot_variant(&mut self, name: &ast::Ident, expect: Expect) -> Option<(AdtId, u32)> {
        let tcx = self.tcx();
        let expected = expect.ty().map(|t| self.shallow(t));
        // An error already reported: nothing more to say.
        if expected.is_some_and(|t| tcx.is_error(t)) {
            return None;
        }
        let adt = expected.and_then(|t| tcx.as_adt(t)).map(|(id, _)| id);
        if let Some(id) = adt.filter(|&id| matches!(self.p.adt(id).kind, crate::defs::AdtKind::Struct { .. })) {
            let a = self.p.adt(id);
            self.err(
                Diagnostic::error("E0348", format!("`{}` is not an enumeration: `.{}` names a variant", a.name, name.name))
                    .primary(name.span, format!("a value of type `{}` is expected here", a.name)),
            );
            return None;
        }
        match adt {
            Some(id) => match self.p.adt(id).variant_index(&name.name) {
                Some(v) => Some((id, v)),
                None => {
                    let a = self.p.adt(id);
                    let names: Vec<String> = a.variants().iter().map(|v| format!(".{}", v.name)).collect();
                    self.err(
                        Diagnostic::error("E0348", format!("`{}` has no variant `{}`", a.name, name.name))
                            .primary(name.span, "unknown variant")
                            .note(format!("the variants of `{}` are {}", a.name, names.join(", "))),
                    );
                    None
                }
            },
            None => {
                self.err(
                    Diagnostic::error("E0348", format!("cannot tell which type `.{}` belongs to", name.name))
                        .primary(name.span, "the expected type is not known here")
                        .help(format!("name the type: `Type.{}`, or give the type of the variable", name.name)),
                );
                None
            }
        }
    }

    /// A path used as a value: local, constant, fieldless variant, function...
    pub(crate) fn check_path_expr(&mut self, e: &ast::Expr, expect: Expect) -> thir::Expr {
        let tcx = self.tcx();
        let span = e.span;
        match self.resolve_path(e) {
            PathLookup::Err => self.error_expr(span),
            PathLookup::NotPath => {
                // Only `base.field` reaches here.
                let A::Field(base, name) = &e.kind else { return self.error_expr(span) };
                let b = self.check_expr(base, Expect::None);
                self.check_field_access(b, name, span)
            }
            PathLookup::Res(res) => match res {
                PathRes::Local(l) => {
                    self.mark_read(l);
                    let ty = self.locals[l as usize].ty;
                    self.mk(T::Local(l), ty, span)
                }
                PathRes::Const(c) => {
                    let ty = self.p.consts[c.idx()].ty;
                    self.mk(T::Const(c), ty, span)
                }
                PathRes::Static(st) => {
                    let sd = &self.p.statics[st.idx()];
                    if sd.mutable {
                        self.require_unsafe(span, &format!("using the mutable static `{}`", sd.name));
                    }
                    self.mk(T::Static(st), sd.ty, span)
                }
                PathRes::PrimConst(v, t) => self.mk(T::Lit(v), t, span),
                PathRes::Variant(id, v, args) => self.fieldless_variant(id, v, args, expect, span),
                PathRes::Fn(f, explicit) => {
                    let fd = self.p.func(f);
                    if !fd.generics.is_empty() || explicit.is_some() {
                        self.err(
                            Diagnostic::error("E0316", format!("generic function `{}` cannot be used as a value", fd.name))
                                .primary(span, "call it instead")
                                .note("function values of generic functions are not supported in NEXUL 0.1"),
                        );
                        return self.error_expr(span);
                    }
                    if fd.is_extern() || fd.intrinsic().is_some() {
                        self.err(Diagnostic::error("E0316", format!("`{}` cannot be used as a value", fd.name)).primary(span, "call it instead"));
                        return self.error_expr(span);
                    }
                    let params = fd.params.iter().map(|p| (p.conv, p.ty)).collect();
                    let ty = tcx.mk(TyKind::Fn(params, fd.ret));
                    self.mk(T::FnRef(f), ty, span)
                }
                PathRes::AssocFn { f, .. } => {
                    let fd = self.p.func(f);
                    self.err(
                        Diagnostic::error("E0316", format!("`{}` must be called", fd.qualified_name(self.p)))
                            .primary(span, "add `(...)`")
                            .note("methods and associated functions cannot be used as values in NEXUL 0.1"),
                    );
                    self.error_expr(span)
                }
                PathRes::Builtin(b) => {
                    self.err(Diagnostic::error("E0316", format!("`{}` must be called", b.name())).primary(span, "add `(...)`"));
                    self.error_expr(span)
                }
                PathRes::Adt(id, _) => {
                    let adt = self.p.adt(id);
                    let mut d = Diagnostic::error("E0314", format!("`{}` is a type, not a value", adt.name)).primary(span, "not a value");
                    if !adt.is_enum() {
                        let fields: Vec<String> = adt.fields().iter().map(|f| format!("{}: ...", f.name)).collect();
                        d = d.help(format!("to build a value, write `{}({})`", adt.name, fields.join(", ")));
                    }
                    self.err(d);
                    self.error_expr(span)
                }
                PathRes::Prim(t) => {
                    self.err(Diagnostic::error("E0314", format!("`{}` is a type, not a value", self.ty_str(t))).primary(span, "not a value"));
                    self.error_expr(span)
                }
                PathRes::PrimFn(..) => {
                    self.err(Diagnostic::error("E0316", "this function must be called").primary(span, "add `()`"));
                    self.error_expr(span)
                }
                PathRes::Module(m) => {
                    self.err(Diagnostic::error("E0314", format!("module `{}` is not a value", self.p.modules[m.idx()].path)).primary(span, "not a value"));
                    self.error_expr(span)
                }
                PathRes::Trait(t) => {
                    self.err(
                        Diagnostic::error("E0314", format!("trait `{}` is not a value", self.p.traits[t.idx()].name))
                            .primary(span, "not a value")
                            .help("call the method on a value: `value.method(...)`"),
                    );
                    self.error_expr(span)
                }
            },
        }
    }

    // -----------------------------------------------------------------------
    // Calls

    fn check_args_for_errors(&mut self, args: &[ast::Arg]) {
        for a in args {
            self.check_expr(&a.expr, Expect::None);
        }
    }

    pub(crate) fn check_call(&mut self, callee: &ast::Expr, args: &[ast::Arg], expect: Expect, span: Span) -> thir::Expr {
        // Edition 1: `.Name(values)`, a variant of the expected type.
        if let A::Variant(name) = &callee.kind {
            return match self.dot_variant(name, expect) {
                Some((id, v)) => self.check_variant_ctor(id, v, None, args, expect, span),
                None => {
                    self.check_args_for_errors(args);
                    self.error_expr(span)
                }
            };
        }
        // Method calls: `value.name(...)` and `value.name[T](...)`.
        let (method_base, method_name, method_targs) = match &callee.kind {
            A::Field(base, name) => (Some(&**base), Some(name), None),
            A::Index(inner, targs) => match &inner.kind {
                A::Field(base, name) => (Some(&**base), Some(name), Some(targs)),
                _ => (None, None, None),
            },
            _ => (None, None, None),
        };
        match self.resolve_path(callee) {
            PathLookup::Err => {
                self.check_args_for_errors(args);
                self.error_expr(span)
            }
            PathLookup::Res(PathRes::Local(_)) | PathLookup::NotPath => {
                if let (Some(base), Some(name)) = (method_base, method_name) {
                    // `base` is a value (or the path failed above).
                    if matches!(
                        self.resolve_path(base),
                        PathLookup::NotPath | PathLookup::Res(PathRes::Local(_)) | PathLookup::Res(PathRes::Static(_)) | PathLookup::Res(PathRes::Const(_))
                    ) {
                        return self.check_method_call(base, name, method_targs.map(|v| v.as_slice()), args, expect, span);
                    }
                }
                let f = self.check_expr(callee, Expect::None);
                self.check_indirect_call(f, args, span)
            }
            PathLookup::Res(res) => match res {
                PathRes::Fn(f, explicit) => self.check_fn_call(f, Vec::new(), explicit, None, args, expect, span),
                PathRes::AssocFn { f, owner_args, explicit } => {
                    let fd = self.p.func(f);
                    if fd.has_self {
                        let tname = fd.owner.map(|o| self.p.adt(o).name.clone()).unwrap_or_default();
                        self.err(
                            Diagnostic::error("E0307", format!("`{}` is a method: call it on a value", fd.name))
                                .primary(span, format!("not an associated function of `{}`", tname))
                                .help(format!("write `value.{}(...)`", fd.name)),
                        );
                        self.check_args_for_errors(args);
                        return self.error_expr(span);
                    }
                    self.check_fn_call(f, owner_args, explicit, None, args, expect, span)
                }
                PathRes::Adt(id, explicit) => self.check_struct_ctor(id, explicit, args, expect, span),
                PathRes::Variant(id, v, explicit) => self.check_variant_ctor(id, v, explicit, args, expect, span),
                PathRes::Builtin(b) => self.check_builtin(b, args, span),
                PathRes::PrimFn(op, t) => {
                    if let Some(et) = expect.ty() {
                        self.unify(t, et);
                    }
                    if op == PrimOp::PtrFromAddr {
                        // `Ptr[T].from_addr(address)`: a pointer to a fixed address (memory-mapped
                        // registers, spec §5.2).
                        self.require_unsafe(span, "`Ptr.from_addr`");
                        if args.len() != 1 {
                            self.err(Diagnostic::error("E0304", "`from_addr` takes one argument (the address)").primary(span, "wrong number of arguments"));
                            self.check_args_for_errors(args);
                            return self.error_expr(span);
                        }
                        let usize_ty = self.tcx().usize;
                        let a = self.check_arg(&args[0], Convention::Borrow, usize_ty, Some("from_addr"));
                        return self.mk(T::Call { callee: Callee::Prim(op), args: vec![a] }, t, span);
                    }
                    if !args.is_empty() {
                        self.err(Diagnostic::error("E0304", "this function takes no arguments").primary(span, "remove the arguments"));
                    }
                    self.mk(T::Call { callee: Callee::Prim(op), args: vec![] }, t, span)
                }
                PathRes::Const(_) | PathRes::PrimConst(..) | PathRes::Static(_) => {
                    let f = self.check_expr(callee, Expect::None);
                    self.check_indirect_call(f, args, span)
                }
                PathRes::Module(_) | PathRes::Prim(_) | PathRes::Trait(_) => {
                    self.err(Diagnostic::error("E0319", "this is not a function").primary(callee.span, "cannot be called"));
                    self.check_args_for_errors(args);
                    self.error_expr(span)
                }
                PathRes::Local(_) => unreachable!(),
            },
        }
    }

    fn check_indirect_call(&mut self, f: thir::Expr, args: &[ast::Arg], span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let ft = self.shallow(f.ty);
        match &*tcx.kind(ft) {
            TyKind::Fn(params, ret) | TyKind::Closure(params, ret) => {
                let params: Vec<(Convention, Ty)> = params.clone();
                let ret = *ret;
                if params.len() != args.len() {
                    self.err(
                        Diagnostic::error("E0304", format!("this function takes {} argument(s) but {} were given", params.len(), args.len()))
                            .primary(span, "wrong number of arguments"),
                    );
                }
                let mut out = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    let (conv, pty) = params.get(i).copied().unwrap_or((Convention::Borrow, tcx.error));
                    out.push(self.check_arg(a, conv, pty, None));
                }
                self.mk(T::Call { callee: Callee::Indirect(Box::new(f)), args: out }, ret, span)
            }
            TyKind::Error => {
                self.check_args_for_errors(args);
                self.error_expr(span)
            }
            _ => {
                self.err(
                    Diagnostic::error("E0319", format!("a value of type `{}` cannot be called", self.ty_str(ft)))
                        .primary(f.span, "not a function"),
                );
                self.check_args_for_errors(args);
                self.error_expr(span)
            }
        }
    }

    /// Checks one argument against a parameter.
    fn check_arg(&mut self, a: &ast::Arg, conv: Convention, pty: Ty, fn_name: Option<&str>) -> CallArg {
        if let Some(l) = &a.label {
            self.err(
                Diagnostic::error("E0305", "named arguments are only used to build structures")
                    .primary(l.span, "unexpected label")
                    .help(format!("remove `{}:`", l.name)),
            );
        }
        match (conv, a.inout) {
            (Convention::Inout, None) => {
                let mut d = Diagnostic::error("E0411", "an `inout` argument must be marked with `&`").primary(a.expr.span, "add `&` before this argument");
                if let Some(n) = fn_name {
                    d = d.note(format!("`{}` may modify this argument", n));
                }
                self.err(d);
            }
            (c, Some(amp)) if c != Convention::Inout => {
                self.err(
                    Diagnostic::error("E0412", "`&` is only used for `inout` arguments")
                        .primary(amp, "remove this `&`")
                        .note(format!("this parameter is not `inout`: it {}", if c == Convention::Owned { "takes ownership of the value" } else { "only reads the value" })),
                );
            }
            _ => {}
        }
        let mut x = match &a.expr.kind {
            A::Closure(c) => self.check_closure(c, pty, a.expr.span),
            _ => self.check_expr(&a.expr, Expect::Has(pty)),
        };
        self.coerce(&mut x, pty);
        if conv == Convention::Inout {
            self.mark_mutated(&x);
        }
        CallArg { conv, expr: x }
    }

    /// Calls a free function or an associated function / method.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn check_fn_call(
        &mut self,
        f: FnId,
        owner_args: Vec<Ty>,
        explicit: Option<Vec<Ty>>,
        receiver: Option<thir::Expr>,
        args: &[ast::Arg],
        expect: Expect,
        span: Span,
    ) -> thir::Expr {
        let tcx = self.tcx();
        let fd = self.p.func(f);
        let n_owner = fd.n_owner_generics;
        let n_own = fd.generics.len() - n_owner;
        let mut targs = if owner_args.len() == n_owner { owner_args } else { self.fresh_args(n_owner, span) };
        match explicit {
            Some(ex) => {
                if ex.len() != n_own {
                    self.err(
                        Diagnostic::error("E0315", format!("`{}` takes {} generic argument(s)", fd.name, n_own))
                            .primary(span, format!("{} given", ex.len())),
                    );
                    let fresh = self.fresh_args(n_own, span);
                    targs.extend(fresh);
                } else {
                    targs.extend(ex);
                }
            }
            None => {
                let fresh = self.fresh_args(n_own, span);
                targs.extend(fresh);
            }
        }
        let ret = tcx.subst(fd.ret, &targs);
        if let Some(t) = expect.ty() {
            // Early hint: helps type literals in the arguments (`Some(1)` as `Option[u8]`).
            self.unify(ret, t);
        }
        for b in &fd.bounds {
            let ty = targs[b.param as usize];
            let what = format!("by `{}`", fd.qualified_name(self.p));
            self.deferred.push(super::Deferred::Bound { ty, trait_id: b.trait_id, span, what });
        }
        for &t in &targs {
            self.deferred.push(super::Deferred::NoClosure { ty: t, span });
        }
        if fd.is_unsafe || fd.is_extern() {
            let what = if fd.is_extern() { format!("calling the external function `{}`", fd.name) } else { format!("calling the unsafe function `{}`", fd.name) };
            self.require_unsafe(span, &what);
        }
        // A private free function of another module is reported where its name is looked
        // up (E0222: `module.f` or an `import`); reporting E0227 here as well gave two
        // errors for one mistake (spec §7.4). Methods and associated functions, which the
        // lookup in a module does not see, are checked where they are resolved (E0227).
        let mut out = Vec::new();
        let params: Vec<(Convention, Ty, String)> = fd.params.iter().map(|p| (p.conv, tcx.subst(p.ty, &targs), p.name.clone())).collect();
        let mut param_iter = params.iter();
        if let Some(recv) = receiver {
            let (conv, pty, _) = param_iter.next().unwrap().clone();
            if !self.unify(recv.ty, pty) && !tcx.is_error(recv.ty) {
                self.mismatch(recv.span, pty, recv.ty);
            }
            if conv == Convention::Inout {
                self.mark_mutated(&recv);
            }
            out.push(CallArg { conv, expr: recv });
        }
        let rest: Vec<&(Convention, Ty, String)> = param_iter.collect();
        if rest.len() != args.len() {
            let sig: Vec<String> = rest.iter().map(|(c, t, n)| {
                if *c == Convention::Borrow { format!("{}: {}", n, self.ty_str(*t)) } else { format!("{}: {} {}", n, c.keyword(), self.ty_str(*t)) }
            }).collect();
            self.err(
                Diagnostic::error("E0304", format!("`{}` takes {} argument{} but {} {} given", fd.name, rest.len(), if rest.len() == 1 { "" } else { "s" }, args.len(), if args.len() == 1 { "was" } else { "were" }))
                    .primary(span, "wrong number of arguments")
                    .note(format!("signature: {}({})", fd.qualified_name(self.p), sig.join(", "))),
            );
        }
        let name = fd.name.clone();
        // The anonymous function given to `tache.portee` launches the tasks (ADR 0013).
        self.next_closure_scope = self.p.lang_fn("task_scope") == Some(f);
        for (i, a) in args.iter().enumerate() {
            match rest.get(i) {
                Some((conv, pty, _)) => out.push(self.check_arg(a, *conv, *pty, Some(&name))),
                None => {
                    self.check_expr(&a.expr, Expect::None);
                }
            }
        }
        self.next_closure_scope = false;
        if self.p.lang_fn("task_launch") == Some(f) {
            self.check_task_launch(&out, span);
        }
        self.mk(T::Call { callee: Callee::Fn(f, targs), args: out }, ret, span)
    }

    /// `p.lancer(fn() { … })` (ADR 0013): directly in the anonymous function given to
    /// `tache.portee`, on its parameter, with an anonymous function written in the call. The
    /// task uses only variables declared before `tache.portee` (the others are destroyed
    /// before it ends); launched in a loop, it modifies none (the task of the next turn would
    /// modify it at the same time); it reads `Share` values and modifies `Transfer` ones.
    /// `places.rs` then keeps what it uses frozen until the end of the `portee`.
    fn check_task_launch(&mut self, args: &[CallArg], span: Span) {
        let scope = self.closure_frames.last().and_then(|f| f.scope_param.map(|p| (p, f.first_local)));
        let receiver_ok = matches!((scope, args.first().map(|a| &a.expr.kind)), (Some((p, _)), Some(T::Local(l))) if *l == p);
        if !receiver_ok {
            self.err(
                Diagnostic::error("E0254", "a task is launched directly in the function given to `tache.portee`, on its parameter")
                    .primary(span, "not launched there")
                    .note("`tache.portee` waits for its tasks: a task launched elsewhere could outlive what it uses")
                    .help("write `tache.portee(fn(inout p: tache.Portee) { p.lancer(fn() { … }) })`"),
            );
            return;
        }
        let first_local = scope.unwrap().1;
        let Some(task) = args.get(1) else { return };
        let T::Closure { captures, .. } = &task.expr.kind else {
            if !matches!(task.expr.kind, T::Error) {
                self.err(
                    Diagnostic::error("E0254", "a task is an anonymous function written in the call")
                        .primary(task.expr.span, "not an anonymous function")
                        .help("write `p.lancer(fn() { work() })`"),
                );
            }
            return;
        };
        let in_loop = !self.loops.is_empty();
        for &(l, mutated) in captures.clone().iter() {
            let (name, ty, decl_span) = {
                let d = &self.locals[l as usize];
                (d.name.clone(), d.ty, d.span)
            };
            if l >= first_local {
                self.err(
                    Diagnostic::error("E0254", format!("a task cannot use `{}`, a variable of the function given to `tache.portee`", name))
                        .primary(task.expr.span, format!("uses `{}`", name))
                        .secondary(decl_span, "destroyed when that function returns, before the task ends")
                        .help(format!("declare `{}` before `tache.portee`", name)),
                );
                continue;
            }
            if mutated && in_loop {
                self.err(
                    Diagnostic::error("E0254", format!("a task launched in a loop cannot modify `{}`", name))
                        .primary(task.expr.span, format!("modifies `{}`", name))
                        .note("the task of the next turn would modify it at the same time"),
                );
            }
            let (property, what) = if mutated { ("transfer", format!("by a task that modifies `{}`", name)) } else { ("share", format!("by a task that reads `{}`", name)) };
            if let Some(tr) = self.p.lang_trait(property) {
                self.deferred.push(super::Deferred::Bound { ty, trait_id: tr, span: task.expr.span, what });
            }
        }
    }

    fn check_method_call(&mut self, base: &ast::Expr, name: &ast::Ident, targs: Option<&[ast::Expr]>, args: &[ast::Arg], expect: Expect, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let mut recv = self.check_expr(base, Expect::None);
        // A constrained type that is not distinct has the methods of its base (ADR 0014).
        if let Some((id, _, false)) = self.newtype_of(recv.ty) {
            if !self.p.adt(id).methods.contains_key(&name.name) && self.trait_methods_for(recv.ty, &name.name).is_empty() {
                recv = self.widen_constrained(recv);
            }
        }
        let rt = self.shallow(recv.ty);
        let explicit = targs.map(|t| self.type_args(t));
        match &*tcx.kind(rt) {
            TyKind::Adt(id, owner_args) => {
                let adt = self.p.adt(*id);
                match adt.methods.get(&name.name) {
                    Some(&f) => {
                        let fd = self.p.func(f);
                        if !fd.has_self {
                            self.err(
                                Diagnostic::error("E0307", format!("`{}` is an associated function, not a method", name.name))
                                    .primary(name.span, "no `self` parameter")
                                    .help(format!("call it as `{}.{}(...)`", adt.name, name.name)),
                            );
                            self.check_args_for_errors(args);
                            return self.error_expr(span);
                        }
                        if !fd.is_pub && fd.module != self.module {
                            self.err(
                                Diagnostic::error("E0227", format!("method `{}` of `{}` is private", name.name, adt.name))
                                    .primary(name.span, "private method")
                                    .secondary(fd.span, "declared here without `pub`"),
                            );
                        }
                        if fd.name == "drop" && Some(f) == adt.drop_fn {
                            self.err(
                                Diagnostic::error("E0430", "a destructor cannot be called explicitly")
                                    .primary(name.span, "called automatically when the value is destroyed")
                                    .help("use `std.mem.drop(value)` to destroy a value early"),
                            );
                        }
                        let owner_args = owner_args.clone();
                        self.check_fn_call(f, owner_args, explicit, Some(recv), args, expect, span)
                    }
                    None if !self.trait_methods_for(rt, &name.name).is_empty() => {
                        let cands = self.trait_methods_for(rt, &name.name);
                        let f = self.pick_trait_method(&cands, &name.name, name.span).unwrap();
                        self.check_trait_method_call(f, rt, recv, name, explicit, args, expect, span)
                    }
                    None => {
                        if let Some(field) = adt.fields().iter().find(|f| f.name == name.name) {
                            if matches!(&*tcx.kind(field.ty), TyKind::Fn(..)) {
                                self.err(
                                    Diagnostic::error("E0308", format!("`{}` is a field, not a method", name.name))
                                        .primary(name.span, "field of function type")
                                        .help(format!("to call the function stored in the field, write `(value.{})(...)`", name.name)),
                                );
                                self.check_args_for_errors(args);
                                return self.error_expr(span);
                            }
                        }
                        let mut d = Diagnostic::error("E0308", format!("no method named `{}` on type `{}`", name.name, self.ty_str(rt))).primary(name.span, "method not found");
                        let cands: Vec<&str> = adt.methods.keys().map(|s| s.as_str()).collect();
                        if let Some(s) = best_match(&name.name, cands) {
                            d = d.help(format!("did you mean `{}`?", s));
                        }
                        self.err(d);
                        self.check_args_for_errors(args);
                        self.error_expr(span)
                    }
                }
            }
            TyKind::Error => {
                self.check_args_for_errors(args);
                self.error_expr(span)
            }
            TyKind::Infer(_) => {
                self.need_type(recv.span);
                self.check_args_for_errors(args);
                self.error_expr(span)
            }
            TyKind::Ptr(elem) => {
                let elem = *elem;
                self.check_ptr_method(recv, elem, name, explicit, args, span)
            }
            TyKind::Int(_) | TyKind::Float(_) | TyKind::Char | TyKind::Bool if !self.prim_has_method(rt, &name.name) && !self.trait_methods_for(rt, &name.name).is_empty() => {
                let cands = self.trait_methods_for(rt, &name.name);
                let f = self.pick_trait_method(&cands, &name.name, name.span).unwrap();
                self.check_trait_method_call(f, rt, recv, name, explicit, args, expect, span)
            }
            TyKind::Int(_) | TyKind::IntVar(_) | TyKind::Float(_) | TyKind::FloatVar(_) | TyKind::Char | TyKind::Bool => {
                self.check_prim_method(recv, name, args, span)
            }
            TyKind::Param(_) | TyKind::Dyn(_) => {
                let cands = self.trait_methods_for(rt, &name.name);
                match self.pick_trait_method(&cands, &name.name, name.span) {
                    Some(f) => {
                        if let TyKind::Dyn(tr) = &*tcx.kind(rt) {
                            if !self.check_dyn_compatible(*tr, recv.span) {
                                self.check_args_for_errors(args);
                                return self.error_expr(span);
                            }
                        }
                        self.check_trait_method_call(f, rt, recv, name, explicit, args, expect, span)
                    }
                    None => {
                        let mut d = Diagnostic::error("E0308", format!("no method named `{}` on type `{}`", name.name, self.ty_str(rt))).primary(name.span, "method not found");
                        if matches!(&*tcx.kind(rt), TyKind::Param(_)) {
                            d = d
                                .note(format!("a generic type only has the methods of the traits it is bound by"))
                                .help(format!("add a bound, for example `[{}: Display]`, or `where {}: ...`", self.ty_str(rt), self.ty_str(rt)));
                        }
                        self.err(d);
                        self.check_args_for_errors(args);
                        self.error_expr(span)
                    }
                }
            }
            _ => {
                self.err(Diagnostic::error("E0308", format!("no method named `{}` on type `{}`", name.name, self.ty_str(rt))).primary(name.span, "method not found"));
                self.check_args_for_errors(args);
                self.error_expr(span)
            }
        }
    }

    /// `value.method(...)` resolved to a trait method declaration (`Self` = `self_ty`).
    #[allow(clippy::too_many_arguments)]
    fn check_trait_method_call(&mut self, f: FnId, self_ty: Ty, recv: thir::Expr, name: &ast::Ident, explicit: Option<Vec<Ty>>, args: &[ast::Arg], expect: Expect, span: Span) -> thir::Expr {
        if !self.p.func(f).has_self {
            self.err(
                Diagnostic::error("E0307", format!("`{}` is an associated function, not a method", name.name))
                    .primary(name.span, "no `self` parameter")
                    .help(format!("call it as `{}.{}(...)`", self.ty_str(self_ty), name.name)),
            );
            self.check_args_for_errors(args);
            return self.error_expr(span);
        }
        self.check_fn_call(f, vec![self_ty], explicit, Some(recv), args, expect, span)
    }

    /// Is `name` a method provided by the compiler for this primitive type?
    fn prim_has_method(&self, t: Ty, name: &str) -> bool {
        let k = self.tcx().kind(t);
        if name == "to_string" {
            return true;
        }
        match &*k {
            TyKind::Int(_) => matches!(name, "abs" | "min" | "max" | "pow" | "leading_zeros" | "trailing_zeros" | "count_ones" | "rotate_left" | "rotate_right" | "swap_bytes"),
            TyKind::Float(_) => matches!(
                name,
                "sqrt" | "floor" | "ceil" | "round" | "trunc" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "exp" | "ln" | "log10" | "log2" | "abs" | "powf" | "atan2" | "min" | "max" | "hypot" | "copysign" | "to_bits" | "is_nan" | "is_infinite" | "is_finite"
            ),
            TyKind::Char => matches!(
                name,
                "is_ascii_digit" | "is_ascii_alphabetic" | "is_ascii_alphanumeric" | "is_whitespace" | "is_ascii_uppercase" | "is_ascii_lowercase" | "is_ascii" | "to_ascii_uppercase" | "to_ascii_lowercase" | "len_utf8"
            ),
            _ => false,
        }
    }

    fn check_prim_method(&mut self, recv: thir::Expr, name: &ast::Ident, args: &[ast::Arg], span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let rt = self.shallow(recv.ty);
        let kind = tcx.kind(rt);
        let is_int = matches!(&*kind, TyKind::Int(_) | TyKind::IntVar(_));
        let is_float = matches!(&*kind, TyKind::Float(_) | TyKind::FloatVar(_));
        let is_char = matches!(&*kind, TyKind::Char);
        let n = name.name.as_str();
        // (operation, parameter types, return type)
        let sig: Option<(PrimOp, Vec<Ty>, Ty)> = if n == "to_string" {
            Some((PrimOp::ToString, vec![], self.p.string_ty()))
        } else if is_int {
            match n {
                "abs" => {
                    self.deferred.push(Deferred::SignedInt { ty: rt, span });
                    Some((PrimOp::IntAbs, vec![], rt))
                }
                "min" => Some((PrimOp::IntMin, vec![rt], rt)),
                "max" => Some((PrimOp::IntMax, vec![rt], rt)),
                "pow" => Some((PrimOp::IntPow, vec![tcx.u32], rt)),
                "leading_zeros" => Some((PrimOp::IntLeadingZeros, vec![], tcx.u32)),
                "trailing_zeros" => Some((PrimOp::IntTrailingZeros, vec![], tcx.u32)),
                "count_ones" => Some((PrimOp::IntCountOnes, vec![], tcx.u32)),
                "rotate_left" => Some((PrimOp::IntRotateLeft, vec![tcx.u32], rt)),
                "rotate_right" => Some((PrimOp::IntRotateRight, vec![tcx.u32], rt)),
                "swap_bytes" => Some((PrimOp::IntSwapBytes, vec![], rt)),
                _ => None,
            }
        } else if is_float {
            match n {
                "sqrt" | "floor" | "ceil" | "round" | "trunc" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "exp" | "ln" | "log10"
                | "log2" | "abs" => Some((PrimOp::FloatUnary(static_name(n)), vec![], rt)),
                "powf" | "atan2" | "min" | "max" | "hypot" | "copysign" => Some((PrimOp::FloatBinary(static_name(n)), vec![rt], rt)),
                "to_bits" => {
                    let bits = match &*kind {
                        TyKind::Float(crate::ty::FloatTy::F32) => tcx.u32,
                        _ => tcx.u64,
                    };
                    Some((PrimOp::FloatToBits, vec![], bits))
                }
                "is_nan" => Some((PrimOp::FloatIsNan, vec![], tcx.bool)),
                "is_infinite" => Some((PrimOp::FloatIsInfinite, vec![], tcx.bool)),
                "is_finite" => Some((PrimOp::FloatIsFinite, vec![], tcx.bool)),
                _ => None,
            }
        } else if is_char {
            match n {
                "is_ascii_digit" | "is_ascii_alphabetic" | "is_ascii_alphanumeric" | "is_whitespace" | "is_ascii_uppercase" | "is_ascii_lowercase"
                | "is_ascii" => Some((PrimOp::CharPredicate(static_name(n)), vec![], tcx.bool)),
                "to_ascii_uppercase" => Some((PrimOp::CharToUpper, vec![], tcx.char)),
                "to_ascii_lowercase" => Some((PrimOp::CharToLower, vec![], tcx.char)),
                "len_utf8" => Some((PrimOp::CharLenUtf8, vec![], tcx.usize)),
                _ => None,
            }
        } else {
            None
        };
        let Some((op, params, ret)) = sig else {
            let mut d = Diagnostic::error("E0308", format!("no method named `{}` on type `{}`", n, self.ty_str(rt))).primary(name.span, "method not found");
            if is_float && n == "pow" {
                d = d.help("for floats, use `x.powf(y)`");
            } else if is_int && matches!(n, "sqrt" | "sin" | "cos" | "floor" | "ceil" | "round") {
                d = d.help(format!("this is a float method: convert first, `(x as f64).{}()`", n));
            }
            self.err(d);
            self.check_args_for_errors(args);
            return self.error_expr(span);
        };
        if params.len() != args.len() {
            self.err(
                Diagnostic::error("E0304", format!("`{}` takes {} argument(s) but {} were given", n, params.len(), args.len()))
                    .primary(span, "wrong number of arguments"),
            );
        }
        let mut out = vec![CallArg { conv: Convention::Borrow, expr: recv }];
        for (i, a) in args.iter().enumerate() {
            let pty = params.get(i).copied().unwrap_or(tcx.error);
            out.push(self.check_arg(a, Convention::Borrow, pty, Some(n)));
        }
        self.mk(T::Call { callee: Callee::Prim(op), args: out }, ret, span)
    }

    fn check_ptr_method(&mut self, recv: thir::Expr, elem: Ty, name: &ast::Ident, explicit: Option<Vec<Ty>>, args: &[ast::Arg], span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let rt = self.shallow(recv.ty);
        let n = name.name.as_str();
        let (op, params, ret, unsafe_op): (PrimOp, Vec<(Convention, Ty)>, Ty, bool) = match n {
            "is_null" => (PrimOp::PtrIsNull, vec![], tcx.bool, false),
            "addr" => (PrimOp::PtrAddr, vec![], tcx.usize, false),
            "offset" => (PrimOp::PtrOffset, vec![(Convention::Borrow, tcx.usize)], rt, true),
            "sub" => (PrimOp::PtrSub, vec![(Convention::Borrow, tcx.usize)], rt, true),
            "read" => (PrimOp::PtrRead, vec![], elem, true),
            "write" => (PrimOp::PtrWrite, vec![(Convention::Owned, elem)], tcx.unit, true),
            "cast" => {
                let target = match &explicit {
                    Some(v) if v.len() == 1 => v[0],
                    _ => {
                        self.err(Diagnostic::error("E0315", "`cast` needs the target type").primary(span, "write `p.cast[U]()`"));
                        tcx.error
                    }
                };
                (PrimOp::PtrCast, vec![], tcx.ptr(target), true)
            }
            _ => {
                self.err(
                    Diagnostic::error("E0308", format!("no method named `{}` on type `{}`", n, self.ty_str(rt)))
                        .primary(name.span, "method not found")
                        .note("pointer methods: is_null, addr, offset, sub, read, write, cast"),
                );
                self.check_args_for_errors(args);
                return self.error_expr(span);
            }
        };
        if unsafe_op {
            self.require_unsafe(span, &format!("`Ptr.{}`", n));
        }
        if params.len() != args.len() {
            self.err(Diagnostic::error("E0304", format!("`{}` takes {} argument(s) but {} were given", n, params.len(), args.len())).primary(span, "wrong number of arguments"));
        }
        let mut out = vec![CallArg { conv: Convention::Borrow, expr: recv }];
        for (i, a) in args.iter().enumerate() {
            let (c, pty) = params.get(i).copied().unwrap_or((Convention::Borrow, tcx.error));
            out.push(self.check_arg(a, c, pty, Some(n)));
        }
        self.mk(T::Call { callee: Callee::Prim(op), args: out }, ret, span)
    }

    fn check_builtin(&mut self, b: Builtin, args: &[ast::Arg], span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let arity_err = |cx: &Self, expected: &str| {
            cx.err(Diagnostic::error("E0304", format!("`{}` takes {}", b.name(), expected)).primary(span, format!("{} argument(s) given", args.len())));
        };
        let mut out = Vec::new();
        let ret = match b {
            Builtin::Print | Builtin::Println | Builtin::Eprint | Builtin::Eprintln => {
                let max = 1;
                let min = if matches!(b, Builtin::Println | Builtin::Eprintln) { 0 } else { 1 };
                if args.len() < min || args.len() > max {
                    arity_err(self, if min == 0 { "zero or one argument" } else { "one argument" });
                }
                for a in args.iter().take(1) {
                    // A fresh variable, not the error type: unified with the error type, an
                    // integer literal or an inferred variable (`println(7)`) silently took
                    // the type `{error}`, which crashed the code generator.
                    let pty = self.new_var(a.expr.span);
                    let arg = self.check_arg(a, Convention::Borrow, pty, None);
                    let arg = self.recheck_value(arg);
                    self.deferred.push(Deferred::Format { ty: arg.expr.ty, spec: None, span: arg.expr.span });
                    out.push(arg);
                }
                tcx.unit
            }
            Builtin::Panic => {
                if args.len() != 1 {
                    arity_err(self, "one argument (the message)");
                }
                let st = self.p.string_ty();
                for a in args.iter().take(1) {
                    out.push(self.check_arg(a, Convention::Borrow, st, None));
                }
                tcx.never
            }
            Builtin::Assert => {
                if args.is_empty() || args.len() > 2 {
                    arity_err(self, "a condition and an optional message");
                }
                if let Some(a) = args.first() {
                    out.push(self.check_arg(a, Convention::Borrow, tcx.bool, None));
                }
                if let Some(a) = args.get(1) {
                    let st = self.p.string_ty();
                    out.push(self.check_arg(a, Convention::Borrow, st, None));
                }
                tcx.unit
            }
            Builtin::Unreachable => {
                if !args.is_empty() {
                    arity_err(self, "no arguments");
                }
                tcx.never
            }
        };
        self.mk(T::Call { callee: Callee::Builtin(b), args: out }, ret, span)
    }

    /// `print(x)` accepts any formattable type: the argument was checked against the
    /// error type (which unifies with anything); give it back its own type.
    fn recheck_value(&mut self, a: CallArg) -> CallArg {
        a
    }

    // -----------------------------------------------------------------------
    // Constructors

    fn check_struct_ctor(&mut self, id: AdtId, explicit: Option<Vec<Ty>>, args: &[ast::Arg], expect: Expect, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let adt = self.p.adt(id);
        if let Some(nt) = &adt.newtype {
            let base = self.p.ty_str(adt.fields()[0].ty);
            let check = if nt.cond.is_some() { format!(", or `{}.check(value)` for an `Option`", adt.name) } else { String::new() };
            self.err(
                Diagnostic::error("E0310", format!("a value of `{}` is made with `as`", adt.name))
                    .primary(span, "not a structure")
                    .help(format!("write `value as {}`, `value` of type `{}`{}", adt.name, base, check)),
            );
            self.check_args_for_errors(args);
            return self.error_expr(span);
        }
        if adt.is_enum() {
            let variants: Vec<&str> = adt.variants().iter().map(|v| v.name.as_str()).collect();
            self.err(
                Diagnostic::error("E0306", format!("`{}` is an enum: build one of its variants", adt.name))
                    .primary(span, "not a structure")
                    .help(format!("for example `{}.{}(...)`", adt.name, variants.first().copied().unwrap_or("Variant"))),
            );
            self.check_args_for_errors(args);
            return self.error_expr(span);
        }
        let targs = match explicit {
            Some(a) => a,
            None => self.fresh_args(adt.generics.len(), span),
        };
        let ty = tcx.adt(id, targs.clone());
        if let Some(t) = expect.ty() {
            self.unify(ty, t);
        }
        let fields = adt.fields();
        let names: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
        let mut out: Vec<Option<thir::Expr>> = vec![None; fields.len()];
        let mut ok = true;
        let mut next_expected = 0usize;
        for a in args {
            if let Some(amp) = a.inout {
                self.err(Diagnostic::error("E0412", "`&` is only used for `inout` arguments").primary(amp, "remove this `&`"));
            }
            let Some(label) = &a.label else {
                if ok {
                    let shape: Vec<String> = names.iter().map(|n| format!("{}: ...", n)).collect();
                    self.err(
                        Diagnostic::error("E0310", "structure fields must be named")
                            .primary(a.expr.span, "missing field name")
                            .help(format!("write `{}({})`", adt.name, shape.join(", "))),
                    );
                }
                ok = false;
                self.check_expr(&a.expr, Expect::None);
                continue;
            };
            let Some(i) = names.iter().position(|n| *n == label.name) else {
                let mut d = Diagnostic::error("E0309", format!("`{}` has no field named `{}`", adt.name, label.name)).primary(label.span, "unknown field");
                if let Some(s) = best_match(&label.name, names.iter().map(|s| s.as_str())) {
                    d = d.help(format!("did you mean `{}`?", s));
                }
                self.err(d);
                ok = false;
                self.check_expr(&a.expr, Expect::None);
                continue;
            };
            let f = &fields[i];
            if !f.is_pub && adt.module != self.module {
                self.err(
                    Diagnostic::error("E0226", format!("field `{}` of `{}` is private", f.name, adt.name))
                        .primary(label.span, "private field")
                        .note("a structure with private fields can only be built in its own module"),
                );
            }
            if out[i].is_some() {
                self.err(Diagnostic::error("E0310", format!("field `{}` is given twice", label.name)).primary(label.span, "duplicate field"));
                ok = false;
            } else if i != next_expected && ok {
                self.err(
                    Diagnostic::error("E0310", "fields must be given in declaration order")
                        .primary(label.span, format!("`{}` given out of order", label.name))
                        .note(format!("declaration order: {}", names.join(", "))),
                );
                ok = false;
            }
            next_expected = i + 1;
            let fty = tcx.subst(f.ty, &targs);
            let mut x = match &a.expr.kind {
                A::Closure(_) => self.misplaced_closure(a.expr.span),
                _ => self.check_expr(&a.expr, Expect::Has(fty)),
            };
            self.coerce(&mut x, fty);
            out[i] = Some(x);
        }
        let missing: Vec<&str> = names.iter().zip(out.iter()).filter(|(_, x)| x.is_none()).map(|(n, _)| n.as_str()).collect();
        if !missing.is_empty() && ok {
            self.err(
                Diagnostic::error("E0310", format!("missing field{} {}", if missing.len() == 1 { "" } else { "s" }, missing.iter().map(|m| format!("`{}`", m)).collect::<Vec<_>>().join(", ")))
                    .primary(span, format!("`{}` has {} field(s)", adt.name, names.len()))
                    .note(format!("every field must be given, in order: {}", names.join(", "))),
            );
        }
        if out.iter().any(|x| x.is_none()) {
            return self.mk(T::Error, ty, span);
        }
        let fields = out.into_iter().map(|x| x.unwrap()).collect();
        self.mk(T::Adt { adt: id, variant: None, type_args: targs, fields }, ty, span)
    }

    fn check_variant_ctor(&mut self, id: AdtId, v: u32, explicit: Option<Vec<Ty>>, args: &[ast::Arg], expect: Expect, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let adt = self.p.adt(id);
        let variant = &adt.variants()[v as usize];
        let targs = match explicit {
            Some(a) => a,
            None => self.fresh_args(adt.generics.len(), span),
        };
        let ty = tcx.adt(id, targs.clone());
        if let Some(t) = expect.ty() {
            self.unify(ty, t);
        }
        if variant.fields.is_empty() {
            self.err(
                Diagnostic::error("E0306", format!("variant `{}.{}` has no values", adt.name, variant.name))
                    .primary(span, "remove the parentheses")
                    .help(format!("write `{}`", variant.name)),
            );
            self.check_args_for_errors(args);
            return self.error_expr(span);
        }
        if variant.fields.len() != args.len() {
            self.err(
                Diagnostic::error("E0304", format!("variant `{}` takes {} value(s) but {} were given", variant.name, variant.fields.len(), args.len()))
                    .primary(span, "wrong number of values"),
            );
        }
        let field_tys: Vec<Ty> = variant.fields.iter().map(|&f| tcx.subst(f, &targs)).collect();
        let mut out = Vec::new();
        for (i, a) in args.iter().enumerate() {
            if let Some(l) = &a.label {
                self.err(Diagnostic::error("E0305", "variant values are positional").primary(l.span, "remove this label"));
            }
            if let Some(amp) = a.inout {
                self.err(Diagnostic::error("E0412", "`&` is only used for `inout` arguments").primary(amp, "remove this `&`"));
            }
            let fty = field_tys.get(i).copied().unwrap_or(tcx.error);
            let mut x = self.check_expr(&a.expr, Expect::Has(fty));
            self.coerce(&mut x, fty);
            out.push(x);
        }
        if out.len() != field_tys.len() {
            return self.mk(T::Error, ty, span);
        }
        self.mk(T::Adt { adt: id, variant: Some(v), type_args: targs, fields: out }, ty, span)
    }
}

fn static_name(n: &str) -> &'static str {
    const NAMES: &[&str] = &[
        "sqrt", "floor", "ceil", "round", "trunc", "sin", "cos", "tan", "asin", "acos", "atan", "exp", "ln", "log10", "log2", "abs", "powf",
        "atan2", "min", "max", "hypot", "copysign", "is_ascii_digit", "is_ascii_alphabetic", "is_ascii_alphanumeric", "is_whitespace",
        "is_ascii_uppercase", "is_ascii_lowercase", "is_ascii",
    ];
    NAMES.iter().copied().find(|s| *s == n).unwrap_or("?")
}

/// Reinterprets an expression as a type expression (for generic arguments written in
/// expression position, such as `Vec[i32]` or `size_of[[u8; 4]]`).
pub(crate) fn expr_to_type_expr(e: &ast::Expr) -> Option<TypeExpr> {
    let kind = match &e.kind {
        A::Ident(_) | A::Field(..) => {
            let mut segs = Vec::new();
            let mut cur = e;
            loop {
                match &cur.kind {
                    A::Ident(n) => {
                        segs.push(ast::Ident { name: n.clone(), span: cur.span });
                        break;
                    }
                    A::Field(b, f) => {
                        segs.push(f.clone());
                        cur = b;
                    }
                    _ => return None,
                }
            }
            segs.reverse();
            TypeExprKind::Path { segments: segs, args: vec![] }
        }
        A::Index(base, args) => {
            let b = expr_to_type_expr(base)?;
            let TypeExprKind::Path { segments, args: prev } = b.kind else { return None };
            if !prev.is_empty() {
                return None;
            }
            let args: Option<Vec<TypeExpr>> = args.iter().map(expr_to_type_expr).collect();
            TypeExprKind::Path { segments, args: args? }
        }
        A::ArrayRepeat(el, count) => TypeExprKind::Array(Box::new(expr_to_type_expr(el)?), count.clone()),
        A::Tuple(elems) => {
            let elems: Option<Vec<TypeExpr>> = elems.iter().map(expr_to_type_expr).collect();
            TypeExprKind::Tuple(elems?)
        }
        A::Unit => TypeExprKind::Unit,
        A::Paren(inner) => return expr_to_type_expr(inner),
        A::SelfType => TypeExprKind::SelfType,
        A::Type(t) => return Some(t.clone()),
        _ => return None,
    };
    Some(TypeExpr { kind, span: e.span })
}

#[allow(dead_code)]
fn is_primitive_name(n: &str) -> bool {
    PRIMITIVE_TYPES.contains(&n)
}

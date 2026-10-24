//! Item collection: declares every item, binds imports, resolves signatures and
//! validates declarations (spec chapters 03 and 06).

use crate::defs::*;
use crate::ty::{IntTy, Ty, TyKind};
use crate::util::{best_match, PRIMITIVE_TYPES};
use nxc_syntax::ast::{self, Convention, ImportKind, ItemKind, Lit, ParamKind, TypeExpr, TypeExprKind};
use nxc_syntax::{DiagSink, Diagnostic, Span};
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

/// Context for resolving a type expression.
pub struct TypeScope<'a> {
    pub module: ModId,
    pub generics: &'a [String],
    pub self_ty: Option<Ty>,
}

struct PendingImpl {
    module: ModId,
    decl: ast::ImplDecl,
    span: Span,
}

/// What a function signature is resolved inside of: an `impl` block or a `trait`.
pub(crate) struct SigOwner {
    pub generics: Vec<String>,
    pub self_ty: Option<Ty>,
    pub bounds: Vec<Bound>,
    pub trait_id: Option<TraitId>,
}

pub fn collect(prog: &mut Program, diags: &DiagSink) {
    let mut impls: Vec<PendingImpl> = Vec::new();
    let mut pending_fns: Vec<(FnId, Rc<ast::FnDecl>, Vec<ast::Attr>)> = Vec::new();
    let mut pending_structs: Vec<(AdtId, ast::StructDecl)> = Vec::new();
    let mut pending_enums: Vec<(AdtId, ast::EnumDecl)> = Vec::new();
    let mut pending_aliases: Vec<(AliasId, ast::TypeAliasDecl)> = Vec::new();
    let mut pending_externs: Vec<(FnId, ast::ExternFnDecl, Vec<ast::Attr>)> = Vec::new();
    let mut pending_traits: Vec<(TraitId, ast::TraitDecl)> = Vec::new();

    // Phase 1: declare names.
    for m in 0..prog.modules.len() {
        let mid = ModId(m as u32);
        let Some(module_ast) = prog.modules[m].ast.take() else { continue };
        let is_std = prog.modules[m].is_std;
        for item in module_ast.items {
            check_attrs(&item, is_std, diags);
            let name_ident = match &item.kind {
                ItemKind::Fn(f) => Some(f.name.clone()),
                ItemKind::Struct(s) => Some(s.name.clone()),
                ItemKind::Enum(e) => Some(e.name.clone()),
                ItemKind::Const(c) => Some(c.name.clone()),
                ItemKind::TypeAlias(t) => Some(t.name.clone()),
                ItemKind::Newtype(n) => Some(n.name.clone()),
                ItemKind::ExternFn(e) => Some(e.name.clone()),
                ItemKind::Static(st) => Some(st.name.clone()),
                ItemKind::Trait(t) => Some(t.name.clone()),
                ItemKind::Impl(_) => None,
            };
            let lang_text = item.attrs.iter().find(|a| a.name.name == "lang").and_then(|a| match a.args.first() {
                Some((Lit::Str(s), _)) => Some(s.clone()),
                _ => None,
            });
            let lang = lang_text.as_deref().and_then(LangItem::from_name);
            if let (Some(name), ItemKind::Struct(_) | ItemKind::Enum(_)) = (&lang_text, &item.kind) {
                prog.lang_adts.insert(name.clone(), AdtId(prog.adts.len() as u32));
            }
            let item_ref = match item.kind {
                ItemKind::Impl(decl) => {
                    impls.push(PendingImpl { module: mid, decl, span: item.span });
                    continue;
                }
                ItemKind::Fn(f) => {
                    let id = FnId(prog.fns.len() as u32);
                    let f = Rc::new(f);
                    prog.fns.push(new_fn_def(&f, mid, item.is_pub, None, &item.attrs));
                    pending_fns.push((id, f, item.attrs.clone()));
                    ItemRef::Fn(id)
                }
                ItemKind::ExternFn(e) => {
                    let id = FnId(prog.fns.len() as u32);
                    let link = item.attrs.iter().find(|a| a.name.name == "link").and_then(|a| match a.args.first() {
                        Some((Lit::Str(s), _)) => Some(s.clone()),
                        _ => None,
                    });
                    prog.fns.push(FnDef {
                        name: e.name.name.clone(),
                        module: mid,
                        span: e.name.span,
                        is_pub: item.is_pub,
                        owner: None,
                        generics: vec![],
                        n_owner_generics: 0,
                        params: vec![],
                        has_self: false,
                        ret: prog.tcx.unit,
                        is_unsafe: true,
                        kind: FnKind::Extern { abi: e.abi.clone(), link },
                        is_test: false,
                        expects_panic: false,
                        track_caller: false,
                        export: None,
                        hints: vec![],
                        self_ty: None,
                        bounds: vec![],
                        abi_params: None,
                        trait_id: None,
                    });
                    pending_externs.push((id, e, item.attrs.clone()));
                    ItemRef::Fn(id)
                }
                ItemKind::Struct(s) => {
                    let id = AdtId(prog.adts.len() as u32);
                    let newtype = prog.modules[m].newtypes.get(&s.name.name).map(|d| NewtypeDef { distinct: d.distinct, cond: d.cond.clone() });
                    prog.adts.push(AdtDef {
                        name: s.name.name.clone(),
                        module: mid,
                        span: s.name.span,
                        is_pub: item.is_pub,
                        generics: s.generics.iter().map(|g| g.name.clone()).collect(),
                        kind: AdtKind::Struct { fields: vec![] },
                        methods: HashMap::new(),
                        drop_fn: None,
                        index_fn: None,
                        lang,
                        newtype,
                    });
                    check_generic_names(&s.generics, diags);
                    pending_structs.push((id, s));
                    ItemRef::Adt(id)
                }
                ItemKind::Enum(e) => {
                    let id = AdtId(prog.adts.len() as u32);
                    prog.adts.push(AdtDef {
                        name: e.name.name.clone(),
                        module: mid,
                        span: e.name.span,
                        is_pub: item.is_pub,
                        generics: e.generics.iter().map(|g| g.name.clone()).collect(),
                        kind: AdtKind::Enum { variants: vec![] },
                        methods: HashMap::new(),
                        drop_fn: None,
                        index_fn: None,
                        lang,
                        newtype: None,
                    });
                    check_generic_names(&e.generics, diags);
                    pending_enums.push((id, e));
                    ItemRef::Adt(id)
                }
                ItemKind::Const(c) => {
                    let id = ConstId(prog.consts.len() as u32);
                    prog.consts.push(ConstDef {
                        name: c.name.name.clone(),
                        module: mid,
                        span: c.name.span,
                        is_pub: item.is_pub,
                        ty: prog.tcx.error,
                        ast: Rc::new(c),
                    });
                    ItemRef::Const(id)
                }
                ItemKind::Static(st) => {
                    let id = StaticId(prog.statics.len() as u32);
                    prog.statics.push(StaticDef {
                        name: st.name.name.clone(),
                        module: mid,
                        span: st.name.span,
                        is_pub: item.is_pub,
                        mutable: st.mutable,
                        ty: prog.tcx.error,
                        ast: Rc::new(st),
                    });
                    ItemRef::Static(id)
                }
                ItemKind::Trait(t) => {
                    let id = TraitId(prog.traits.len() as u32);
                    prog.traits.push(TraitDef { name: t.name.name.clone(), module: mid, span: t.name.span, is_pub: item.is_pub, supertraits: vec![], methods: vec![] });
                    if let Some(name) = lang_name(&item.attrs) {
                        prog.lang_traits.insert(name, id);
                    }
                    pending_traits.push((id, t));
                    ItemRef::Trait(id)
                }
                // Replaced by a structure before the analysis (`newtype::expand`).
                ItemKind::Newtype(_) => continue,
                ItemKind::TypeAlias(t) => {
                    let id = AliasId(prog.aliases.len() as u32);
                    prog.aliases.push(AliasDef { name: t.name.name.clone(), module: mid, span: t.name.span, is_pub: item.is_pub, ty: prog.tcx.error });
                    pending_aliases.push((id, t));
                    ItemRef::Alias(id)
                }
            };
            let ident = name_ident.unwrap();
            declare(prog, mid, &ident, item_ref, item.is_pub, diags);
        }
        // Keep the imports for binding.
        prog.modules[m].ast = Some(ast::Module { decl: module_ast.decl, imports: module_ast.imports, items: vec![] });
    }

    // Lang items and prelude.
    for (i, adt) in prog.adts.iter().enumerate() {
        let id = AdtId(i as u32);
        match adt.lang {
            Some(LangItem::String) => prog.lang.string = Some(id),
            Some(LangItem::Vec) => prog.lang.vec = Some(id),
            Some(LangItem::Option) => prog.lang.option = Some(id),
            Some(LangItem::Result) => prog.lang.result = Some(id),
            None => {}
        }
    }
    for (id, _, attrs) in &pending_fns {
        if let Some(name) = lang_name(attrs) {
            prog.lang_fns.insert(name, *id);
        }
    }
    for b in [Builtin::Print, Builtin::Println, Builtin::Eprint, Builtin::Eprintln, Builtin::Panic, Builtin::Assert, Builtin::Unreachable] {
        prog.prelude.insert(b.name().to_string(), ItemRef::Builtin(b));
    }
    for (name, id) in [("String", prog.lang.string), ("Vec", prog.lang.vec), ("Option", prog.lang.option), ("Result", prog.lang.result)] {
        if let Some(id) = id {
            prog.prelude.insert(name.to_string(), ItemRef::Adt(id));
        }
    }
    // The traits of `std.core` (`Eq`, `Ord`, `Display`...) and `Ordering`.
    if let Some(core) = prog.modules.iter().position(|m| m.path == "std.core") {
        let items: Vec<(String, ItemRef)> = prog.modules[core].items.iter().filter(|(_, v)| v.2).map(|(k, v)| (k.clone(), v.0)).collect();
        for (name, item) in items {
            if matches!(item, ItemRef::Trait(_) | ItemRef::Adt(_)) {
                prog.prelude.entry(name).or_insert(item);
            }
        }
    }
    // The variants of `Option` and `Result` are in the prelude too.
    for id in [prog.lang.option, prog.lang.result].into_iter().flatten() {
        let names: Vec<String> = pending_enums
            .iter()
            .find(|(a, _)| *a == id)
            .map(|(_, e)| e.variants.iter().map(|v| v.name.name.clone()).collect())
            .unwrap_or_default();
        for (i, n) in names.into_iter().enumerate() {
            prog.prelude.insert(n, ItemRef::Variant(id, i as u32));
        }
    }

    // Phase 2: item imports.
    bind_item_imports(prog, diags);

    // Phase 3: resolve declarations. Aliases first (other declarations may use them).
    for (id, decl) in &pending_aliases {
        let module = prog.aliases[id.idx()].module;
        let mut visiting = vec![*id];
        let ty = resolve_alias_target(prog, module, decl, &mut visiting, diags);
        prog.aliases[id.idx()].ty = ty;
    }
    for (id, decl) in &pending_structs {
        let adt = &prog.adts[id.idx()];
        let (module, generics) = (adt.module, adt.generics.clone());
        let self_ty = Some(prog.tcx.adt(*id, (0..generics.len() as u32).map(|i| prog.tcx.param(i)).collect()));
        let scope = TypeScope { module, generics: &generics, self_ty };
        let mut fields: Vec<FieldDef> = Vec::new();
        for f in &decl.fields {
            if fields.iter().any(|x| x.name == f.name.name) {
                diags.emit(Diagnostic::error("E0201", format!("field `{}` is declared twice", f.name.name)).primary(f.name.span, "duplicate field"));
                continue;
            }
            let ty = resolve_type(prog, &scope, &f.ty, diags);
            fields.push(FieldDef { name: f.name.name.clone(), ty, is_pub: f.is_pub, indirect: f.indirect, span: f.name.span });
        }
        prog.adts[id.idx()].kind = AdtKind::Struct { fields };
    }
    for (id, decl) in &pending_enums {
        let adt = &prog.adts[id.idx()];
        let (module, generics) = (adt.module, adt.generics.clone());
        let self_ty = Some(prog.tcx.adt(*id, (0..generics.len() as u32).map(|i| prog.tcx.param(i)).collect()));
        let scope = TypeScope { module, generics: &generics, self_ty };
        let mut variants: Vec<VariantDef> = Vec::new();
        for v in &decl.variants {
            if variants.iter().any(|x| x.name == v.name.name) {
                diags.emit(Diagnostic::error("E0201", format!("variant `{}` is declared twice", v.name.name)).primary(v.name.span, "duplicate variant"));
                continue;
            }
            let fields = v.fields.iter().map(|t| resolve_type(prog, &scope, t, diags)).collect();
            variants.push(VariantDef { name: v.name.name.clone(), fields, indirect: v.indirect, span: v.name.span });
        }
        if variants.is_empty() {
            diags.emit(
                Diagnostic::error("E0312", format!("enum `{}` has no variants", decl.name.name))
                    .primary(decl.name.span, "an enum needs at least one variant")
                    .help("a type without values is spelled `Never`"),
            );
        }
        prog.adts[id.idx()].kind = AdtKind::Enum { variants };
    }
    for (id, decl, _) in &pending_externs {
        resolve_extern(prog, *id, decl, diags);
    }
    for c in 0..prog.consts.len() {
        let module = prog.consts[c].module;
        let ast = prog.consts[c].ast.clone();
        let scope = TypeScope { module, generics: &[], self_ty: None };
        let ty = resolve_type(prog, &scope, &ast.ty, diags);
        prog.consts[c].ty = ty;
    }
    for (id, decl) in &pending_traits {
        collect_trait(prog, *id, decl, diags);
    }
    for (id, decl, attrs) in &pending_fns {
        resolve_fn_sig(prog, *id, decl, attrs, None, diags);
    }
    for imp in impls {
        if imp.decl.trait_.is_some() {
            collect_trait_impl(prog, imp, diags);
        } else {
            collect_impl(prog, imp, diags);
        }
    }
    check_impls(prog, diags);
    // Static variables, once every destructor is known (a type with `drop` is a resource).
    for st in 0..prog.statics.len() {
        let module = prog.statics[st].module;
        let ast = prog.statics[st].ast.clone();
        let scope = TypeScope { module, generics: &[], self_ty: None };
        let ty = resolve_type(prog, &scope, &ast.ty, diags);
        if !prog.tcx.is_error(ty) && !prog.is_copy(ty) {
            diags.emit(
                Diagnostic::error("E0374", format!("a static variable cannot hold a resource of type `{}`", prog.ty_str(ty)))
                    .primary(ast.ty.span, "not a copyable type")
                    .note("static variables live for the whole program and are never destroyed")
                    .help("use a `const` for a constant string, or store the resource in a local variable"),
            );
        }
        prog.statics[st].ty = ty;
    }

    check_recursive_types(prog, diags);
    check_lang_items(prog, diags);
    find_main(prog, diags);
}

fn new_fn_def(f: &Rc<ast::FnDecl>, module: ModId, is_pub: bool, owner: Option<AdtId>, attrs: &[ast::Attr]) -> FnDef {
    let intrinsic = attrs.iter().any(|a| a.name.name == "intrinsic");
    let kind = if intrinsic { FnKind::Intrinsic(f.name.name.clone()) } else { FnKind::User(f.clone()) };
    FnDef {
        name: f.name.name.clone(),
        module,
        span: f.name.span,
        is_pub,
        owner,
        generics: vec![],
        n_owner_generics: 0,
        params: vec![],
        has_self: false,
        ret: Ty(0),
        is_unsafe: f.is_unsafe,
        kind,
        is_test: attrs.iter().any(|a| a.name.name == "test"),
        expects_panic: attrs.iter().any(|a| a.name.name == "expects panic"),
        track_caller: attrs.iter().any(|a| a.name.name == "track_caller"),
        export: export_name(attrs, &f.name.name),
        hints: attrs.iter().filter(|a| matches!(a.name.name.as_str(), "cold" | "inline" | "no_builtins")).map(|a| a.name.name.clone()).collect(),
        self_ty: None,
        bounds: vec![],
        abi_params: None,
        trait_id: None,
    }
}

/// `@lang("name")` on a function.
fn lang_name(attrs: &[ast::Attr]) -> Option<String> {
    attrs.iter().find(|a| a.name.name == "lang").and_then(|a| match a.args.first() {
        Some((Lit::Str(s), _)) => Some(s.clone()),
        _ => None,
    })
}

/// `@export` (symbol = function name) or `@export("symbol")`.
fn export_name(attrs: &[ast::Attr], fn_name: &str) -> Option<String> {
    attrs.iter().find(|a| a.name.name == "export").map(|a| match a.args.first() {
        Some((Lit::Str(s), _)) => s.clone(),
        _ => fn_name.to_string(),
    })
}

fn check_generic_names(gens: &[ast::Ident], diags: &DiagSink) {
    for (i, g) in gens.iter().enumerate() {
        if gens[..i].iter().any(|h| h.name == g.name) {
            diags.emit(Diagnostic::error("E0201", format!("type parameter `{}` is declared twice", g.name)).primary(g.span, "duplicate parameter"));
        }
        if PRIMITIVE_TYPES.contains(&g.name.as_str()) {
            diags.emit(Diagnostic::error("E0213", format!("`{}` is a built-in type and cannot be a type parameter name", g.name)).primary(g.span, "reserved name"));
        }
    }
}

fn check_attrs(item: &ast::Item, is_std: bool, diags: &DiagSink) {
    for a in &item.attrs {
        let name = a.name.name.as_str();
        match name {
            "test" | "link" | "export" | "cfg" | "inline" | "cold" | "derive" | "expects panic" => {}
            "lang" | "intrinsic" | "index" | "track_caller" | "no_builtins" => {
                // The validation of a constrained type panics where `as` is written (ADR 0014).
                let validation = name == "track_caller" && matches!(&item.kind, ItemKind::Fn(f) if f.name.name == crate::newtype::VALIDATE_FN);
                if !is_std && !validation {
                    diags.emit(
                        Diagnostic::error("E0231", format!("the attribute `@{}` is reserved for the standard library", name))
                            .primary(a.span, "not allowed here"),
                    );
                }
            }
            _ => {
                let mut d = Diagnostic::error("E0232", format!("unknown attribute `@{}`", name)).primary(a.name.span, "unknown attribute");
                if let Some(s) = best_match(name, ["test", "link", "export", "cfg", "inline", "cold", "derive"]) {
                    d = d.help(format!("did you mean `@{}`?", s));
                }
                diags.emit(d);
            }
        }
        let ok_target = match (name, &item.kind) {
            ("test" | "expects panic", ItemKind::Fn(_)) => true,
            ("link", ItemKind::ExternFn(_)) => true,
            ("lang", ItemKind::Struct(_) | ItemKind::Enum(_) | ItemKind::Fn(_) | ItemKind::Trait(_)) => true,
            ("intrinsic", ItemKind::Fn(_)) => true,
            ("index", ItemKind::Fn(_)) => true,
            ("track_caller", ItemKind::Fn(_)) => true,
            ("export" | "inline" | "cold" | "no_builtins", ItemKind::Fn(_)) => true,
            ("cfg", _) => true,
            ("derive", ItemKind::Struct(_) | ItemKind::Enum(_)) => true,
            ("derive", _) => false,
            ("test" | "link" | "lang" | "intrinsic" | "index" | "track_caller" | "export" | "inline" | "cold" | "no_builtins", _) => false,
            _ => true,
        };
        if !ok_target {
            diags.emit(Diagnostic::error("E0232", format!("the attribute `@{}` cannot be applied to this item", name)).primary(a.span, "misplaced attribute"));
        }
    }
}

fn declare(prog: &mut Program, mid: ModId, ident: &ast::Ident, item: ItemRef, is_pub: bool, diags: &DiagSink) {
    if PRIMITIVE_TYPES.contains(&ident.name.as_str()) || ident.name == "Self" {
        diags.emit(
            Diagnostic::error("E0213", format!("`{}` is a built-in type name and cannot be redefined", ident.name)).primary(ident.span, "reserved name"),
        );
        return;
    }
    let module = &mut prog.modules[mid.idx()];
    if let Some((_, prev, _)) = module.items.get(&ident.name) {
        diags.emit(
            Diagnostic::error("E0201", format!("the name `{}` is defined twice in this module", ident.name))
                .primary(ident.span, "redefined here")
                .secondary(*prev, "first definition"),
        );
        return;
    }
    if let Some(imp) = module.imports.get(&ident.name) {
        diags.emit(
            Diagnostic::error("E0223", format!("the name `{}` is both imported and defined", ident.name))
                .primary(ident.span, "defined here")
                .secondary(imp.span, "imported here"),
        );
        return;
    }
    module.items.insert(ident.name.clone(), (item, ident.span, is_pub));
}

fn bind_item_imports(prog: &mut Program, diags: &DiagSink) {
    for m in 0..prog.modules.len() {
        let imports = match &prog.modules[m].ast {
            Some(a) => a.imports.clone(),
            None => continue,
        };
        for imp in imports {
            let ImportKind::Items(list) = &imp.kind else { continue };
            let path: String = imp.path.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(".");
            let Some(target) = crate::load::module_by_path(prog, &path) else { continue };
            for (name, alias) in list {
                let found = prog.modules[target.idx()].items.get(&name.name).copied();
                let Some((item, _, is_pub)) = found else {
                    let mut d = Diagnostic::error("E0222", format!("module `{}` has no item `{}`", path, name.name)).primary(name.span, "not found");
                    let cands: Vec<String> = prog.modules[target.idx()].items.iter().filter(|(_, v)| v.2).map(|(k, _)| k.clone()).collect();
                    if let Some(s) = best_match(&name.name, cands.iter().map(|s| s.as_str())) {
                        d = d.help(format!("did you mean `{}`?", s));
                    }
                    diags.emit(d);
                    continue;
                };
                if !is_pub && target.idx() != m {
                    diags.emit(
                        Diagnostic::error("E0222", format!("`{}` is private to module `{}`", name.name, path))
                            .primary(name.span, "private item")
                            .help(format!("mark it `pub` in `{}` to export it", path)),
                    );
                    continue;
                }
                let local = alias.as_ref().unwrap_or(name);
                let module = &mut prog.modules[m];
                if module.items.contains_key(&local.name) || module.imports.contains_key(&local.name) {
                    diags.emit(Diagnostic::error("E0223", format!("the name `{}` is already defined or imported", local.name)).primary(local.span, "conflicting name"));
                    continue;
                }
                module.imports.insert(local.name.clone(), ImportEntry { item, span: local.span, used: Cell::new(false) });
            }
        }
    }
}

/// Looks a name up at module level: own items, then imports, then the prelude.
pub fn lookup_item(prog: &Program, module: ModId, name: &str) -> Option<ItemRef> {
    let m = &prog.modules[module.idx()];
    if let Some((it, _, _)) = m.items.get(name) {
        return Some(*it);
    }
    if let Some(imp) = m.imports.get(name) {
        imp.used.set(true);
        return Some(imp.item);
    }
    prog.prelude.get(name).copied()
}

/// Is the item visible from `from`?
pub fn item_is_pub(prog: &Program, item: ItemRef) -> bool {
    match item {
        ItemRef::Module(_) | ItemRef::Builtin(_) => true,
        ItemRef::Adt(a) => prog.adts[a.idx()].is_pub,
        ItemRef::Fn(f) => prog.fns[f.idx()].is_pub,
        ItemRef::Const(c) => prog.consts[c.idx()].is_pub,
        ItemRef::Alias(a) => prog.aliases[a.idx()].is_pub,
        ItemRef::Variant(a, _) => prog.adts[a.idx()].is_pub,
        ItemRef::Static(st) => prog.statics[st.idx()].is_pub,
        ItemRef::Trait(t) => prog.traits[t.idx()].is_pub,
    }
}

/// Looks up `name` inside module `target`, as seen from module `from`.
pub fn lookup_in_module(prog: &Program, from: ModId, target: ModId, name: &str, span: Span, diags: &DiagSink) -> Option<ItemRef> {
    let m = &prog.modules[target.idx()];
    match m.items.get(name) {
        Some(&(item, _, is_pub)) => {
            if !is_pub && from != target {
                diags.emit(
                    Diagnostic::error("E0222", format!("`{}` is private to module `{}`", name, m.path))
                        .primary(span, "private item")
                        .help(format!("mark it `pub` in `{}` to use it here", m.path)),
                );
            }
            Some(item)
        }
        None => {
            let mut d = Diagnostic::error("E0202", format!("module `{}` has no item `{}`", m.path, name)).primary(span, "not found in this module");
            let cands: Vec<&str> = m.items.iter().filter(|(_, v)| v.2).map(|(k, _)| k.as_str()).collect();
            if let Some(s) = best_match(name, cands) {
                d = d.help(format!("did you mean `{}`?", s));
            }
            diags.emit(d);
            None
        }
    }
}

fn resolve_alias_target(prog: &mut Program, module: ModId, decl: &ast::TypeAliasDecl, visiting: &mut Vec<AliasId>, diags: &DiagSink) -> Ty {
    // Aliases may refer to other aliases declared later: resolve those first.
    if let TypeExprKind::Path { segments, .. } = &decl.ty.kind {
        if segments.len() == 1 {
            if let Some(ItemRef::Alias(other)) = lookup_item(prog, module, &segments[0].name) {
                if visiting.contains(&other) {
                    diags.emit(Diagnostic::error("E0311", format!("type alias `{}` refers to itself", decl.name.name)).primary(decl.name.span, "cyclic alias"));
                    return prog.tcx.error;
                }
            }
        }
    }
    let scope = TypeScope { module, generics: &[], self_ty: None };
    resolve_type(prog, &scope, &decl.ty, diags)
}

/// Converts a type expression into a type.
pub fn resolve_type(prog: &Program, scope: &TypeScope, t: &TypeExpr, diags: &DiagSink) -> Ty {
    let tcx = &prog.tcx;
    match &t.kind {
        TypeExprKind::Error => tcx.error,
        TypeExprKind::Unit => tcx.unit,
        TypeExprKind::SelfType => match scope.self_ty {
            Some(s) => s,
            None => {
                diags.emit(Diagnostic::error("E0202", "`Self` is only available inside an `impl` block").primary(t.span, "no `Self` here"));
                tcx.error
            }
        },
        TypeExprKind::Tuple(elems) => {
            let tys = elems.iter().map(|e| resolve_type(prog, scope, e, diags)).collect();
            tcx.tuple(tys)
        }
        TypeExprKind::Array(elem, len) => {
            let e = resolve_type(prog, scope, elem, diags);
            match crate::consts::eval_array_len(prog, scope.module, len, diags) {
                Some(n) => tcx.mk(TyKind::Array(e, n)),
                None => tcx.error,
            }
        }
        TypeExprKind::Fn(params, ret) => {
            let ps = params.iter().map(|(c, p)| (*c, resolve_type(prog, scope, p, diags))).collect();
            let r = match ret {
                Some(r) => resolve_type(prog, scope, r, diags),
                None => tcx.unit,
            };
            tcx.mk(TyKind::Fn(ps, r))
        }
        TypeExprKind::Dyn(inner) => match resolve_trait_ref(prog, scope.module, inner, diags) {
            Some(tr) => tcx.mk(TyKind::Dyn(tr)),
            None => tcx.error,
        },
        TypeExprKind::Path { segments, args } => {
            let arg_tys: Vec<Ty> = args.iter().map(|a| resolve_type(prog, scope, a, diags)).collect();
            if segments.len() == 1 {
                let name = segments[0].name.as_str();
                if let Some(i) = scope.generics.iter().position(|g| g == name) {
                    if !args.is_empty() {
                        diags.emit(Diagnostic::error("E0315", format!("type parameter `{}` does not take generic arguments", name)).primary(t.span, "unexpected arguments"));
                    }
                    return tcx.param(i as u32);
                }
                if let Some(prim) = primitive_type(prog, name) {
                    if !args.is_empty() {
                        diags.emit(Diagnostic::error("E0315", format!("`{}` does not take generic arguments", name)).primary(t.span, "unexpected arguments"));
                    }
                    return prim;
                }
                if name == "Ptr" {
                    if arg_tys.len() != 1 {
                        diags.emit(
                            Diagnostic::error("E0315", "`Ptr` takes exactly one generic argument")
                                .primary(t.span, format!("{} argument(s) given", arg_tys.len()))
                                .help("write `Ptr[T]`"),
                        );
                        return tcx.error;
                    }
                    return tcx.ptr(arg_tys[0]);
                }
                match lookup_item(prog, scope.module, name) {
                    Some(item) => item_as_type(prog, item, &arg_tys, t.span, name, diags),
                    None => {
                        let mut d = Diagnostic::error("E0202", format!("cannot find type `{}`", name)).primary(segments[0].span, "not found in this scope");
                        let mut cands: Vec<String> = prog.modules[scope.module.idx()].items.keys().cloned().collect();
                        cands.extend(prog.modules[scope.module.idx()].imports.keys().cloned());
                        cands.extend(prog.prelude.keys().cloned());
                        cands.extend(PRIMITIVE_TYPES.iter().map(|s| s.to_string()));
                        cands.extend(scope.generics.iter().cloned());
                        if let Some(s) = best_match(name, cands.iter().map(|s| s.as_str())) {
                            d = d.help(format!("did you mean `{}`?", s));
                        } else if matches!(name, "int" | "float" | "string" | "str" | "double" | "boolean") {
                            let s = match name {
                                "int" => "i32",
                                "float" | "double" => "f64",
                                "boolean" => "bool",
                                _ => "String",
                            };
                            d = d.help(format!("NEXUL spells this type `{}`", s));
                        }
                        diags.emit(d);
                        tcx.error
                    }
                }
            } else {
                // module.path.Type
                let mut module = match lookup_item(prog, scope.module, &segments[0].name) {
                    Some(ItemRef::Module(m)) => m,
                    Some(_) => {
                        diags.emit(Diagnostic::error("E0314", format!("`{}` is not a module", segments[0].name)).primary(segments[0].span, "expected a module"));
                        return tcx.error;
                    }
                    None => {
                        diags.emit(
                            Diagnostic::error("E0202", format!("cannot find module `{}`", segments[0].name))
                                .primary(segments[0].span, "not found")
                                .help(format!("add `import ...{}` at the top of the file", segments[0].name)),
                        );
                        return tcx.error;
                    }
                };
                for (i, seg) in segments.iter().enumerate().skip(1) {
                    let item = match lookup_in_module(prog, scope.module, module, &seg.name, seg.span, diags) {
                        Some(it) => it,
                        None => return tcx.error,
                    };
                    if i + 1 == segments.len() {
                        return item_as_type(prog, item, &arg_tys, t.span, &seg.name, diags);
                    }
                    match item {
                        ItemRef::Module(m) => module = m,
                        _ => {
                            diags.emit(Diagnostic::error("E0314", format!("`{}` is not a module", seg.name)).primary(seg.span, "expected a module"));
                            return tcx.error;
                        }
                    }
                }
                tcx.error
            }
        }
    }
}

pub fn primitive_type(prog: &Program, name: &str) -> Option<Ty> {
    let tcx = &prog.tcx;
    if let Some(i) = IntTy::from_name(name) {
        return Some(tcx.int(i));
    }
    Some(match name {
        "f32" => tcx.f32,
        "f64" => tcx.f64,
        "bool" => tcx.bool,
        "char" => tcx.char,
        "Never" => tcx.never,
        _ => return None,
    })
}

fn item_as_type(prog: &Program, item: ItemRef, args: &[Ty], span: Span, name: &str, diags: &DiagSink) -> Ty {
    let tcx = &prog.tcx;
    match item {
        ItemRef::Adt(id) => {
            let adt = &prog.adts[id.idx()];
            if args.len() != adt.generics.len() {
                let expected = if adt.generics.is_empty() {
                    format!("`{}` takes no generic arguments", adt.name)
                } else {
                    format!("`{}` takes {} generic argument{}", adt.name, adt.generics.len(), if adt.generics.len() == 1 { "" } else { "s" })
                };
                let mut d = Diagnostic::error("E0315", expected).primary(span, format!("{} given", args.len()));
                if !adt.generics.is_empty() {
                    d = d.help(format!("write `{}[{}]`", adt.name, adt.generics.join(", ")));
                }
                diags.emit(d);
                return tcx.error;
            }
            tcx.adt(id, args.to_vec())
        }
        ItemRef::Alias(a) => {
            if !args.is_empty() {
                diags.emit(Diagnostic::error("E0315", format!("type alias `{}` takes no generic arguments", name)).primary(span, "unexpected arguments"));
            }
            prog.aliases[a.idx()].ty
        }
        ItemRef::Module(_) => {
            diags.emit(Diagnostic::error("E0314", format!("expected a type, found module `{}`", name)).primary(span, "not a type"));
            tcx.error
        }
        ItemRef::Trait(_) => {
            diags.emit(
                Diagnostic::error("E0314", format!("expected a type, found trait `{}`", name))
                    .primary(span, "a trait is not a type")
                    .help(format!("for a value of any type implementing it, write `dyn {}`; in a generic function, write `[T: {}]`", name, name)),
            );
            tcx.error
        }
        _ => {
            diags.emit(Diagnostic::error("E0314", format!("expected a type, found `{}`", name)).primary(span, "not a type"));
            tcx.error
        }
    }
}

fn is_ffi_safe(prog: &Program, t: Ty, is_ret: bool) -> bool {
    match &*prog.tcx.kind(t) {
        TyKind::Int(_) | TyKind::Float(_) | TyKind::Bool | TyKind::Char | TyKind::Ptr(_) | TyKind::Error => true,
        TyKind::Unit => is_ret,
        TyKind::Never => is_ret,
        // A function pointer is a C callback when its signature is itself FFI-safe:
        // scalar parameters are passed by value, exactly as in C.
        TyKind::Fn(params, r) => params.iter().all(|&(c, p)| c != Convention::Inout && is_ffi_safe(prog, p, false)) && is_ffi_safe(prog, *r, true),
        _ => false,
    }
}

fn resolve_extern(prog: &mut Program, id: FnId, decl: &ast::ExternFnDecl, diags: &DiagSink) {
    let module = prog.fns[id.idx()].module;
    if decl.abi != "C" {
        diags.emit(
            Diagnostic::error("E0502", format!("unsupported ABI `\"{}\"`", decl.abi))
                .primary(decl.abi_span, "unknown ABI")
                .note("NEXUL 0.1 supports only `extern \"C\"`"),
        );
    }
    let scope = TypeScope { module, generics: &[], self_ty: None };
    let mut params = Vec::new();
    for (name, texpr) in &decl.params {
        let ty = resolve_type(prog, &scope, texpr, diags);
        if !is_ffi_safe(prog, ty, false) {
            diags.emit(
                Diagnostic::error("E0503", format!("type `{}` cannot cross the C boundary", prog.ty_str(ty)))
                    .primary(texpr.span, "not FFI-safe")
                    .note("allowed: integers, floats, `bool`, `char` and `Ptr[T]` (see spec §8.2)"),
            );
        }
        params.push(ParamDef { name: name.name.clone(), conv: Convention::Borrow, ty, span: name.span });
    }
    let ret = match &decl.ret {
        Some(t) => {
            let ty = resolve_type(prog, &scope, t, diags);
            if !is_ffi_safe(prog, ty, true) {
                diags.emit(
                    Diagnostic::error("E0503", format!("type `{}` cannot cross the C boundary", prog.ty_str(ty)))
                        .primary(t.span, "not FFI-safe")
                        .note("allowed: integers, floats, `bool`, `char`, `Ptr[T]` and `()`"),
                );
            }
            ty
        }
        None => prog.tcx.unit,
    };
    let f = &mut prog.fns[id.idx()];
    f.params = params;
    f.ret = ret;
}

/// Resolves a function signature. `owner` is the impl block or trait, if any.
fn resolve_fn_sig(prog: &mut Program, id: FnId, decl: &ast::FnDecl, attrs: &[ast::Attr], owner: Option<SigOwner>, diags: &DiagSink) {
    let module = prog.fns[id.idx()].module;
    let mut generics: Vec<String> = owner.as_ref().map(|o| o.generics.clone()).unwrap_or_default();
    let n_owner = generics.len();
    for g in &decl.generics {
        if generics.contains(&g.name) {
            diags.emit(Diagnostic::error("E0201", format!("type parameter `{}` is already declared", g.name)).primary(g.span, "duplicate parameter"));
        }
        generics.push(g.name.clone());
    }
    check_generic_names(&decl.generics, diags);
    let self_ty = owner.as_ref().and_then(|o| o.self_ty);
    let scope = TypeScope { module, generics: &generics, self_ty };
    let mut bounds: Vec<Bound> = owner.as_ref().map(|o| o.bounds.clone()).unwrap_or_default();
    for b in &decl.bounds {
        let Some(i) = generics.iter().position(|g| *g == b.param.name) else {
            diags.emit(
                Diagnostic::error("E0240", format!("`{}` is not a type parameter of this function", b.param.name))
                    .primary(b.param.span, "unknown type parameter")
                    .help("bounds apply to the type parameters declared in `[...]`"),
            );
            continue;
        };
        for t in &b.traits {
            if let Some(tr) = resolve_trait_ref(prog, module, t, diags) {
                let bound = Bound { param: i as u32, trait_id: tr };
                if !bounds.contains(&bound) {
                    bounds.push(bound);
                }
            }
        }
    }
    let mut params = Vec::new();
    let mut has_self = false;
    for (i, p) in decl.params.iter().enumerate() {
        match &p.kind {
            ParamKind::SelfParam => {
                if self_ty.is_none() {
                    diags.emit(
                        Diagnostic::error("E0203", "`self` parameter outside of an `impl` block")
                            .primary(p.span, "no type to be a method of")
                            .help("methods are declared inside `impl Type { ... }`"),
                    );
                    continue;
                }
                if i != 0 {
                    diags.emit(Diagnostic::error("E0203", "`self` must be the first parameter").primary(p.span, "misplaced `self`"));
                    continue;
                }
                has_self = true;
                params.push(ParamDef { name: "self".into(), conv: p.conv, ty: self_ty.unwrap(), span: p.span });
            }
            ParamKind::Named { name, ty } => {
                if params.iter().any(|q: &ParamDef| q.name == name.name) {
                    diags.emit(Diagnostic::error("E0201", format!("parameter `{}` is declared twice", name.name)).primary(name.span, "duplicate parameter"));
                }
                let mut t = resolve_type(prog, &scope, ty, diags);
                // A borrowed parameter of function type accepts anonymous functions
                // using their caller's variables (spec §4.12); exported functions keep
                // plain C function pointers.
                if p.conv == Convention::Borrow && prog.fns[id.idx()].export.is_none() {
                    if let TyKind::Fn(ps, r) = &*prog.tcx.kind(t) {
                        t = prog.tcx.mk(TyKind::Closure(ps.clone(), *r));
                    }
                }
                params.push(ParamDef { name: name.name.clone(), conv: p.conv, ty: t, span: name.span });
            }
        }
    }
    let ret = match &decl.ret {
        Some(t) => resolve_type(prog, &scope, t, diags),
        None => prog.tcx.unit,
    };
    let is_std = prog.modules[module.idx()].is_std;
    let intrinsic = attrs.iter().any(|a| a.name.name == "intrinsic");
    if decl.body.is_none() && !intrinsic && !prog.fns[id.idx()].is_trait_decl() {
        diags.emit(
            Diagnostic::error("E0103", format!("function `{}` has no body", decl.name.name))
                .primary(decl.name.span, "missing `{ ... }`")
                .help("to declare a C function, write `extern \"C\" fn ...`"),
        );
    }
    if decl.body.is_some() && intrinsic && is_std {
        diags.emit(Diagnostic::error("E0103", "an `@intrinsic` function cannot have a body").primary(decl.name.span, "unexpected body"));
    }
    let is_test = attrs.iter().any(|a| a.name.name == "test");
    if is_test && (!params.is_empty() || !generics.is_empty() || ret != prog.tcx.unit || self_ty.is_some()) {
        diags.emit(
            Diagnostic::error("E0233", "a `@test` function must be a free function without parameters, generics or return value")
                .primary(decl.name.span, "invalid test function"),
        );
    }
    if let Some(sym) = &prog.fns[id.idx()].export {
        if !generics.is_empty() || self_ty.is_some() {
            diags.emit(
                Diagnostic::error("E0504", "an `@export` function must be a free function without generic parameters")
                    .primary(decl.name.span, "cannot be exported"),
            );
        }
        if sym.is_empty() || !sym.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '.') {
            diags.emit(Diagnostic::error("E0504", format!("invalid exported symbol name `{}`", sym)).primary(decl.name.span, "letters, digits and `_` only"));
        }
        for p in &params {
            if p.conv != Convention::Borrow && p.conv != Convention::Owned || !is_ffi_safe(prog, p.ty, false) {
                diags.emit(
                    Diagnostic::error("E0503", format!("parameter `{}` of an exported function cannot cross the C boundary", p.name))
                        .primary(p.span, format!("type `{}`", prog.ty_str(p.ty)))
                        .note("allowed: integers, floats, `bool`, `char`, `Ptr[T]` and `fn` types, passed by value (see spec §8.2)"),
                );
            }
        }
        if !is_ffi_safe(prog, ret, true) {
            diags.emit(
                Diagnostic::error("E0503", format!("type `{}` cannot cross the C boundary", prog.ty_str(ret)))
                    .primary(decl.ret.as_ref().map(|r| r.span).unwrap_or(decl.name.span), "not FFI-safe"),
            );
        }
    }
    if prog.tcx.has_closure(ret) {
        diags.emit(
            Diagnostic::error("E0248", "a function cannot return an anonymous function")
                .primary(decl.ret.as_ref().map(|r| r.span).unwrap_or(decl.name.span), "closure type")
                .note("anonymous functions only live during the call that receives them"),
        );
    }
    let f = &mut prog.fns[id.idx()];
    f.generics = generics;
    f.n_owner_generics = n_owner;
    f.params = params;
    f.has_self = has_self;
    f.ret = ret;
    f.self_ty = self_ty;
    f.bounds = bounds;
    f.trait_id = owner.as_ref().and_then(|o| o.trait_id);
}

/// Resolves a trait name (`Display`, `module.Trait`).
pub fn resolve_trait_ref(prog: &Program, module: ModId, t: &TypeExpr, diags: &DiagSink) -> Option<TraitId> {
    let TypeExprKind::Path { segments, args } = &t.kind else {
        diags.emit(Diagnostic::error("E0240", "expected a trait name").primary(t.span, "not a trait"));
        return None;
    };
    if !args.is_empty() {
        diags.emit(Diagnostic::error("E0240", "traits take no generic arguments").primary(t.span, "remove `[...]`"));
    }
    let mut item = match lookup_item(prog, module, &segments[0].name) {
        Some(it) => it,
        None => {
            let mut d = Diagnostic::error("E0240", format!("cannot find trait `{}`", segments[0].name)).primary(segments[0].span, "not found in this scope");
            let cands: Vec<String> = prog.traits.iter().map(|t| t.name.clone()).collect();
            if let Some(s) = best_match(&segments[0].name, cands.iter().map(|s| s.as_str())) {
                d = d.help(format!("did you mean `{}`?", s));
            }
            diags.emit(d);
            return None;
        }
    };
    for seg in &segments[1..] {
        match item {
            ItemRef::Module(m) => item = lookup_in_module(prog, module, m, &seg.name, seg.span, diags)?,
            _ => {
                diags.emit(Diagnostic::error("E0240", format!("`{}` is not a module", seg.name)).primary(seg.span, "expected a module"));
                return None;
            }
        }
    }
    match item {
        ItemRef::Trait(tr) => Some(tr),
        _ => {
            diags.emit(Diagnostic::error("E0240", format!("`{}` is not a trait", segments.last().unwrap().name)).primary(t.span, "expected a trait"));
            None
        }
    }
}

/// `trait Name: Super { ... }`: supertraits and method declarations.
fn collect_trait(prog: &mut Program, id: TraitId, decl: &ast::TraitDecl, diags: &DiagSink) {
    let module = prog.traits[id.idx()].module;
    let is_pub = prog.traits[id.idx()].is_pub;
    let mut supers = Vec::new();
    for s in &decl.supertraits {
        if let Some(tr) = resolve_trait_ref(prog, module, s, diags) {
            if tr == id {
                diags.emit(Diagnostic::error("E0240", format!("trait `{}` cannot be its own supertrait", decl.name.name)).primary(s.span, "cyclic trait"));
            } else if !supers.contains(&tr) {
                supers.push(tr);
            }
        }
    }
    prog.traits[id.idx()].supertraits = supers;
    let self_ty = prog.tcx.param(0);
    for item in &decl.items {
        let ItemKind::Fn(f) = &item.kind else { continue };
        if prog.traits[id.idx()].method(&f.name.name).is_some() {
            diags.emit(Diagnostic::error("E0201", format!("trait `{}` already declares `{}`", decl.name.name, f.name.name)).primary(f.name.span, "duplicate method"));
            continue;
        }
        let f = Rc::new(f.clone());
        let owner = || SigOwner { generics: vec!["Self".to_string()], self_ty: Some(self_ty), bounds: vec![Bound { param: 0, trait_id: id }], trait_id: Some(id) };
        let decl_id = FnId(prog.fns.len() as u32);
        let mut def = new_fn_def(&f, module, is_pub, None, &item.attrs);
        def.kind = FnKind::TraitDecl(id);
        prog.fns.push(def);
        resolve_fn_sig(prog, decl_id, &f, &item.attrs, Some(owner()), diags);
        let default = if f.body.is_some() {
            let did = FnId(prog.fns.len() as u32);
            prog.fns.push(new_fn_def(&f, module, is_pub, None, &item.attrs));
            resolve_fn_sig(prog, did, &f, &item.attrs, Some(owner()), diags);
            Some(did)
        } else {
            None
        };
        prog.traits[id.idx()].methods.push(TraitMethod { name: f.name.name.clone(), decl: decl_id, default });
    }
}

/// `impl Trait for Type { ... }`
fn collect_trait_impl(prog: &mut Program, imp: PendingImpl, diags: &DiagSink) {
    let trait_expr = imp.decl.trait_.clone().unwrap();
    let Some(tr) = resolve_trait_ref(prog, imp.module, &trait_expr, diags) else { return };
    let TypeExprKind::Path { segments, args } = &imp.decl.ty.kind else { return };
    // The implementing type: a primitive type or a struct/enum, generic parameters
    // listed (optionally) with their bounds.
    let head_name = segments.last().unwrap();
    let (adt, generics, self_ty) = if segments.len() == 1 && primitive_type(prog, &head_name.name).is_some() {
        (None, Vec::new(), primitive_type(prog, &head_name.name).unwrap())
    } else {
        // The type's name, without its generic arguments.
        let mut item = lookup_item(prog, imp.module, &segments[0].name);
        for seg in &segments[1..] {
            item = match item {
                Some(ItemRef::Module(m)) => lookup_in_module(prog, imp.module, m, &seg.name, seg.span, diags),
                _ => None,
            };
        }
        let a = match item {
            Some(ItemRef::Adt(a)) => a,
            Some(ItemRef::Alias(al)) if prog.tcx.as_adt(prog.aliases[al.idx()].ty).is_some() => prog.tcx.as_adt(prog.aliases[al.idx()].ty).unwrap().0,
            None => {
                diags.emit(Diagnostic::error("E0202", format!("cannot find type `{}`", head_name.name)).primary(head_name.span, "not found"));
                return;
            }
            _ => {
                diags.emit(
                    Diagnostic::error("E0224", "a trait can be implemented for a struct, an enum or a primitive type")
                        .primary(imp.decl.ty.span, "not a struct, enum or primitive type"),
                );
                return;
            }
        };
        let adt_generics = prog.adts[a.idx()].generics.clone();
        let generics: Vec<String> = if args.is_empty() {
            adt_generics.clone()
        } else {
            if args.len() != adt_generics.len() {
                diags.emit(
                    Diagnostic::error("E0315", format!("`{}` takes {} generic argument(s)", prog.adts[a.idx()].name, adt_generics.len()))
                        .primary(imp.decl.ty.span, format!("{} given", args.len())),
                );
                return;
            }
            args.iter()
                .map(|t| match &t.kind {
                    TypeExprKind::Path { segments, .. } => segments[0].name.clone(),
                    _ => "_".to_string(),
                })
                .collect()
        };
        let self_ty = prog.tcx.adt(a, (0..generics.len() as u32).map(|i| prog.tcx.param(i)).collect());
        (Some(a), generics, self_ty)
    };
    let tdef_module = prog.traits[tr.idx()].module;
    let type_module = adt.map(|a| prog.adts[a.idx()].module);
    if imp.module != tdef_module && Some(imp.module) != type_module {
        diags.emit(
            Diagnostic::error("E0247", format!("`impl {} for {}` must be in the module of the trait or of the type", prog.traits[tr.idx()].name, prog.ty_str(self_ty)))
                .primary(imp.span, "implementation in an unrelated module")
                .note("this rule guarantees that a type implements a trait at most once in a program"),
        );
        return;
    }
    if prog.find_impl(tr, self_ty).is_some() {
        diags.emit(
            Diagnostic::error("E0243", format!("`{}` is implemented twice for `{}`", prog.traits[tr.idx()].name, prog.ty_str_with(self_ty, &generics)))
                .primary(imp.span, "second implementation"),
        );
        return;
    }
    let mut bounds = Vec::new();
    for b in &imp.decl.bounds {
        let Some(i) = generics.iter().position(|g| *g == b.param.name) else { continue };
        for t in &b.traits {
            if let Some(btr) = resolve_trait_ref(prog, imp.module, t, diags) {
                bounds.push(Bound { param: i as u32, trait_id: btr });
            }
        }
    }
    let impl_id = ImplId(prog.impls.len() as u32);
    prog.impls.push(ImplDef { trait_id: tr, module: imp.module, span: imp.span, self_ty, generics: generics.clone(), bounds: bounds.clone(), methods: HashMap::new() });
    let tname = prog.traits[tr.idx()].name.clone();
    let k = generics.len() as u32;
    for item in imp.decl.items {
        let ItemKind::Fn(f) = item.kind else { continue };
        check_attrs(&ast::Item { attrs: item.attrs.clone(), is_pub: item.is_pub, kind: ItemKind::Fn(f.clone()), span: item.span }, prog.modules[imp.module.idx()].is_std, diags);
        let Some(decl_fn) = prog.traits[tr.idx()].method(&f.name.name).map(|m| m.decl) else {
            let mut d = Diagnostic::error("E0241", format!("`{}` is not a method of trait `{}`", f.name.name, tname)).primary(f.name.span, "not declared by the trait");
            let cands: Vec<String> = prog.traits[tr.idx()].methods.iter().map(|m| m.name.clone()).collect();
            if let Some(s) = best_match(&f.name.name, cands.iter().map(|s| s.as_str())) {
                d = d.help(format!("did you mean `{}`?", s));
            }
            diags.emit(d);
            continue;
        };
        if prog.impls[impl_id.idx()].methods.contains_key(&f.name.name) {
            diags.emit(Diagnostic::error("E0201", format!("`{}` is implemented twice", f.name.name)).primary(f.name.span, "duplicate method"));
            continue;
        }
        let f = Rc::new(f);
        let id = FnId(prog.fns.len() as u32);
        prog.fns.push(new_fn_def(&f, imp.module, true, adt, &item.attrs));
        let owner = SigOwner { generics: generics.clone(), self_ty: Some(self_ty), bounds: bounds.clone(), trait_id: Some(tr) };
        resolve_fn_sig(prog, id, &f, &item.attrs, Some(owner), diags);
        // The signature must be the trait's, with `Self` replaced by the type.
        let (dparams, dret, dgen, dself): (Vec<(Convention, Ty)>, Ty, usize, bool) = {
            let d = prog.func(decl_fn);
            (d.params.iter().map(|p| (p.conv, p.ty)).collect(), d.ret, d.generics.len() - 1, d.has_self)
        };
        let mut subst = vec![self_ty];
        subst.extend((0..dgen as u32).map(|i| prog.tcx.param(k + i)));
        let fd = prog.func(id);
        let same = fd.has_self == dself
            && fd.generics.len() - fd.n_owner_generics == dgen
            && fd.params.len() == dparams.len()
            && fd.params.iter().zip(dparams.iter()).all(|(p, (c, t))| p.conv == *c && p.ty == prog.tcx.subst(*t, &subst))
            && fd.ret == prog.tcx.subst(dret, &subst);
        if !same {
            let names: Vec<String> = std::iter::once("Self".to_string()).chain((0..dgen).map(|i| format!("T{}", i))).collect();
            let d = prog.func(decl_fn);
            let sig: Vec<String> = d
                .params
                .iter()
                .map(|p| {
                    let t = if p.name == "self" { "self".to_string() } else { format!("{}: {}", p.name, prog.ty_str_with(p.ty, &names)) };
                    if p.conv == Convention::Borrow { t } else { format!("{} {}", p.conv.keyword(), t) }
                })
                .collect();
            let ret = if d.ret == prog.tcx.unit { String::new() } else { format!(" -> {}", prog.ty_str_with(d.ret, &names)) };
            diags.emit(
                Diagnostic::error("E0241", format!("`{}` does not match its declaration in trait `{}`", f.name.name, tname))
                    .primary(f.name.span, "different signature")
                    .secondary(d.span, "declared here")
                    .note(format!("expected `fn {}({}){}` with `Self` = `{}`", f.name.name, sig.join(", "), ret, prog.ty_str_with(self_ty, &generics))),
            );
        }
        let abi: Vec<Ty> = prog.func(decl_fn).params.iter().map(|p| p.ty).collect();
        if abi.len() == prog.func(id).params.len() {
            prog.fns[id.idx()].abi_params = Some(abi);
        }
        prog.impls[impl_id.idx()].methods.insert(f.name.name.clone(), id);
    }
}

/// Every trait method without a default must be implemented, and the supertraits
/// must be implemented for the same type.
fn check_impls(prog: &Program, diags: &DiagSink) {
    let copy = prog.lang_trait("copy");
    for imp in &prog.impls {
        // A declaration of `Copy` is checked against the structure of the type (ADR 0012).
        if Some(imp.trait_id) == copy {
            if let TyKind::Adt(id, _) = &*prog.tcx.kind(imp.self_ty) {
                if let Some(why) = prog.copy_obstacle(*id) {
                    let a = prog.adt(*id);
                    diags.emit(
                        Diagnostic::error("E0250", format!("`{}` cannot be `Copy`", a.name))
                            .primary(a.span, "declared `Copy`")
                            .note(why)
                            .help("remove the declaration, or keep only copyable values in the type (`.clone()` copies the others explicitly)"),
                    );
                }
            }
        }
        let tdef = &prog.traits[imp.trait_id.idx()];
        let missing: Vec<&str> = tdef.methods.iter().filter(|m| m.default.is_none() && !imp.methods.contains_key(&m.name)).map(|m| m.name.as_str()).collect();
        if !missing.is_empty() {
            diags.emit(
                Diagnostic::error("E0242", format!("missing method{} of trait `{}`: {}", if missing.len() == 1 { "" } else { "s" }, tdef.name, missing.iter().map(|m| format!("`{}`", m)).collect::<Vec<_>>().join(", ")))
                    .primary(imp.span, format!("`{}` is not fully implemented", tdef.name))
                    .help("add the missing functions to this `impl` block"),
            );
        }
        for &s in &tdef.supertraits {
            if prog.find_impl(s, imp.self_ty).is_none() {
                diags.emit(
                    Diagnostic::error("E0244", format!("`{}` requires `{}`, which `{}` does not implement", tdef.name, prog.traits[s.idx()].name, prog.ty_str_with(imp.self_ty, &imp.generics)))
                        .primary(imp.span, format!("`{}` is a supertrait of `{}`", prog.traits[s.idx()].name, tdef.name))
                        .help(format!("add `impl {} for {}`", prog.traits[s.idx()].name, prog.ty_str_with(imp.self_ty, &imp.generics))),
                );
            }
        }
    }
}

fn collect_impl(prog: &mut Program, imp: PendingImpl, diags: &DiagSink) {
    // The target must be a type declared in the same module (spec §6.3).
    let target = match &imp.decl.ty.kind {
        TypeExprKind::Path { segments, args } if segments.len() == 1 => {
            if !args.is_empty() {
                diags.emit(
                    Diagnostic::error("E0224", "an `impl` block applies to the whole generic type")
                        .primary(imp.decl.ty.span, "remove the generic arguments")
                        .note("inside the block, the type's parameters are in scope automatically"),
                );
            }
            if primitive_type(prog, &segments[0].name).is_some() || segments[0].name == "Ptr" {
                diags.emit(
                    Diagnostic::error("E0224", format!("cannot add methods to the built-in type `{}`", segments[0].name))
                        .primary(imp.decl.ty.span, "built-in type")
                        .note("the methods of built-in types are provided by the language")
                        .help("write a free function instead, for example `fn double(x: i32) -> i32`"),
                );
                return;
            }
            match lookup_item(prog, imp.module, &segments[0].name) {
                Some(ItemRef::Adt(a)) => Some(a),
                Some(_) => None,
                None => {
                    diags.emit(Diagnostic::error("E0202", format!("cannot find type `{}`", segments[0].name)).primary(segments[0].span, "not found"));
                    return;
                }
            }
        }
        // `impl module.Type`: resolve the type to report where it is declared.
        TypeExprKind::Path { .. } => {
            let scope = TypeScope { module: imp.module, generics: &[], self_ty: None };
            let unit_args = |t: &TypeExpr| -> TypeExpr {
                match &t.kind {
                    TypeExprKind::Path { segments, .. } => TypeExpr { kind: TypeExprKind::Path { segments: segments.clone(), args: vec![] }, span: t.span },
                    _ => t.clone(),
                }
            };
            let quiet = DiagSink::new();
            let ty = resolve_type(prog, &scope, &unit_args(&imp.decl.ty), &quiet);
            match prog.tcx.as_adt(ty) {
                Some((a, _)) => Some(a),
                None => {
                    for d in quiet.take_sorted() {
                        diags.emit(d);
                    }
                    return;
                }
            }
        }
        _ => None,
    };
    let Some(adt_id) = target else {
        diags.emit(
            Diagnostic::error("E0224", "`impl` blocks can only extend structs and enums declared in the same module")
                .primary(imp.decl.ty.span, "not a struct or enum name"),
        );
        return;
    };
    if prog.adts[adt_id.idx()].module != imp.module {
        diags.emit(
            Diagnostic::error("E0224", format!("`impl {}` must be in the module that declares `{}`", prog.adts[adt_id.idx()].name, prog.adts[adt_id.idx()].name))
                .primary(imp.span, "impl outside of the type's module")
                .secondary(prog.adts[adt_id.idx()].span, "type declared here"),
        );
        return;
    }
    let owner_generics = prog.adts[adt_id.idx()].generics.clone();
    for item in imp.decl.items {
        let ItemKind::Fn(f) = item.kind else { continue };
        check_attrs(&ast::Item { attrs: item.attrs.clone(), is_pub: item.is_pub, kind: ItemKind::Fn(f.clone()), span: item.span }, prog.modules[imp.module.idx()].is_std, diags);
        let f = Rc::new(f);
        let id = FnId(prog.fns.len() as u32);
        prog.fns.push(new_fn_def(&f, imp.module, item.is_pub, Some(adt_id), &item.attrs));
        let self_ty = prog.tcx.adt(adt_id, (0..owner_generics.len() as u32).map(|i| prog.tcx.param(i)).collect());
        let owner = SigOwner { generics: owner_generics.clone(), self_ty: Some(self_ty), bounds: vec![], trait_id: None };
        resolve_fn_sig(prog, id, &f, &item.attrs, Some(owner), diags);
        let name = f.name.name.clone();
        let adt = &prog.adts[adt_id.idx()];
        if adt.methods.contains_key(&name) {
            diags.emit(Diagnostic::error("E0201", format!("`{}` already has a method named `{}`", adt.name, name)).primary(f.name.span, "duplicate method"));
            continue;
        }
        // A private field and a public method may share a name (`len` and `len()`):
        // outside the module only the method is visible.
        if adt.fields().iter().any(|fd| fd.name == name && fd.is_pub) {
            diags.emit(
                Diagnostic::error("E0212", format!("`{}` has both a field and a method named `{}`", adt.name, name))
                    .primary(f.name.span, "method declared here")
                    .help("rename the method or the field"),
            );
        }
        if name == "drop" {
            let fd = &prog.fns[id.idx()];
            let ok = fd.has_self && fd.params.len() == 1 && fd.params[0].conv == Convention::Inout && fd.generics.len() == fd.n_owner_generics && fd.ret == prog.tcx.unit;
            if !ok {
                diags.emit(
                    Diagnostic::error("E0432", "a destructor must be declared `fn drop(inout self)`")
                        .primary(f.name.span, "invalid destructor signature"),
                );
            } else {
                prog.adts[adt_id.idx()].drop_fn = Some(id);
            }
        }
        if item.attrs.iter().any(|a| a.name.name == "index") {
            prog.adts[adt_id.idx()].index_fn = Some(id);
        }
        prog.adts[adt_id.idx()].methods.insert(name, id);
    }
}

/// A value type that contains itself without `indirect` storage would be infinitely
/// large (spec §3.2, E0311).
fn check_recursive_types(prog: &Program, diags: &DiagSink) {
    for (i, adt) in prog.adts.iter().enumerate() {
        let id = AdtId(i as u32);
        let mut path: Vec<String> = Vec::new();
        let mut seen: Vec<AdtId> = Vec::new();
        let direct: Vec<(Ty, String)> = match &adt.kind {
            AdtKind::Struct { fields } => fields.iter().filter(|f| !f.indirect).map(|f| (f.ty, format!("field `{}`", f.name))).collect(),
            AdtKind::Enum { variants } => variants
                .iter()
                .filter(|v| !v.indirect)
                .flat_map(|v| v.fields.iter().map(move |&t| (t, format!("variant `{}`", v.name))))
                .collect(),
        };
        for (t, what) in direct {
            path.clear();
            seen.clear();
            if contains_directly(prog, t, id, &mut seen) {
                let help = if adt.is_enum() {
                    format!("mark the variant `indirect`, for example `indirect {}`", what.trim_start_matches("variant `").trim_end_matches('`'))
                } else {
                    format!("mark the field `indirect`, for example `indirect {}`", what.trim_start_matches("field `").trim_end_matches('`'))
                };
                diags.emit(
                    Diagnostic::error("E0311", format!("recursive type `{}` has infinite size", adt.name))
                        .primary(adt.span, format!("`{}` contains itself through its {}", adt.name, what))
                        .help(help),
                );
                break;
            }
        }
    }
}

fn contains_directly(prog: &Program, t: Ty, target: AdtId, seen: &mut Vec<AdtId>) -> bool {
    match &*prog.tcx.kind(t) {
        TyKind::Adt(id, args) => {
            if *id == target {
                return true;
            }
            if args.iter().any(|&a| contains_directly(prog, a, target, seen)) {
                // e.g. `Option[Self]` stores the payload inline.
                let adt = &prog.adts[id.idx()];
                let inline = match &adt.kind {
                    AdtKind::Struct { fields } => fields.iter().any(|f| !f.indirect && prog.tcx.has_param(f.ty)),
                    AdtKind::Enum { variants } => variants.iter().any(|v| !v.indirect && v.fields.iter().any(|&f| prog.tcx.has_param(f))),
                };
                if inline && !is_heap_container(prog, *id) {
                    return true;
                }
            }
            if seen.contains(id) {
                return false;
            }
            seen.push(*id);
            let adt = &prog.adts[id.idx()];
            let r = match &adt.kind {
                AdtKind::Struct { fields } => fields.iter().any(|f| !f.indirect && contains_directly(prog, prog.tcx.subst(f.ty, args), target, seen)),
                AdtKind::Enum { variants } => {
                    variants.iter().any(|v| !v.indirect && v.fields.iter().any(|&f| contains_directly(prog, prog.tcx.subst(f, args), target, seen)))
                }
            };
            r
        }
        TyKind::Tuple(elems) => elems.iter().any(|&e| contains_directly(prog, e, target, seen)),
        TyKind::Array(e, n) => *n > 0 && contains_directly(prog, *e, target, seen),
        _ => false,
    }
}

/// Types that store their parameters behind a pointer (`Vec[T]`).
fn is_heap_container(prog: &Program, id: AdtId) -> bool {
    Some(id) == prog.lang.vec
}

fn check_lang_items(prog: &Program, diags: &DiagSink) {
    let missing: Vec<&str> = [("String", prog.lang.string), ("Vec", prog.lang.vec), ("Option", prog.lang.option), ("Result", prog.lang.result)]
        .iter()
        .filter(|(_, id)| id.is_none())
        .map(|(n, _)| *n)
        .collect();
    if !missing.is_empty() {
        diags.emit(Diagnostic::error("E0900", format!("standard library is incomplete: missing {}", missing.join(", "))));
    }
}

fn find_main(prog: &mut Program, diags: &DiagSink) {
    let root = prog.root;
    match prog.modules[root.idx()].items.get("main").copied() {
        Some((ItemRef::Fn(f), span, _)) => {
            let fd = &prog.fns[f.idx()];
            if !fd.params.is_empty() || !fd.generics.is_empty() || fd.ret != prog.tcx.unit || fd.is_extern() {
                diags.emit(
                    Diagnostic::error("E0230", "`main` must be declared `fn main()`")
                        .primary(span, "invalid signature")
                        .help("use `std.system.args()` for arguments and `std.system.exit(code)` for the exit code"),
                );
            }
            prog.main_fn = Some(f);
        }
        Some((_, span, _)) => {
            diags.emit(Diagnostic::error("E0230", "`main` must be a function").primary(span, "not a function"));
        }
        None => {}
    }
}

//! Pattern checking (spec §2.6 and §4.7).

use super::FnCtxt;
use crate::collect::{lookup_in_module, lookup_item};
use crate::defs::{AdtId, AdtKind, ConstValue, ItemRef};
use crate::thir::{LocalKind, Pat, PatKind as P};
use crate::ty::{Ty, TyKind};
use crate::util::best_match;
use nxc_syntax::ast::{self, PatArg, PatKind as A, PatLit};
use nxc_syntax::{Diagnostic, Span};

/// How pattern bindings hold their value (spec §5.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BindMode {
    /// The scrutinee is a value: bindings own the parts they designate.
    Owned,
    /// The scrutinee is a place: bindings are read-only aliases.
    Ref,
    /// `match &place`: bindings are modifiable aliases.
    RefMut,
}

enum CtorTarget {
    Variant(AdtId, u32),
    Struct(AdtId),
    Const(ConstValue, Ty),
}

impl<'a> FnCtxt<'a> {
    fn err_pat(&self, ty: Ty, span: Span) -> Pat {
        Pat { kind: P::Error, ty, span }
    }

    /// After an error in a pattern, still declares the names it binds (with the error
    /// type) so that their uses do not cause further errors.
    fn declare_error_bindings(&mut self, p: &ast::Pattern, names: &mut Vec<(String, Span)>) {
        let tcx = self.tcx();
        match &p.kind {
            A::Ident(name) => {
                let is_item = matches!(lookup_item(self.p, self.module, name), Some(ItemRef::Variant(..)) | Some(ItemRef::Const(_)));
                if !is_item && !names.iter().any(|(n, _)| n == name) && !name.chars().next().map_or(false, |c| c.is_ascii_uppercase()) {
                    names.push((name.clone(), p.span));
                    self.declare_local(name, tcx.error, LocalKind::Let, p.span);
                }
            }
            A::Tuple(ps) | A::Or(ps) => ps.iter().for_each(|q| self.declare_error_bindings(q, names)),
            A::Ctor { args, .. } => {
                for a in args {
                    match a {
                        PatArg::Positional(q) | PatArg::Named(_, q) => self.declare_error_bindings(q, names),
                        PatArg::Rest(_) => {}
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn check_pat(&mut self, p: &ast::Pattern, expected: Ty, mode: BindMode) -> Pat {
        let mut names: Vec<(String, Span)> = Vec::new();
        self.check_pat_inner(p, expected, mode, &mut names, false)
    }

    fn check_pat_inner(&mut self, p: &ast::Pattern, expected: Ty, mode: BindMode, names: &mut Vec<(String, Span)>, in_or: bool) -> Pat {
        let tcx = self.tcx();
        let span = p.span;
        let exp = self.shallow(expected);
        match &p.kind {
            A::Error => self.err_pat(expected, span),
            A::Wild => Pat { kind: P::Wild, ty: expected, span },
            A::Ident(name) => {
                // 1. A fieldless variant of the expected enum.
                if let Some((id, args)) = tcx.as_adt(exp) {
                    let adt = self.p.adt(id);
                    if let Some(v) = adt.variant_index(name) {
                        let variant = &adt.variants()[v as usize];
                        if !variant.fields.is_empty() {
                            self.err(
                                Diagnostic::error("E0345", format!("variant `{}` has {} value(s)", name, variant.fields.len()))
                                    .primary(span, "missing sub-patterns")
                                    .help(format!("write `{}({})`", name, vec!["_"; variant.fields.len()].join(", "))),
                            );
                            return self.err_pat(expected, span);
                        }
                        return Pat { kind: P::Variant { adt: id, variant: v, type_args: args, fields: vec![] }, ty: expected, span };
                    }
                }
                // 2. A variant or constant in scope.
                if self.lookup_local(name).is_none() {
                    match lookup_item(self.p, self.module, name) {
                        Some(ItemRef::Variant(id, v)) => return self.variant_pat(id, v, &[], expected, mode, names, span, in_or),
                        Some(ItemRef::Const(c)) => return self.const_pat(c, expected, span),
                        _ => {}
                    }
                }
                // 3. A new binding.
                if in_or {
                    self.err(
                        Diagnostic::error("E0341", "alternatives in a pattern cannot bind variables")
                            .primary(span, format!("`{}` would be bound here", name))
                            .help("use one arm per alternative, or `_`"),
                    );
                    if !names.iter().any(|(n, _)| n == name) {
                        names.push((name.clone(), span));
                        self.declare_local(name, tcx.error, LocalKind::Let, span);
                    }
                    return self.err_pat(expected, span);
                }
                if let Some((_, prev)) = names.iter().find(|(n, _)| n == name) {
                    self.err(
                        Diagnostic::error("E0344", format!("`{}` is bound twice in the same pattern", name))
                            .primary(span, "second binding")
                            .secondary(*prev, "first binding"),
                    );
                    return self.err_pat(expected, span);
                }
                names.push((name.clone(), span));
                if name.chars().next().map_or(false, |c| c.is_ascii_uppercase()) {
                    self.err(
                        Diagnostic::warning("W0007", format!("`{}` is a new variable that matches anything", name))
                            .primary(span, "this is a binding, not a variant")
                            .help("variant names must exist in the matched type; check the spelling, or use a lowercase name for a binding"),
                    );
                }
                let kind = match mode {
                    BindMode::Owned => LocalKind::Let,
                    BindMode::Ref => LocalKind::Ref,
                    BindMode::RefMut => LocalKind::RefMut,
                };
                let l = self.declare_local(name, expected, kind, span);
                Pat { kind: P::Binding(l), ty: expected, span }
            }
            A::Lit(lit) => {
                let (v, ty) = self.pat_lit(lit, exp, span);
                if !self.unify(ty, expected) {
                    self.mismatch(span, expected, ty);
                    return self.err_pat(expected, span);
                }
                Pat { kind: P::Lit(v), ty: expected, span }
            }
            A::Range(lo, hi, inclusive) => {
                let (a, ta) = self.pat_lit(lo, exp, span);
                let (b, tb) = self.pat_lit(hi, exp, span);
                let ok_kind = matches!((&a, &b), (ConstValue::Int(_), ConstValue::Int(_)) | (ConstValue::Char(_), ConstValue::Char(_)));
                if !ok_kind {
                    self.err(Diagnostic::error("E0346", "range patterns need integer or character bounds").primary(span, "invalid range"));
                    return self.err_pat(expected, span);
                }
                if !self.unify(ta, expected) || !self.unify(tb, expected) {
                    self.mismatch(span, expected, ta);
                    return self.err_pat(expected, span);
                }
                let empty = match (&a, &b) {
                    (ConstValue::Int(x), ConstValue::Int(y)) => if *inclusive { x > y } else { x >= y },
                    (ConstValue::Char(x), ConstValue::Char(y)) => if *inclusive { x > y } else { x >= y },
                    _ => false,
                };
                if empty {
                    self.err(Diagnostic::error("E0346", "this range pattern is empty").primary(span, "matches no value"));
                }
                Pat { kind: P::Range(a, b, *inclusive), ty: expected, span }
            }
            A::Tuple(elems) => {
                let elem_tys: Vec<Ty> = match &*tcx.kind(exp) {
                    TyKind::Tuple(ts) if ts.len() == elems.len() => ts.clone(),
                    TyKind::Unit if elems.is_empty() => vec![],
                    TyKind::Infer(_) => {
                        let vs: Vec<Ty> = elems.iter().map(|e| self.new_var(e.span)).collect();
                        let t = tcx.tuple(vs.clone());
                        self.unify(exp, t);
                        vs
                    }
                    TyKind::Error => vec![tcx.error; elems.len()],
                    _ => {
                        self.err(
                            Diagnostic::error("E0302", "mismatched types")
                                .primary(span, format!("expected `{}`, found a tuple of {} elements", self.ty_str(exp), elems.len())),
                        );
                        return self.err_pat(expected, span);
                    }
                };
                if elems.is_empty() {
                    return Pat { kind: P::Wild, ty: expected, span };
                }
                let ps = elems.iter().zip(elem_tys).map(|(e, t)| self.check_pat_inner(e, t, mode, names, in_or)).collect();
                Pat { kind: P::Tuple(ps), ty: expected, span }
            }
            A::Or(alts) => {
                let ps: Vec<Pat> = alts.iter().map(|a| self.check_pat_inner(a, expected, mode, names, true)).collect();
                Pat { kind: P::Or(ps), ty: expected, span }
            }
            A::Path(path) => match self.resolve_ctor_path(path, exp) {
                Some(CtorTarget::Variant(id, v)) => self.variant_pat(id, v, &[], expected, mode, names, span, in_or),
                Some(CtorTarget::Const(cv, ct)) => {
                    if !self.unify(ct, expected) {
                        self.mismatch(span, expected, ct);
                    }
                    Pat { kind: P::Lit(cv), ty: expected, span }
                }
                Some(CtorTarget::Struct(id)) => {
                    let name = self.p.adt(id).name.clone();
                    self.err(Diagnostic::error("E0345", format!("struct `{}` needs field patterns", name)).primary(span, format!("write `{}(field: pattern, ..)`", name)));
                    self.err_pat(expected, span)
                }
                None => self.err_pat(expected, span),
            },
            A::Ctor { path, args } => match self.resolve_ctor_path(path, exp) {
                Some(CtorTarget::Variant(id, v)) => self.variant_pat(id, v, args, expected, mode, names, span, in_or),
                Some(CtorTarget::Struct(id)) => self.struct_pat(id, args, expected, mode, names, span, in_or),
                Some(CtorTarget::Const(..)) => {
                    self.err(Diagnostic::error("E0345", "a constant pattern takes no arguments").primary(span, "remove the parentheses"));
                    self.err_pat(expected, span)
                }
                None => self.err_pat(expected, span),
            },
        }
    }

    fn const_pat(&mut self, c: crate::defs::ConstId, expected: Ty, span: Span) -> Pat {
        let ct = self.p.consts[c.idx()].ty;
        let Some(v) = self.p.const_values.get(&c).cloned() else {
            return self.err_pat(expected, span);
        };
        if !matches!(v, ConstValue::Int(_) | ConstValue::Char(_) | ConstValue::Bool(_) | ConstValue::Str(_)) {
            self.err(Diagnostic::error("E0346", "only integer, character, boolean and string constants can be used in patterns").primary(span, "unsupported constant"));
            return self.err_pat(expected, span);
        }
        if !self.unify(ct, expected) {
            self.mismatch(span, expected, ct);
            return self.err_pat(expected, span);
        }
        Pat { kind: P::Lit(v), ty: expected, span }
    }

    fn pat_lit(&mut self, lit: &PatLit, exp: Ty, span: Span) -> (ConstValue, Ty) {
        let tcx = self.tcx();
        match lit {
            PatLit::Int(v) => {
                let ty = match &*tcx.kind(exp) {
                    TyKind::Int(_) | TyKind::IntVar(_) => exp,
                    _ => self.new_var_of(super::VarKind::Int, span),
                };
                (ConstValue::Int(*v), ty)
            }
            PatLit::Char(c) => (ConstValue::Char(*c), tcx.char),
            PatLit::Bool(b) => (ConstValue::Bool(*b), tcx.bool),
            PatLit::Str(s) => (ConstValue::Str(s.clone()), self.p.string_ty()),
        }
    }

    /// Resolves the path of a constructor pattern, preferring variants of the expected type.
    fn resolve_ctor_path(&mut self, path: &[ast::Ident], exp: Ty) -> Option<CtorTarget> {
        let tcx = self.tcx();
        if path.len() == 1 {
            let name = &path[0].name;
            if let Some((id, _)) = tcx.as_adt(exp) {
                if let Some(v) = self.p.adt(id).variant_index(name) {
                    return Some(CtorTarget::Variant(id, v));
                }
            }
            return match lookup_item(self.p, self.module, name) {
                Some(ItemRef::Variant(id, v)) => Some(CtorTarget::Variant(id, v)),
                Some(ItemRef::Adt(id)) if !self.p.adt(id).is_enum() => Some(CtorTarget::Struct(id)),
                Some(ItemRef::Const(c)) => {
                    let v = self.p.const_values.get(&c).cloned()?;
                    Some(CtorTarget::Const(v, self.p.consts[c.idx()].ty))
                }
                _ => {
                    let mut d = Diagnostic::error("E0202", format!("cannot find variant or struct `{}`", name)).primary(path[0].span, "not found");
                    if let Some((id, _)) = tcx.as_adt(exp) {
                        let adt = self.p.adt(id);
                        let vs: Vec<&str> = adt.variants().iter().map(|v| v.name.as_str()).collect();
                        if let Some(s) = best_match(name, vs.iter().copied()) {
                            d = d.help(format!("did you mean `{}`?", s));
                        } else if !vs.is_empty() {
                            d = d.note(format!("variants of `{}`: {}", adt.name, vs.join(", ")));
                        }
                    }
                    self.err(d);
                    None
                }
            };
        }
        // Qualified path: [module...] Type . Variant, or module... . Struct
        let mut item = match lookup_item(self.p, self.module, &path[0].name) {
            Some(it) => it,
            None => {
                self.err(Diagnostic::error("E0202", format!("cannot find `{}`", path[0].name)).primary(path[0].span, "not found"));
                return None;
            }
        };
        for (i, seg) in path.iter().enumerate().skip(1) {
            item = match item {
                ItemRef::Module(m) => lookup_in_module(self.p, self.module, m, &seg.name, seg.span, self.diags)?,
                ItemRef::Adt(id) => {
                    let adt = self.p.adt(id);
                    match adt.variant_index(&seg.name) {
                        Some(v) if i + 1 == path.len() => ItemRef::Variant(id, v),
                        _ => {
                            self.err(Diagnostic::error("E0202", format!("`{}` has no variant `{}`", adt.name, seg.name)).primary(seg.span, "not found"));
                            return None;
                        }
                    }
                }
                _ => {
                    self.err(Diagnostic::error("E0202", format!("`{}` is not a module or type", path[i - 1].name)).primary(path[i - 1].span, "cannot look inside"));
                    return None;
                }
            };
        }
        match item {
            ItemRef::Variant(id, v) => Some(CtorTarget::Variant(id, v)),
            ItemRef::Adt(id) if !self.p.adt(id).is_enum() => Some(CtorTarget::Struct(id)),
            ItemRef::Const(c) => {
                let v = self.p.const_values.get(&c).cloned()?;
                Some(CtorTarget::Const(v, self.p.consts[c.idx()].ty))
            }
            _ => {
                let last = path.last().unwrap();
                self.err(Diagnostic::error("E0345", format!("`{}` is not a variant, struct or constant", last.name)).primary(last.span, "cannot be used as a pattern"));
                None
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn variant_pat(&mut self, id: AdtId, v: u32, args: &[PatArg], expected: Ty, mode: BindMode, names: &mut Vec<(String, Span)>, span: Span, in_or: bool) -> Pat {
        let tcx = self.tcx();
        let adt = self.p.adt(id);
        let targs = self.fresh_args(adt.generics.len(), span);
        let ty = tcx.adt(id, targs.clone());
        if !self.unify(ty, expected) {
            self.mismatch(span, expected, ty);
            return self.err_pat(expected, span);
        }
        let variant = &adt.variants()[v as usize];
        let field_tys: Vec<Ty> = variant.fields.iter().map(|&f| tcx.subst(f, &targs)).collect();
        let mut subs: Vec<&ast::Pattern> = Vec::new();
        let mut rest = false;
        for a in args {
            match a {
                PatArg::Positional(p) => subs.push(p),
                PatArg::Rest(_) => rest = true,
                PatArg::Named(n, _) => {
                    self.err(Diagnostic::error("E0345", "variant values are positional").primary(n.span, "remove this field name"));
                    return self.err_pat(expected, span);
                }
            }
        }
        if subs.len() != field_tys.len() && !(rest && subs.len() <= field_tys.len()) {
            self.err(
                Diagnostic::error("E0345", format!("variant `{}` has {} value(s) but the pattern has {}", variant.name, field_tys.len(), subs.len()))
                    .primary(span, "wrong number of sub-patterns"),
            );
            for sp in subs {
                self.declare_error_bindings(sp, names);
            }
            return self.err_pat(expected, span);
        }
        let mut fields = Vec::new();
        for (i, &fty) in field_tys.iter().enumerate() {
            match subs.get(i) {
                Some(sp) => fields.push(self.check_pat_inner(sp, fty, mode, names, in_or)),
                None => fields.push(Pat { kind: P::Wild, ty: fty, span }),
            }
        }
        let targs: Vec<Ty> = targs.iter().map(|&t| self.shallow(t)).collect();
        Pat { kind: P::Variant { adt: id, variant: v, type_args: targs, fields }, ty: expected, span }
    }

    #[allow(clippy::too_many_arguments)]
    fn struct_pat(&mut self, id: AdtId, args: &[PatArg], expected: Ty, mode: BindMode, names: &mut Vec<(String, Span)>, span: Span, in_or: bool) -> Pat {
        let tcx = self.tcx();
        let adt = self.p.adt(id);
        let targs = self.fresh_args(adt.generics.len(), span);
        let ty = tcx.adt(id, targs.clone());
        if !self.unify(ty, expected) {
            self.mismatch(span, expected, ty);
            return self.err_pat(expected, span);
        }
        let AdtKind::Struct { fields } = &adt.kind else { return self.err_pat(expected, span) };
        let mut subs: Vec<Option<Pat>> = vec![None; fields.len()];
        let mut rest = false;
        for a in args {
            match a {
                PatArg::Rest(_) => rest = true,
                PatArg::Positional(p) => {
                    self.err(
                        Diagnostic::error("E0342", "struct patterns name their fields")
                            .primary(p.span, "missing field name")
                            .help("write `field: pattern`"),
                    );
                    return self.err_pat(expected, span);
                }
                PatArg::Named(n, p) => match fields.iter().position(|f| f.name == n.name) {
                    Some(i) => {
                        if !fields[i].is_pub && adt.module != self.module {
                            self.err(Diagnostic::error("E0226", format!("field `{}` of `{}` is private", n.name, adt.name)).primary(n.span, "private field"));
                        }
                        let fty = tcx.subst(fields[i].ty, &targs);
                        let sub = self.check_pat_inner(p, fty, mode, names, in_or);
                        subs[i] = Some(sub);
                    }
                    None => {
                        self.err(Diagnostic::error("E0309", format!("`{}` has no field named `{}`", adt.name, n.name)).primary(n.span, "unknown field"));
                        self.declare_error_bindings(p, names);
                        return self.err_pat(expected, span);
                    }
                },
            }
        }
        let missing: Vec<&str> = fields.iter().zip(subs.iter()).filter(|(_, s)| s.is_none()).map(|(f, _)| f.name.as_str()).collect();
        if !missing.is_empty() && !rest {
            self.err(
                Diagnostic::error("E0342", format!("pattern does not mention field{} {}", if missing.len() == 1 { "" } else { "s" }, missing.iter().map(|m| format!("`{}`", m)).collect::<Vec<_>>().join(", ")))
                    .primary(span, "incomplete struct pattern")
                    .help("add the missing fields, or `..` to ignore them"),
            );
        }
        let fields: Vec<Pat> = subs
            .into_iter()
            .zip(fields.iter())
            .map(|(s, f)| s.unwrap_or(Pat { kind: P::Wild, ty: tcx.subst(f.ty, &targs), span }))
            .collect();
        let targs: Vec<Ty> = targs.iter().map(|&t| self.shallow(t)).collect();
        Pat { kind: P::Struct { adt: id, type_args: targs, fields }, ty: expected, span }
    }
}

//! Traits in the type checker: bounds, method lookup through implementations and
//! bounds, `dyn Trait` compatibility.

use super::FnCtxt;
use crate::defs::{FnId, TraitId};
use crate::ty::{Ty, TyKind};
use nxc_syntax::{Diagnostic, Span};

impl<'a> FnCtxt<'a> {
    /// Does `ty` implement the trait? Generic parameters implement the traits of their
    /// bounds (and the supertraits of those).
    pub(crate) fn implements(&self, ty: Ty, tr: TraitId) -> bool {
        let ty = self.resolve(ty);
        let tcx = self.tcx();
        // `Copy` is satisfied by the structure of the type (spec §3.7), not by an `impl`.
        if self.p.lang_trait("copy") == Some(tr) && !matches!(&*tcx.kind(ty), TyKind::Error | TyKind::Never | TyKind::Infer(_)) {
            return self.p.is_copy_with(ty, &self.bounds);
        }
        match &*tcx.kind(ty) {
            TyKind::Error | TyKind::Never => true,
            TyKind::Param(i) => self.bounds.iter().any(|b| b.param == *i && self.p.trait_closure(b.trait_id).contains(&tr)),
            TyKind::Dyn(d) => self.p.trait_closure(*d).contains(&tr),
            _ => match self.p.find_impl(tr, ty) {
                Some((imp, args)) => self.p.impls[imp.idx()].bounds.iter().all(|b| self.implements(args[b.param as usize], b.trait_id)),
                None => false,
            },
        }
    }

    /// The traits that `ty` implements and that declare a function `name`: candidates
    /// for `value.name(...)` or `Type.name(...)` (trait, declaration).
    pub(crate) fn trait_methods_for(&self, ty: Ty, name: &str) -> Vec<(TraitId, FnId)> {
        let ty = self.resolve(ty);
        let tcx = self.tcx();
        let traits: Vec<TraitId> = match &*tcx.kind(ty) {
            TyKind::Param(i) => {
                let mut v = Vec::new();
                for b in self.bounds.iter().filter(|b| b.param == *i) {
                    for t in self.p.trait_closure(b.trait_id) {
                        if !v.contains(&t) {
                            v.push(t);
                        }
                    }
                }
                v
            }
            TyKind::Dyn(d) => self.p.trait_closure(*d),
            TyKind::Error | TyKind::Infer(_) => vec![],
            _ => {
                let mut v: Vec<TraitId> = Vec::new();
                for imp in &self.p.impls {
                    if !v.contains(&imp.trait_id) && self.p.find_impl(imp.trait_id, ty).is_some() {
                        v.push(imp.trait_id);
                    }
                }
                v
            }
        };
        traits.into_iter().filter_map(|t| self.p.traits[t.idx()].method(name).map(|m| (t, m.decl))).collect()
    }

    /// Picks one trait method among candidates, reporting an ambiguity.
    pub(crate) fn pick_trait_method(&self, cands: &[(TraitId, FnId)], name: &str, span: Span) -> Option<FnId> {
        match cands {
            [] => None,
            [(_, f)] => Some(*f),
            _ => {
                let names: Vec<String> = cands.iter().map(|(t, _)| format!("`{}`", self.p.traits[t.idx()].name)).collect();
                self.err(
                    Diagnostic::error("E0245", format!("`{}` is ambiguous: it is declared by several traits", name))
                        .primary(span, "ambiguous method")
                        .note(format!("declared by {}", names.join(" and "))),
                );
                Some(cands[0].1)
            }
        }
    }

    /// `dyn Trait` needs every method of the trait (and its supertraits) to be callable
    /// through a pointer: a `self` or `inout self` receiver, no generic parameters, and
    /// `Self` nowhere else in the signature.
    pub(crate) fn check_dyn_compatible(&self, tr: TraitId, span: Span) -> bool {
        for t in self.p.trait_closure(tr) {
            for m in &self.p.traits[t.idx()].methods {
                let fd = self.p.func(m.decl);
                let self_ty = self.tcx().param(0);
                let bad = if !fd.has_self {
                    Some("it has no `self` parameter")
                } else if fd.params[0].conv == nxc_syntax::ast::Convention::Owned {
                    Some("it takes `owned self`")
                } else if fd.generics.len() > 1 {
                    Some("it has generic parameters")
                } else if fd.params[1..].iter().any(|p| self.mentions(p.ty, self_ty)) || self.mentions(fd.ret, self_ty) {
                    Some("it uses `Self` outside of the receiver")
                } else {
                    None
                };
                if let Some(why) = bad {
                    self.err(
                        Diagnostic::error("E0246", format!("`{}` cannot be used as `dyn {}`", self.p.traits[tr.idx()].name, self.p.traits[tr.idx()].name))
                            .primary(span, "not usable through `dyn`")
                            .secondary(fd.span, format!("method `{}` cannot be called through `dyn` because {}", m.name, why))
                            .help("use a generic parameter `[T: Trait]` instead, or change the method"),
                    );
                    return false;
                }
            }
        }
        true
    }

    fn mentions(&self, t: Ty, needle: Ty) -> bool {
        if t == needle {
            return true;
        }
        match &*self.tcx().kind(t) {
            TyKind::Adt(_, a) | TyKind::Tuple(a) => a.iter().any(|&x| self.mentions(x, needle)),
            TyKind::Array(e, _) | TyKind::Ptr(e) => self.mentions(*e, needle),
            TyKind::Fn(ps, r) | TyKind::Closure(ps, r) => ps.iter().any(|&(_, x)| self.mentions(x, needle)) || self.mentions(*r, needle),
            _ => false,
        }
    }
}

//! Distinct and constrained types in the type checker (ADR 0014). Such a type is a
//! structure whose only field, hidden, holds a value of its base (`crate::newtype`):
//! - `x as T` builds it, after the generated validation for a constrained type, which
//!   panics at the position of `as`; `t as Base` reads the field;
//! - a literal where `T` is expected is a `T`, its condition checked at compile time (E0252);
//! - a constrained type that is not `distinct` converts implicitly to its base: as an
//!   operand, an argument of the base type, or a receiver of the methods of the base.

use super::{Expect, FnCtxt};
use crate::defs::{AdtId, ConstValue};
use crate::newtype::VALIDATE_FN;
use crate::thir::{self, CallArg, Callee, ExprKind as T};
use crate::ty::{FloatTy, Ty, TyKind};
use nxc_syntax::ast::{self, BinOp, ConstraintKind, Convention, Lit, UnOp};
use nxc_syntax::{Diagnostic, Span};

impl<'a> FnCtxt<'a> {
    /// The distinct or constrained type that `t` names: (type, base, distinct).
    pub(crate) fn newtype_of(&self, t: Ty) -> Option<(AdtId, Ty, bool)> {
        let (id, _) = self.tcx().as_adt(self.shallow(t))?;
        let a = self.p.adt(id);
        let nt = a.newtype.as_ref()?;
        Some((id, a.fields().first()?.ty, nt.distinct))
    }

    /// `x`, of the base type, as a value of the new type `id`. The condition of a constrained
    /// type is checked: at compile time on a literal (E0252), otherwise by the generated
    /// validation.
    pub(crate) fn wrap_newtype(&mut self, id: AdtId, x: thir::Expr, span: Span) -> thir::Expr {
        let tcx = self.tcx();
        let a = self.p.adt(id);
        let constrained = a.newtype.as_ref().is_some_and(|n| n.cond.is_some());
        let inner = match (&x.kind, a.methods.get(VALIDATE_FN)) {
            (T::Lit(v), _) if constrained => {
                let v = v.clone();
                self.check_literal_condition(id, &v, x.span);
                x
            }
            (_, Some(&f)) if constrained => {
                let ty = x.ty;
                self.mk(T::Call { callee: Callee::Fn(f, vec![]), args: vec![CallArg { conv: Convention::Borrow, expr: x }] }, ty, span)
            }
            _ => x,
        };
        self.mk(T::Adt { adt: id, variant: None, type_args: vec![], fields: vec![inner] }, tcx.adt(id, vec![]), span)
    }

    /// The value of the base held by `x`, a value of a new type.
    pub(crate) fn unwrap_newtype(&mut self, x: thir::Expr, base: Ty) -> thir::Expr {
        let span = x.span;
        self.mk(T::Field(Box::new(x), 0), base, span)
    }

    /// A value of a constrained type that is not `distinct` is a value of its base
    /// (ADR 0014 §2.4); other values are unchanged.
    pub(crate) fn widen_constrained(&mut self, x: thir::Expr) -> thir::Expr {
        match self.newtype_of(x.ty) {
            Some((_, base, false)) => self.unwrap_newtype(x, base),
            _ => x,
        }
    }

    /// `inner as T`, `T` a new type: `inner` converted to the base, then checked.
    pub(crate) fn cast_to_newtype(&mut self, inner: &ast::Expr, id: AdtId, base: Ty, span: Span) -> thir::Expr {
        let x = self.check_expr(inner, Expect::Has(base));
        let x = self.to_base(x, base, span);
        self.wrap_newtype(id, x, span)
    }

    /// `x` converted to `to` with the rules of `as` (nothing to do if it has that type);
    /// a value of a new type gives its base first.
    pub(crate) fn to_base(&mut self, x: thir::Expr, base: Ty, span: Span) -> thir::Expr {
        let x = match self.newtype_of(x.ty) {
            Some((_, b, _)) => self.unwrap_newtype(x, b),
            None => x,
        };
        if self.unify(x.ty, base) {
            return x;
        }
        self.deferred.push(super::Deferred::Cast { from: x.ty, to: base, span });
        self.mk(T::Cast(Box::new(x)), base, span)
    }

    /// A literal where a new type is expected is a value of that type (ADR 0014 §1.3).
    pub(crate) fn newtype_literal(&mut self, lit: &Lit, expected: Ty, span: Span, negated: bool) -> Option<thir::Expr> {
        let (id, base, _) = self.newtype_of(expected)?;
        let fits = match (lit, &*self.tcx().kind(base)) {
            (Lit::Int(_), TyKind::Int(_) | TyKind::Float(_)) | (Lit::Float(_), TyKind::Float(_)) | (Lit::Char(_), TyKind::Char) | (Lit::Bool(_), TyKind::Bool) => true,
            _ => false,
        };
        if !fits {
            return None;
        }
        let x = self.check_lit(lit, Expect::Has(base), span, negated);
        Some(self.wrap_newtype(id, x, span))
    }

    /// E0252: a constant that does not satisfy the condition of its constrained type.
    fn check_literal_condition(&mut self, id: AdtId, v: &ConstValue, span: Span) {
        let a = self.p.adt(id);
        let Some(cond) = a.newtype.as_ref().and_then(|n| n.cond.as_ref()) else { return };
        let base = a.fields()[0].ty;
        let f32_base = matches!(
            &*self.tcx().kind(base),
            TyKind::Float(FloatTy::F32)
        );
        if eval(cond, v, f32_base) == Some(false) {
            self.err(
                Diagnostic::error("E0252", format!("`{}` does not satisfy the condition of `{}`", show(v), a.name))
                    .primary(span, "outside the condition")
                    .secondary(cond.span, "the condition"),
            );
        }
    }
}

fn show(v: &ConstValue) -> String {
    match v {
        ConstValue::Int(n) => n.to_string(),
        ConstValue::Float(f) => f.to_string(),
        ConstValue::Char(c) => format!("{:?}", c),
        ConstValue::Bool(b) => b.to_string(),
        _ => "this value".into(),
    }
}

/// A number to compare: integers exactly, floats as `f64`, characters by code point.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Num {
    Int(i128),
    Float(f64),
}

fn num_of_value(v: &ConstValue) -> Option<Num> {
    match v {
        ConstValue::Int(n) => Some(Num::Int(*n)),
        ConstValue::Float(f) => Some(Num::Float(*f)),
        ConstValue::Char(c) => Some(Num::Int(*c as i128)),
        _ => None,
    }
}

fn num_of_expr(e: &ast::Expr) -> Option<Num> {
    match &e.kind {
        ast::ExprKind::Paren(x) => num_of_expr(x),
        ast::ExprKind::Unary(UnOp::Neg, x) => match num_of_expr(x)? {
            Num::Int(n) => Some(Num::Int(-n)),
            Num::Float(f) => Some(Num::Float(-f)),
        },
        ast::ExprKind::Lit(Lit::Int(n)) => Some(Num::Int(*n as i128)),
        ast::ExprKind::Lit(Lit::Float(f)) => Some(Num::Float(*f)),
        ast::ExprKind::Lit(Lit::Char(c)) => Some(Num::Int(*c as i128)),
        _ => None,
    }
}

/// `a` compared with `b` (an integer and a float compare as floats).
fn cmp(a: Num, op: BinOp, b: Num) -> Option<bool> {
    let (x, y) = match (a, b) {
        (Num::Int(x), Num::Int(y)) => return Some(cmp_ord(x.cmp(&y), op)),
        (Num::Int(x), Num::Float(y)) => (x as f64, y),
        (Num::Float(x), Num::Int(y)) => (x, y as f64),
        (Num::Float(x), Num::Float(y)) => (x, y),
    };
    Some(match op {
        BinOp::Eq => x == y,
        BinOp::Ne => x != y,
        BinOp::Lt => x < y,
        BinOp::Le => x <= y,
        BinOp::Gt => x > y,
        BinOp::Ge => x >= y,
        _ => return None,
    })
}

fn cmp_ord(o: std::cmp::Ordering, op: BinOp) -> bool {
    use std::cmp::Ordering::*;
    match op {
        BinOp::Eq => o == Equal,
        BinOp::Ne => o != Equal,
        BinOp::Lt => o == Less,
        BinOp::Le => o != Greater,
        BinOp::Gt => o == Greater,
        _ => o != Less,
    }
}

fn at_f32_precision(n: Num, f32_base: bool) -> Num {
    if !f32_base {
        return n;
    }
    let f = match n {
        Num::Int(n) => n as f32,
        Num::Float(f) => f as f32,
    };
    Num::Float(f as f64)
}

/// The condition evaluated on a constant; `None` if it cannot be (an error was reported
/// on the declaration).
fn eval(c: &ast::Constraint, v: &ConstValue, f32_base: bool) -> Option<bool> {
    let it = at_f32_precision(num_of_value(v)?, f32_base);
    match &c.kind {
        ConstraintKind::Or(a, b) => Some(eval(a, v, f32_base)? || eval(b, v, f32_base)?),
        ConstraintKind::And(a, b) => Some(eval(a, v, f32_base)? && eval(b, v, f32_base)?),
        ConstraintKind::Not(a) => Some(!eval(a, v, f32_base)?),
        ConstraintKind::In { lo, hi, inclusive } => {
            let lo = at_f32_precision(num_of_expr(lo)?, f32_base);
            let hi = at_f32_precision(num_of_expr(hi)?, f32_base);
            let ge = cmp(it, BinOp::Ge, lo)?;
            let below = cmp(it, if *inclusive { BinOp::Le } else { BinOp::Lt }, hi)?;
            Some(ge && below)
        }
        ConstraintKind::Cmp { op, value, it_left } => {
            let k = at_f32_precision(num_of_expr(value)?, f32_base);
            if *it_left {
                cmp(it, *op, k)
            } else {
                cmp(k, *op, it)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expr(lit: Lit) -> ast::Expr {
        ast::Expr {
            kind: ast::ExprKind::Lit(lit),
            span: Span::DUMMY,
        }
    }

    fn cmp_constraint(op: BinOp, value: Lit, it_left: bool) -> ast::Constraint {
        ast::Constraint {
            kind: ConstraintKind::Cmp {
                op,
                value: expr(value),
                it_left,
            },
            span: Span::DUMMY,
        }
    }

    #[test]
    fn f32_literal_constraints_use_the_base_type_precision() {
        let value = ConstValue::Float(16_777_216.0);
        let upper_float = cmp_constraint(BinOp::Lt, Lit::Float(16_777_217.0), true);
        let upper_int = cmp_constraint(BinOp::Lt, Lit::Int(16_777_217), true);
        let reversed = cmp_constraint(BinOp::Gt, Lit::Float(16_777_217.0), false);

        assert_eq!(eval(&upper_float, &value, true), Some(false));
        assert_eq!(eval(&upper_float, &value, false), Some(true));
        assert_eq!(eval(&upper_int, &value, true), Some(false));
        assert_eq!(eval(&upper_int, &value, false), Some(true));
        assert_eq!(eval(&reversed, &value, true), Some(false));
        assert_eq!(eval(&reversed, &value, false), Some(true));
    }

    #[test]
    fn f32_precision_is_kept_through_ranges_and_compound_conditions() {
        let value = ConstValue::Float(16_777_216.0);
        let range = ast::Constraint {
            kind: ConstraintKind::In {
                lo: expr(Lit::Float(16_777_217.0)),
                hi: expr(Lit::Float(16_777_218.0)),
                inclusive: true,
            },
            span: Span::DUMMY,
        };
        let below_unrounded_bound = cmp_constraint(BinOp::Lt, Lit::Float(16_777_217.0), true);
        let range_and_not = ast::Constraint {
            kind: ConstraintKind::And(
                Box::new(range.clone()),
                Box::new(ast::Constraint {
                    kind: ConstraintKind::Not(Box::new(below_unrounded_bound.clone())),
                    span: Span::DUMMY,
                }),
            ),
            span: Span::DUMMY,
        };
        let condition = ast::Constraint {
            kind: ConstraintKind::Or(Box::new(below_unrounded_bound), Box::new(range_and_not)),
            span: Span::DUMMY,
        };

        assert_eq!(eval(&range, &value, true), Some(true));
        assert_eq!(eval(&range, &value, false), Some(false));
        assert_eq!(eval(&condition, &value, true), Some(true));
    }
}

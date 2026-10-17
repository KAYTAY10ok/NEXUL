//! Compile-time evaluation of `const` items and array lengths.
//!
//! Constant expressions may use literals, other constants, unary and binary operators,
//! `as` conversions and the associated constants of primitive types (`i32.MAX`).
//! Every operation is checked: an overflow is a compile-time error.

use crate::collect::{lookup_in_module, lookup_item, primitive_type};
use crate::defs::{ConstId, ConstValue, ItemRef, ModId, Program};
use crate::ty::{FloatTy, IntTy, Ty, TyKind};
use nxc_syntax::ast::{BinOp, Expr, ExprKind, Lit, TypeExprKind, UnOp};
use nxc_syntax::{DiagSink, Diagnostic, Span};

/// Evaluates every `const` item, checking it against its declared type.
pub fn eval_all(prog: &mut Program, diags: &DiagSink) {
    for i in 0..prog.consts.len() {
        let id = ConstId(i as u32);
        if prog.const_values.contains_key(&id) {
            continue;
        }
        let mut visiting = Vec::new();
        if let Some(v) = eval_const(prog, id, &mut visiting, diags) {
            prog.const_values.insert(id, v);
        }
    }
}

fn eval_const(prog: &Program, id: ConstId, visiting: &mut Vec<ConstId>, diags: &DiagSink) -> Option<ConstValue> {
    if let Some(v) = prog.const_values.get(&id) {
        return Some(v.clone());
    }
    let c = &prog.consts[id.idx()];
    if visiting.contains(&id) {
        diags.emit(Diagnostic::error("E0372", format!("constant `{}` depends on itself", c.name)).primary(c.span, "cyclic constant"));
        return None;
    }
    // The type of a constant may not be resolved yet (an array length in a type alias
    // can refer to a constant): resolve it on demand.
    let ty = if prog.tcx.is_error(c.ty) {
        let scope = crate::collect::TypeScope { module: c.module, generics: &[], self_ty: None };
        crate::collect::resolve_type(prog, &scope, &c.ast.ty, diags)
    } else {
        c.ty
    };
    if prog.tcx.is_error(ty) {
        return None;
    }
    visiting.push(id);
    let r = eval_expr(prog, c.module, &c.ast.value, ty, visiting, diags);
    visiting.pop();
    let v = r?;
    check_fits(prog, &v, ty, c.ast.value.span, diags)
}

/// Array length in a type: an integer literal or an integer constant.
pub fn eval_array_len(prog: &Program, module: ModId, e: &Expr, diags: &DiagSink) -> Option<u64> {
    let mut visiting = Vec::new();
    match eval_expr(prog, module, e, prog.tcx.usize, &mut visiting, diags)? {
        ConstValue::Int(n) if n >= 0 && n <= u32::MAX as i128 => Some(n as u64),
        ConstValue::Int(_) => {
            diags.emit(Diagnostic::error("E0373", "array length is out of range").primary(e.span, "must be between 0 and 4294967295"));
            None
        }
        _ => {
            diags.emit(Diagnostic::error("E0373", "array length must be an integer").primary(e.span, "not an integer"));
            None
        }
    }
}

fn not_const(e: &Expr, diags: &DiagSink) -> Option<ConstValue> {
    diags.emit(
        Diagnostic::error("E0371", "this expression cannot be evaluated at compile time")
            .primary(e.span, "not a constant expression")
            .note("constant expressions may use literals, other constants, operators and `as`"),
    );
    None
}

fn overflow(span: Span, ty: &str, diags: &DiagSink) -> Option<ConstValue> {
    diags.emit(Diagnostic::error("E0370", format!("arithmetic overflow in constant expression of type `{}`", ty)).primary(span, "overflows"));
    None
}

/// ` on the target `name``, after the range of `isize`/`usize`, whose width depends on
/// the target (spec §3.1).
pub fn on_target(prog: &Program, i: IntTy) -> String {
    match i {
        IntTy::Isize | IntTy::Usize => format!(" on the target `{}`", prog.cfg.target.name),
        _ => String::new(),
    }
}

/// Checks that a value matches a type and fits in its range.
pub fn check_fits(prog: &Program, v: &ConstValue, ty: Ty, span: Span, diags: &DiagSink) -> Option<ConstValue> {
    let tcx = &prog.tcx;
    let mismatch = |what: &str| {
        diags.emit(
            Diagnostic::error("E0302", "mismatched types in constant")
                .primary(span, format!("expected `{}`, found {}", prog.ty_str(ty), what)),
        );
        None
    };
    match (&*tcx.kind(ty), v) {
        (TyKind::Int(i), ConstValue::Int(n)) => {
            if *n < prog.int_min(*i) || *n > prog.int_max(*i) {
                diags.emit(
                    Diagnostic::error("E0301", format!("value {} does not fit in `{}`", n, i.name()))
                        .primary(span, format!("the range of `{}` is {}..={}{}", i.name(), prog.int_min(*i), prog.int_max(*i), on_target(prog, *i))),
                );
                return None;
            }
            Some(v.clone())
        }
        (TyKind::Float(_), ConstValue::Float(_)) => Some(v.clone()),
        (TyKind::Float(f), ConstValue::Int(n)) => {
            let x = *n as f64;
            if x as i128 != *n || (*f == FloatTy::F32 && (x as f32) as f64 != x) {
                diags.emit(Diagnostic::error("E0301", format!("{} cannot be represented exactly as `{}`", n, f.name())).primary(span, "inexact value"));
                return None;
            }
            Some(ConstValue::Float(x))
        }
        (TyKind::Bool, ConstValue::Bool(_)) | (TyKind::Char, ConstValue::Char(_)) | (TyKind::Unit, ConstValue::Unit) => Some(v.clone()),
        (TyKind::Adt(..), ConstValue::Str(_)) if prog.is_string(ty) => Some(v.clone()),
        (TyKind::Error, _) => None,
        (_, ConstValue::Int(_)) => mismatch("an integer"),
        (_, ConstValue::Float(_)) => mismatch("a float"),
        (_, ConstValue::Bool(_)) => mismatch("`bool`"),
        (_, ConstValue::Char(_)) => mismatch("`char`"),
        (_, ConstValue::Str(_)) => mismatch("`String`"),
        (_, ConstValue::Unit) => mismatch("`()`"),
    }
}

/// Associated constants of primitive types (`i32.MAX`, `f64.INFINITY`, ...).
pub fn prim_assoc_const(prog: &Program, ty: Ty, name: &str) -> Option<ConstValue> {
    let tcx = &prog.tcx;
    match &*tcx.kind(ty) {
        TyKind::Int(i) => match name {
            "MIN" => Some(ConstValue::Int(prog.int_min(*i))),
            "MAX" => Some(ConstValue::Int(prog.int_max(*i))),
            "BITS" => Some(ConstValue::Int(prog.int_bits(*i) as i128)),
            _ => None,
        },
        TyKind::Float(f) => {
            let (max, eps, min_pos) = match f {
                FloatTy::F32 => (f32::MAX as f64, f32::EPSILON as f64, f32::MIN_POSITIVE as f64),
                FloatTy::F64 => (f64::MAX, f64::EPSILON, f64::MIN_POSITIVE),
            };
            Some(ConstValue::Float(match name {
                "INFINITY" => f64::INFINITY,
                "NEG_INFINITY" => f64::NEG_INFINITY,
                "NAN" => f64::NAN,
                "MAX" => max,
                "MIN" => -max,
                "EPSILON" => eps,
                "MIN_POSITIVE" => min_pos,
                _ => return None,
            }))
        }
        TyKind::Char => match name {
            "MAX" => Some(ConstValue::Char(char::MAX)),
            _ => None,
        },
        _ => None,
    }
}

fn int_ty_of(prog: &Program, ty: Ty) -> Option<IntTy> {
    prog.tcx.as_int(ty)
}

/// Resolves a path expression (`NAME`, `math.PI`, `i32.MAX`) to a constant value.
fn eval_path(prog: &Program, module: ModId, e: &Expr, visiting: &mut Vec<ConstId>, diags: &DiagSink) -> Option<ConstValue> {
    match &e.kind {
        ExprKind::Ident(name) => match lookup_item(prog, module, name) {
            Some(ItemRef::Const(c)) => eval_const(prog, c, visiting, diags),
            Some(_) => not_const(e, diags),
            None => {
                diags.emit(Diagnostic::error("E0202", format!("cannot find constant `{}`", name)).primary(e.span, "not found in this scope"));
                None
            }
        },
        ExprKind::Field(base, field) => {
            if let ExprKind::Ident(tn) = &base.kind {
                if let Some(pt) = primitive_type(prog, tn) {
                    return match prim_assoc_const(prog, pt, &field.name) {
                        Some(v) => Some(v),
                        None => {
                            diags.emit(Diagnostic::error("E0202", format!("`{}` has no constant `{}`", tn, field.name)).primary(field.span, "unknown constant"));
                            None
                        }
                    };
                }
            }
            // module.path.CONST
            let mut segs = vec![field];
            let mut cur = &**base;
            loop {
                match &cur.kind {
                    ExprKind::Field(b, f) => {
                        segs.push(f);
                        cur = b;
                    }
                    ExprKind::Ident(first) => {
                        let mut m = match lookup_item(prog, module, first) {
                            Some(ItemRef::Module(m)) => m,
                            _ => return not_const(e, diags),
                        };
                        segs.reverse();
                        for (i, s) in segs.iter().enumerate() {
                            let item = lookup_in_module(prog, module, m, &s.name, s.span, diags)?;
                            match item {
                                ItemRef::Module(next) if i + 1 < segs.len() => m = next,
                                ItemRef::Const(c) if i + 1 == segs.len() => return eval_const(prog, c, visiting, diags),
                                _ => return not_const(e, diags),
                            }
                        }
                        return not_const(e, diags);
                    }
                    _ => return not_const(e, diags),
                }
            }
        }
        _ => not_const(e, diags),
    }
}

/// Evaluates `e`, whose expected type is `ty` (used to type literals).
pub fn eval_expr(prog: &Program, module: ModId, e: &Expr, ty: Ty, visiting: &mut Vec<ConstId>, diags: &DiagSink) -> Option<ConstValue> {
    let tcx = &prog.tcx;
    let int_ty = int_ty_of(prog, ty);
    match &e.kind {
        ExprKind::Paren(inner) => eval_expr(prog, module, inner, ty, visiting, diags),
        ExprKind::Lit(Lit::Int(v)) => {
            if tcx.as_float(ty).is_some() {
                Some(ConstValue::Float(*v as f64))
            } else {
                Some(ConstValue::Int(*v as i128))
            }
        }
        ExprKind::Lit(Lit::Float(v)) => Some(ConstValue::Float(*v)),
        ExprKind::Lit(Lit::Bool(b)) => Some(ConstValue::Bool(*b)),
        ExprKind::Lit(Lit::Char(c)) => Some(ConstValue::Char(*c)),
        ExprKind::Lit(Lit::Str(s)) => Some(ConstValue::Str(s.clone())),
        ExprKind::Unit => Some(ConstValue::Unit),
        ExprKind::Ident(_) | ExprKind::Field(..) => eval_path(prog, module, e, visiting, diags),
        ExprKind::Unary(op, inner) => {
            let v = eval_expr(prog, module, inner, ty, visiting, diags)?;
            match (op, v) {
                (UnOp::Neg, ConstValue::Int(n)) => Some(ConstValue::Int(-n)),
                (UnOp::Neg, ConstValue::Float(f)) => Some(ConstValue::Float(-f)),
                (UnOp::Not, ConstValue::Bool(b)) => Some(ConstValue::Bool(!b)),
                (UnOp::BitNot, ConstValue::Int(n)) => match int_ty {
                    Some(i) if i.signed() => Some(ConstValue::Int(!n)),
                    Some(i) => Some(ConstValue::Int(prog.int_max(i) - n)),
                    None => not_const(e, diags),
                },
                _ => not_const(e, diags),
            }
        }
        ExprKind::Cast(inner, texpr) => {
            let target = match &texpr.kind {
                TypeExprKind::Path { segments, args } if segments.len() == 1 && args.is_empty() => primitive_type(prog, &segments[0].name),
                _ => None,
            };
            let Some(target) = target else { return not_const(e, diags) };
            // The operand's own type is unknown here; evaluate it with a neutral hint.
            let hint = match &inner.kind {
                ExprKind::Lit(Lit::Float(_)) => tcx.f64,
                _ => tcx.i64,
            };
            let v = eval_expr(prog, module, inner, hint, visiting, diags)?;
            let r = match (&*tcx.kind(target), v) {
                (TyKind::Int(_), ConstValue::Int(n)) => ConstValue::Int(n),
                (TyKind::Int(_), ConstValue::Float(f)) => {
                    if f.is_nan() {
                        return not_const(e, diags);
                    }
                    ConstValue::Int(f.trunc() as i128)
                }
                (TyKind::Int(_), ConstValue::Bool(b)) => ConstValue::Int(b as i128),
                (TyKind::Int(_), ConstValue::Char(c)) => ConstValue::Int(c as i128),
                (TyKind::Float(_), ConstValue::Int(n)) => ConstValue::Float(n as f64),
                (TyKind::Float(FloatTy::F32), ConstValue::Float(f)) => ConstValue::Float(f as f32 as f64),
                (TyKind::Float(_), ConstValue::Float(f)) => ConstValue::Float(f),
                (TyKind::Char, ConstValue::Int(n)) => match u32::try_from(n).ok().and_then(char::from_u32) {
                    Some(c) => ConstValue::Char(c),
                    None => {
                        diags.emit(Diagnostic::error("E0301", format!("{} is not a valid `char`", n)).primary(e.span, "invalid character value"));
                        return None;
                    }
                },
                _ => return not_const(e, diags),
            };
            check_fits(prog, &r, target, e.span, diags)
        }
        ExprKind::Binary(op, l, r) => {
            let operand_ty = if op.is_comparison() || op.is_logical() {
                // Operands of a comparison have their own type: infer it from a literal.
                tcx.i64
            } else {
                ty
            };
            let a = eval_expr(prog, module, l, operand_ty, visiting, diags)?;
            let shift_ty = if op.is_shift() { tcx.u32 } else { operand_ty };
            let b = eval_expr(prog, module, r, shift_ty, visiting, diags)?;
            let name = prog.ty_str(ty);
            let range_check = |n: i128| -> Option<i128> {
                match int_ty {
                    Some(i) if n < prog.int_min(i) || n > prog.int_max(i) => None,
                    _ => Some(n),
                }
            };
            use BinOp::*;
            match (a, b) {
                (ConstValue::Int(x), ConstValue::Int(y)) => {
                    let res = match op {
                        Add | AddWrap => x.checked_add(y),
                        Sub | SubWrap => x.checked_sub(y),
                        Mul | MulWrap => x.checked_mul(y),
                        Div | Rem => {
                            if y == 0 {
                                diags.emit(Diagnostic::error("E0370", "division by zero in constant expression").primary(e.span, "divides by zero"));
                                return None;
                            }
                            if *op == Div {
                                x.checked_div(y)
                            } else {
                                x.checked_rem(y)
                            }
                        }
                        BitAnd => Some(x & y),
                        BitOr => Some(x | y),
                        BitXor => Some(x ^ y),
                        Shl | Shr => {
                            let bits = int_ty.map(|i| prog.int_bits(i)).unwrap_or(64) as i128;
                            if y < 0 || y >= bits {
                                diags.emit(Diagnostic::error("E0370", "shift amount out of range in constant expression").primary(e.span, format!("must be between 0 and {}", bits - 1)));
                                return None;
                            }
                            if *op == Shl {
                                let m = x << y;
                                // Wrap to the type's width like the runtime operation.
                                match int_ty {
                                    Some(i) => {
                                        let mask = (1i128 << prog.int_bits(i)) - 1;
                                        let u = m & mask;
                                        if i.signed() && u > prog.int_max(i) {
                                            Some(u - (1i128 << prog.int_bits(i)))
                                        } else {
                                            Some(u)
                                        }
                                    }
                                    None => Some(m),
                                }
                            } else {
                                Some(x >> y)
                            }
                        }
                        Eq => return Some(ConstValue::Bool(x == y)),
                        Ne => return Some(ConstValue::Bool(x != y)),
                        Lt => return Some(ConstValue::Bool(x < y)),
                        Le => return Some(ConstValue::Bool(x <= y)),
                        Gt => return Some(ConstValue::Bool(x > y)),
                        Ge => return Some(ConstValue::Bool(x >= y)),
                        And | Or => return not_const(e, diags),
                    };
                    let wrapping = matches!(op, AddWrap | SubWrap | MulWrap);
                    match res {
                        Some(n) if wrapping => match int_ty {
                            Some(i) => {
                                let m = 1i128 << prog.int_bits(i);
                                let mut u = n.rem_euclid(m);
                                if i.signed() && u > prog.int_max(i) {
                                    u -= m;
                                }
                                Some(ConstValue::Int(u))
                            }
                            None => Some(ConstValue::Int(n)),
                        },
                        Some(n) => match range_check(n) {
                            Some(n) => Some(ConstValue::Int(n)),
                            None => overflow(e.span, &name, diags),
                        },
                        None => overflow(e.span, &name, diags),
                    }
                }
                (ConstValue::Float(x), ConstValue::Float(y)) => Some(match op {
                    Add => ConstValue::Float(x + y),
                    Sub => ConstValue::Float(x - y),
                    Mul => ConstValue::Float(x * y),
                    Div => ConstValue::Float(x / y),
                    Rem => ConstValue::Float(x % y),
                    Eq => ConstValue::Bool(x == y),
                    Ne => ConstValue::Bool(x != y),
                    Lt => ConstValue::Bool(x < y),
                    Le => ConstValue::Bool(x <= y),
                    Gt => ConstValue::Bool(x > y),
                    Ge => ConstValue::Bool(x >= y),
                    _ => return not_const(e, diags),
                }),
                (ConstValue::Bool(x), ConstValue::Bool(y)) => Some(ConstValue::Bool(match op {
                    And => x && y,
                    Or => x || y,
                    Eq => x == y,
                    Ne => x != y,
                    _ => return not_const(e, diags),
                })),
                (ConstValue::Str(x), ConstValue::Str(y)) if *op == Add => Some(ConstValue::Str(x + &y)),
                _ => not_const(e, diags),
            }
        }
        _ => not_const(e, diags),
    }
}

// ---------------------------------------------------------------------------
// Static variables (spec §3.9)

/// Evaluates the initial value of every `static` item.
pub fn eval_statics(prog: &mut Program, diags: &DiagSink) {
    for i in 0..prog.statics.len() {
        let st = &prog.statics[i];
        if prog.tcx.is_error(st.ty) || !prog.is_copy(st.ty) {
            continue;
        }
        let (module, ty, ast) = (st.module, st.ty, st.ast.clone());
        if let Some(v) = eval_static_value(prog, module, &ast.value, ty, diags) {
            prog.static_values.insert(crate::defs::StaticId(i as u32), v);
        }
    }
}

fn is_ptr_null(e: &Expr) -> bool {
    let ExprKind::Call(callee, args) = &e.kind else { return false };
    if !args.is_empty() {
        return false;
    }
    let ExprKind::Field(base, name) = &callee.kind else { return false };
    if name.name != "null" {
        return false;
    }
    match &base.kind {
        ExprKind::Ident(n) => n == "Ptr",
        ExprKind::Index(b, _) => matches!(&b.kind, ExprKind::Ident(n) if n == "Ptr"),
        _ => false,
    }
}

/// The value of a static initializer of type `ty`: a constant expression, `Ptr.null()`,
/// or an array, tuple or structure built from them.
pub fn eval_static_value(prog: &Program, module: ModId, e: &Expr, ty: Ty, diags: &DiagSink) -> Option<crate::defs::StaticValue> {
    use crate::defs::{AdtKind, StaticValue};
    let tcx = &prog.tcx;
    let not_static = |e: &Expr| {
        diags.emit(
            Diagnostic::error("E0371", "this expression cannot be evaluated at compile time")
                .primary(e.span, "not a constant initializer")
                .note("a static variable is initialized with constants, `Ptr.null()`, arrays, tuples and structures of them"),
        );
        None
    };
    if let ExprKind::Paren(inner) = &e.kind {
        return eval_static_value(prog, module, inner, ty, diags);
    }
    match &*tcx.kind(ty) {
        TyKind::Int(_) | TyKind::Float(_) | TyKind::Bool | TyKind::Char | TyKind::Unit => {
            let mut visiting = Vec::new();
            let v = eval_expr(prog, module, e, ty, &mut visiting, diags)?;
            check_fits(prog, &v, ty, e.span, diags).map(StaticValue::Scalar)
        }
        TyKind::Ptr(_) | TyKind::Fn(..) => {
            if is_ptr_null(e) {
                Some(StaticValue::Null)
            } else {
                not_static(e)
            }
        }
        TyKind::Array(elem, n) => match &e.kind {
            ExprKind::ArrayRepeat(x, count) => {
                let c = eval_array_len(prog, module, count, diags)?;
                if c != *n {
                    diags.emit(Diagnostic::error("E0302", "mismatched array length").primary(count.span, format!("expected {} elements, found {}", n, c)));
                    return None;
                }
                let v = eval_static_value(prog, module, x, *elem, diags)?;
                Some(StaticValue::Repeat(Box::new(v), *n))
            }
            ExprKind::Array(xs) => {
                if xs.len() as u64 != *n {
                    diags.emit(Diagnostic::error("E0302", "mismatched array length").primary(e.span, format!("expected {} elements, found {}", n, xs.len())));
                    return None;
                }
                let vs: Option<Vec<StaticValue>> = xs.iter().map(|x| eval_static_value(prog, module, x, *elem, diags)).collect();
                Some(StaticValue::Aggregate(vs?))
            }
            _ => not_static(e),
        },
        TyKind::Tuple(ts) => match &e.kind {
            ExprKind::Tuple(xs) if xs.len() == ts.len() => {
                let vs: Option<Vec<StaticValue>> = xs.iter().zip(ts.iter()).map(|(x, &t)| eval_static_value(prog, module, x, t, diags)).collect();
                Some(StaticValue::Aggregate(vs?))
            }
            _ => not_static(e),
        },
        TyKind::Adt(id, args) => {
            let adt = prog.adt(*id);
            match &adt.kind {
                AdtKind::Struct { fields } => {
                    let ExprKind::Call(_, cargs) = &e.kind else { return not_static(e) };
                    if cargs.len() != fields.len() || cargs.iter().zip(fields.iter()).any(|(a, f)| a.label.as_ref().map(|l| l.name.as_str()) != Some(f.name.as_str())) {
                        diags.emit(
                            Diagnostic::error("E0310", "every field must be given by name, in declaration order")
                                .primary(e.span, format!("`{}` has the fields {}", adt.name, fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", "))),
                        );
                        return None;
                    }
                    let vs: Option<Vec<StaticValue>> =
                        cargs.iter().zip(fields.iter()).map(|(a, f)| eval_static_value(prog, module, &a.expr, tcx.subst(f.ty, args), diags)).collect();
                    Some(StaticValue::Aggregate(vs?))
                }
                AdtKind::Enum { variants } => {
                    let name = match &e.kind {
                        ExprKind::Ident(n) => n.clone(),
                        ExprKind::Field(_, f) => f.name.clone(),
                        _ => return not_static(e),
                    };
                    match variants.iter().position(|v| v.name == name && v.fields.is_empty()) {
                        Some(i) => Some(StaticValue::Variant(i as u32)),
                        None => not_static(e),
                    }
                }
            }
        }
        _ => not_static(e),
    }
}

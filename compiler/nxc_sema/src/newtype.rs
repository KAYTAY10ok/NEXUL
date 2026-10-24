//! `type Name = distinct Base …` and `type Name = Base where …` (edition 1, ADR 0014).
//!
//! Before the analysis, each such type becomes a structure with one hidden field (`#base`,
//! a name the source cannot write) of type `Base`, so its representation and layout are
//! those of `Base`. The functions it needs are generated as edition 1 source, as
//! `@derive` does: for a constrained type, `check` and the validation used by `as`; the
//! interfaces of `Base` that the type reuses, by delegation. The type checker gives `as`,
//! the literals and the implicit conversion of a constrained type to its base their meaning
//! (`check/newtype.rs`).

use nxc_syntax::ast::{self, BinOp, ConstraintKind, ExprKind, FieldDecl, Ident, ItemKind, Lit, NewtypeDecl, StructDecl, TypeExprKind, UnOp};
use nxc_syntax::{DiagSink, Diagnostic, Edition, SourceMap};
use std::collections::HashMap;

/// The bases of a distinct or constrained type in NEXUL 1.0: numbers, `char`, `bool`.
const SCALARS: &[&str] = &["i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize", "f32", "f64", "char", "bool"];

/// The name of the hidden field.
pub const BASE_FIELD: &str = "#base";

/// The function that checks the condition for `as` (it panics at the position of `as`).
pub const VALIDATE_FN: &str = "__nexul_validate";

/// The interfaces a constrained type that is not `distinct` takes from its base: its values
/// are values of the base.
const INHERITED: &[&str] = &["Copy", "Eq", "Ord", "Display", "Hash", "Clone"];

/// Replaces the new types of the module by structures and adds their generated functions.
/// Returns the declarations, by name, for the type checker.
pub fn expand(module: &mut ast::Module, sm: &mut SourceMap, file_name: &str, diags: &DiagSink) -> HashMap<String, NewtypeDecl> {
    let mut decls = HashMap::new();
    let mut text = String::new();
    for item in module.items.iter_mut() {
        let ItemKind::Newtype(nt) = &item.kind else { continue };
        let nt = nt.clone();
        let name = nt.name.name.clone();
        let base = match &nt.base.kind {
            TypeExprKind::Path { segments, args } if segments.len() == 1 && args.is_empty() && SCALARS.contains(&segments[0].name.as_str()) => Some(segments[0].name.clone()),
            _ => {
                diags.emit(
                    Diagnostic::error("E0251", format!("`{}` must be based on a number, `char` or `bool`", name))
                        .primary(nt.base.span, "not a scalar type")
                        .note("NEXUL 1.0 accepts distinct and constrained types over the scalar types only (ADR 0014)"),
                );
                None
            }
        };
        if let (Some(b), Some(cond)) = (&base, &nt.cond) {
            if b == "bool" {
                diags.emit(Diagnostic::error("E0251", format!("`{}`: a condition applies to a number or a `char`", name)).primary(cond.span, "condition on a `bool`"));
            } else {
                check_constants(cond, b, diags);
            }
        }
        if let Some(b) = &base {
            text.push_str(&generated(item.is_pub, &nt, b, sm, diags));
        }
        let field = FieldDecl { is_pub: false, indirect: false, name: Ident { name: BASE_FIELD.into(), span: nt.name.span }, ty: nt.base.clone(), span: nt.base.span };
        item.kind = ItemKind::Struct(StructDecl { name: nt.name.clone(), generics: vec![], fields: vec![field] });
        decls.insert(name, nt);
    }
    if !text.is_empty() {
        let file = sm.add_with_edition(format!("{} (distinct and constrained types)", file_name), text, Edition::V1);
        let generated = nxc_syntax::parse_file(sm, file, diags);
        module.items.extend(generated.items);
    }
    decls
}

/// The constants of a condition are literals of the base type (ADR 0014 §2.2).
fn check_constants(c: &ast::Constraint, base: &str, diags: &DiagSink) {
    match &c.kind {
        ConstraintKind::Or(a, b) | ConstraintKind::And(a, b) => {
            check_constants(a, base, diags);
            check_constants(b, base, diags);
        }
        ConstraintKind::Not(a) => check_constants(a, base, diags),
        ConstraintKind::In { lo, hi, .. } => {
            check_constant(lo, base, diags);
            check_constant(hi, base, diags);
        }
        ConstraintKind::Cmp { value, .. } => check_constant(value, base, diags),
    }
}

fn check_constant(e: &ast::Expr, base: &str, diags: &DiagSink) {
    let mut x = e;
    let mut negated = false;
    loop {
        match &x.kind {
            ExprKind::Paren(inner) => x = inner,
            ExprKind::Unary(UnOp::Neg, inner) if !negated => {
                negated = true;
                x = inner;
            }
            _ => break,
        }
    }
    let float = base.starts_with('f');
    let ok = match &x.kind {
        ExprKind::Lit(Lit::Int(_)) => base != "char" && !(negated && base.starts_with('u')),
        ExprKind::Lit(Lit::Float(_)) => float,
        ExprKind::Lit(Lit::Char(_)) => base == "char" && !negated,
        _ => false,
    };
    if !ok {
        diags.emit(
            Diagnostic::error("E0251", format!("a condition compares `it` with literal constants of type `{}`", base))
                .primary(e.span, "not a literal of the base type")
                .note("a condition has no call and no variable: it is checked in a few instructions, and at compile time on constants"),
        );
    }
}

/// The condition as an edition 1 expression on `value`.
fn render(c: &ast::Constraint, sm: &SourceMap) -> String {
    match &c.kind {
        ConstraintKind::Or(a, b) => format!("({} or {})", render(a, sm), render(b, sm)),
        ConstraintKind::And(a, b) => format!("({} and {})", render(a, sm), render(b, sm)),
        ConstraintKind::Not(a) => format!("(not {})", render(a, sm)),
        ConstraintKind::In { lo, hi, inclusive } => format!("(value >= {} and value {} {})", sm.snippet(lo.span), if *inclusive { "<=" } else { "<" }, sm.snippet(hi.span)),
        ConstraintKind::Cmp { op, value, it_left: true } => format!("(value {} {})", op.symbol(), sm.snippet(value.span)),
        ConstraintKind::Cmp { op, value, it_left: false } => format!("({} {} value)", sm.snippet(value.span), op.symbol()),
    }
}

/// The edition 1 source generated for one type.
fn generated(is_pub: bool, nt: &NewtypeDecl, base: &str, sm: &SourceMap, diags: &DiagSink) -> String {
    let name = &nt.name.name;
    let vis = if is_pub { "pub " } else { "" };
    let mut out = String::new();
    if let Some(cond) = &nt.cond {
        let c = render(cond, sm);
        let shown = sm.snippet(cond.span).replace('\\', "\\\\").replace('"', "\\\"").replace('{', "{{").replace('}', "}}");
        out.push_str(&format!(
            "extend {name} {{\n    /// `value` as a `{name}` if it satisfies `{shown}`, else `None`.\n    {vis}fn Option[{name}] check({base} value) {{\n        if {c} {{ return .Some(value as {name}) }}\n        return .None\n    }}\n\n    @track_caller\n    {vis}fn {base} {VALIDATE_FN}({base} value) {{\n        if not {c} {{ panic(\"{{value}} does not satisfy the condition of `{name}`: {shown}\") }}\n        return value\n    }}\n}}\n\n"
        ));
    }
    let names: Vec<(String, nxc_syntax::Span)> = if nt.distinct {
        nt.interfaces
            .iter()
            .filter_map(|t| match &t.kind {
                TypeExprKind::Path { segments, args } if args.is_empty() => segments.last().map(|s| (s.name.clone(), t.span)),
                _ => {
                    diags.emit(Diagnostic::error("E0251", "a distinct type reuses interfaces of its base by name").primary(t.span, "expected an interface name"));
                    None
                }
            })
            .collect()
    } else {
        INHERITED.iter().map(|n| (n.to_string(), nt.name.span)).collect()
    };
    let constrained = nt.cond.is_some();
    for (iface, span) in names {
        let op = |f: &str, sym: BinOp| format!("    fn {name} {f}(self, {name} other) = ((self as {base}) {} (other as {base})) as {name}\n", sym.symbol());
        let body = match iface.as_str() {
            "Copy" => String::new(),
            "Eq" => format!("    fn bool eq(self, {name} other) = (self as {base}) == (other as {base})\n"),
            "Ord" => format!("    fn Ordering cmp(self, {name} other) = (self as {base}).cmp(other as {base})\n"),
            "Display" => format!("    fn String to_string(self) = (self as {base}).to_string()\n"),
            "Hash" => format!("    fn u64 hash(self) = (self as {base}).hash()\n"),
            "Clone" => format!("    fn {name} clone(self) = self\n"),
            "Add" | "Sub" | "Mul" | "Div" | "Rem" | "Neg" | "Default" if constrained => {
                diags.emit(
                    Diagnostic::error("E0251", format!("the constrained type `{}` cannot reuse `{}`", name, iface))
                        .primary(span, "its result could break the condition")
                        .help(format!("compute on the base and convert the result with `as {}`, which checks it", name)),
                );
                continue;
            }
            "Add" => op("add", BinOp::Add),
            "Sub" => op("sub", BinOp::Sub),
            "Mul" => op("mul", BinOp::Mul),
            "Div" => op("div", BinOp::Div),
            "Rem" => op("rem", BinOp::Rem),
            "Neg" => format!("    fn {name} neg(self) = (-(self as {base})) as {name}\n"),
            "Default" => format!("    fn {name} default() = {base}.default() as {name}\n"),
            _ => {
                diags.emit(
                    Diagnostic::error("E0251", format!("`{}` cannot reuse `{}` of `{}`", name, iface, base))
                        .primary(span, "not a standard interface of the base")
                        .note("reusable: Copy, Eq, Ord, Display, Hash, Clone, Default, Add, Sub, Mul, Div, Rem, Neg")
                        .help(format!("implement it yourself: `extend {} is {} {{ … }}`", name, iface)),
                );
                continue;
            }
        };
        out.push_str(&format!("extend {name} is {iface} {{\n{body}}}\n\n"));
    }
    out
}

//! `@derive(Eq, Ord, Clone, Display, Hash, Default)` on a struct or an enum: the
//! implementations are generated as NEXUL source and parsed like the rest of the module
//! (spec §3.9).

use nxc_syntax::ast::{self, ItemKind, Lit};
use nxc_syntax::{DiagSink, Diagnostic, SourceMap};

const DERIVABLE: &[&str] = &["Eq", "Ord", "Clone", "Display", "Hash", "Default"];

/// Adds the generated `impl` blocks to the module.
pub fn expand(module: &mut ast::Module, sm: &mut SourceMap, file_name: &str, diags: &DiagSink) {
    let mut text = String::new();
    for item in &module.items {
        let Some(attr) = item.attrs.iter().find(|a| a.name.name == "derive") else { continue };
        let (name, generics, shape) = match &item.kind {
            ItemKind::Struct(s) => (s.name.name.clone(), s.generics.iter().map(|g| g.name.clone()).collect::<Vec<_>>(), Shape::Struct(s.fields.iter().map(|f| f.name.name.clone()).collect())),
            ItemKind::Enum(e) => (e.name.name.clone(), e.generics.iter().map(|g| g.name.clone()).collect(), Shape::Enum(e.variants.iter().map(|v| (v.name.name.clone(), v.fields.len())).collect())),
            _ => {
                diags.emit(Diagnostic::error("E0232", "`@derive` applies to structs and enums").primary(attr.span, "misplaced attribute"));
                continue;
            }
        };
        for (arg, span) in &attr.args {
            let Lit::Str(tr) = arg else {
                diags.emit(Diagnostic::error("E0232", "`@derive` takes trait names").primary(*span, "expected a trait name"));
                continue;
            };
            if !DERIVABLE.contains(&tr.as_str()) {
                diags.emit(
                    Diagnostic::error("E0232", format!("`{}` cannot be derived", tr))
                        .primary(*span, "not derivable")
                        .note(format!("derivable traits: {}", DERIVABLE.join(", "))),
                );
                continue;
            }
            let target = if generics.is_empty() { name.clone() } else { format!("{}[{}]", name, generics.iter().map(|g| format!("{}: {}", g, tr)).collect::<Vec<_>>().join(", ")) };
            let body = match tr.as_str() {
                "Eq" => eq(&shape),
                "Ord" => ord(&shape),
                "Clone" => clone(&name, &shape),
                "Display" => display(&name, &shape),
                "Hash" => hash(&name, &shape),
                _ => match default(&name, &shape) {
                    Some(b) => b,
                    None => {
                        diags.emit(
                            Diagnostic::error("E0232", format!("`Default` cannot be derived for `{}`", name))
                                .primary(*span, "the first variant has values")
                                .help("put a variant without values first, or implement `Default` yourself"),
                        );
                        continue;
                    }
                },
            };
            text.push_str(&format!("impl {} for {} {{\n{}}}\n\n", tr, target, body));
        }
    }
    if text.is_empty() {
        return;
    }
    let file = sm.add(format!("{} (derive)", file_name), text);
    let generated = nxc_syntax::parse_file(sm, file, diags);
    module.items.extend(generated.items);
}

enum Shape {
    Struct(Vec<String>),
    /// Variant names and value counts.
    Enum(Vec<(String, usize)>),
}

fn vars(prefix: &str, n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{}{}", prefix, i)).collect()
}

fn pattern(v: &str, names: &[String]) -> String {
    if names.is_empty() {
        v.to_string()
    } else {
        format!("{}({})", v, names.join(", "))
    }
}

fn eq(shape: &Shape) -> String {
    match shape {
        Shape::Struct(fields) => {
            let cond = if fields.is_empty() { "true".to_string() } else { fields.iter().map(|f| format!("self.{} == other.{}", f, f)).collect::<Vec<_>>().join(" and ") };
            format!("    fn eq(self, other: Self) -> bool {{\n        return {}\n    }}\n", cond)
        }
        Shape::Enum(variants) => {
            let mut s = String::from("    fn eq(self, other: Self) -> bool {\n        match self {\n");
            for (v, n) in variants {
                let (a, b) = (vars("a", *n), vars("b", *n));
                let cond = if *n == 0 { "true".to_string() } else { a.iter().zip(&b).map(|(x, y)| format!("{} == {}", x, y)).collect::<Vec<_>>().join(" and ") };
                s.push_str(&format!("            {} => {{\n                match other {{\n                    {} => {{ return {} }}\n", pattern(v, &a), pattern(v, &b), cond));
                if variants.len() > 1 {
                    s.push_str("                    _ => { return false }\n");
                }
                s.push_str("                }\n            }\n");
            }
            s.push_str("        }\n    }\n");
            s
        }
    }
}

fn ord(shape: &Shape) -> String {
    let chain = |pairs: Vec<(String, String)>, indent: &str| -> String {
        let mut s = String::new();
        for (i, (x, y)) in pairs.iter().enumerate() {
            s.push_str(&format!("{}let c{} = {}.cmp({})\n{}if c{} != Ordering.Equal {{ return c{} }}\n", indent, i, x, y, indent, i, i));
        }
        s.push_str(&format!("{}return Ordering.Equal\n", indent));
        s
    };
    match shape {
        Shape::Struct(fields) => {
            let pairs = fields.iter().map(|f| (format!("self.{}", f), format!("other.{}", f))).collect();
            format!("    fn cmp(self, other: Self) -> Ordering {{\n{}    }}\n", chain(pairs, "        "))
        }
        Shape::Enum(variants) => {
            let index = |who: &str| -> String {
                let arms: Vec<String> = variants.iter().enumerate().map(|(i, (v, n))| format!("{} => {}", if *n == 0 { v.clone() } else { format!("{}(..)", v) }, i)).collect();
                format!("        let i_{} = match {} {{\n            {}\n        }}\n", who, who, arms.join("\n            "))
            };
            let mut s = String::from("    fn cmp(self, other: Self) -> Ordering {\n");
            if variants.len() > 1 {
                s.push_str(&index("self"));
                s.push_str(&index("other"));
                s.push_str("        if i_self < i_other { return Ordering.Less }\n        if i_self > i_other { return Ordering.Greater }\n");
            }
            s.push_str("        match self {\n");
            for (v, n) in variants {
                let (a, b) = (vars("a", *n), vars("b", *n));
                s.push_str(&format!("            {} => {{\n                match other {{\n                    {} => {{\n", pattern(v, &a), pattern(v, &b)));
                s.push_str(&chain(a.iter().cloned().zip(b.iter().cloned()).collect(), "                        "));
                s.push_str("                    }\n");
                if variants.len() > 1 {
                    s.push_str("                    _ => { return Ordering.Equal }\n");
                }
                s.push_str("                }\n            }\n");
            }
            s.push_str("        }\n    }\n");
            s
        }
    }
}

fn clone(name: &str, shape: &Shape) -> String {
    match shape {
        Shape::Struct(fields) => {
            let args: Vec<String> = fields.iter().map(|f| format!("{}: self.{}.clone()", f, f)).collect();
            format!("    fn clone(self) -> Self {{\n        return {}({})\n    }}\n", name, args.join(", "))
        }
        Shape::Enum(variants) => {
            let mut s = String::from("    fn clone(self) -> Self {\n        match self {\n");
            for (v, n) in variants {
                let a = vars("a", *n);
                let value = if *n == 0 { format!("Self.{}", v) } else { format!("Self.{}({})", v, a.iter().map(|x| format!("{}.clone()", x)).collect::<Vec<_>>().join(", ")) };
                s.push_str(&format!("            {} => {{ return {} }}\n", pattern(v, &a), value));
            }
            s.push_str("        }\n    }\n");
            s
        }
    }
}

fn display(name: &str, shape: &Shape) -> String {
    match shape {
        Shape::Struct(fields) => {
            let parts: Vec<String> = fields.iter().map(|f| format!("{}: {{self.{}}}", f, f)).collect();
            format!("    fn to_string(self) -> String {{\n        return \"{}({})\"\n    }}\n", name, parts.join(", "))
        }
        Shape::Enum(variants) => {
            let mut s = String::from("    fn to_string(self) -> String {\n        match self {\n");
            for (v, n) in variants {
                let a = vars("a", *n);
                let text = if *n == 0 { v.clone() } else { format!("{}({})", v, a.iter().map(|x| format!("{{{}}}", x)).collect::<Vec<_>>().join(", ")) };
                s.push_str(&format!("            {} => {{ return \"{}\" }}\n", pattern(v, &a), text));
            }
            s.push_str("        }\n    }\n");
            s
        }
    }
}

fn hash(name: &str, shape: &Shape) -> String {
    // FNV-1a style combination of the hashes of the parts.
    let seed: u64 = name.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    let combine = |parts: Vec<String>, indent: &str| -> String {
        let mut s = format!("{}var h: u64 = {}\n", indent, seed);
        for p in parts {
            s.push_str(&format!("{}h = (h ^ {}) *% 0x100000001B3\n", indent, p));
        }
        s.push_str(&format!("{}return h\n", indent));
        s
    };
    match shape {
        Shape::Struct(fields) => {
            let parts = fields.iter().map(|f| format!("self.{}.hash()", f)).collect();
            format!("    fn hash(self) -> u64 {{\n{}    }}\n", combine(parts, "        "))
        }
        Shape::Enum(variants) => {
            let mut s = String::from("    fn hash(self) -> u64 {\n        match self {\n");
            for (i, (v, n)) in variants.iter().enumerate() {
                let a = vars("a", *n);
                let mut parts = vec![format!("{}", i)];
                parts.extend(a.iter().map(|x| format!("{}.hash()", x)));
                s.push_str(&format!("            {} => {{\n{}            }}\n", pattern(v, &a), combine(parts, "                ")));
            }
            s.push_str("        }\n    }\n");
            s
        }
    }
}

fn default(name: &str, shape: &Shape) -> Option<String> {
    match shape {
        Shape::Struct(fields) => {
            let args: Vec<String> = fields.iter().map(|f| format!("{}: Default.default()", f)).collect();
            Some(format!("    fn default() -> Self {{\n        return {}({})\n    }}\n", name, args.join(", ")))
        }
        Shape::Enum(variants) => match variants.first() {
            Some((v, 0)) => Some(format!("    fn default() -> Self {{\n        return Self.{}\n    }}\n", v)),
            _ => None,
        },
    }
}

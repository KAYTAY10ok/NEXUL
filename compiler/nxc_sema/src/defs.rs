//! Program-wide definition tables.

use crate::thir;
use crate::ty::{TyCtx, TyKind, Ty};
use nxc_syntax::ast::{self, Convention};
use nxc_syntax::{FileId, Span};
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub u32);
        impl $name {
            pub fn idx(self) -> usize {
                self.0 as usize
            }
        }
    };
}

id_type!(ModId);
id_type!(AdtId);
id_type!(FnId);
id_type!(ConstId);
id_type!(AliasId);
id_type!(StaticId);

/// Functions provided by the compiler itself (spec §4.10).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    Print,
    Println,
    Eprint,
    Eprintln,
    Panic,
    Assert,
    Unreachable,
}

impl Builtin {
    pub fn from_name(s: &str) -> Option<Builtin> {
        Some(match s {
            "print" => Builtin::Print,
            "println" => Builtin::Println,
            "eprint" => Builtin::Eprint,
            "eprintln" => Builtin::Eprintln,
            "panic" => Builtin::Panic,
            "assert" => Builtin::Assert,
            "unreachable" => Builtin::Unreachable,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Builtin::Print => "print",
            Builtin::Println => "println",
            Builtin::Eprint => "eprint",
            Builtin::Eprintln => "eprintln",
            Builtin::Panic => "panic",
            Builtin::Assert => "assert",
            Builtin::Unreachable => "unreachable",
        }
    }
}

/// What a name refers to at module level.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ItemRef {
    Module(ModId),
    Adt(AdtId),
    Fn(FnId),
    Const(ConstId),
    Alias(AliasId),
    Variant(AdtId, u32),
    Builtin(Builtin),
    Static(StaticId),
}

pub struct ImportEntry {
    pub item: ItemRef,
    pub span: Span,
    pub used: Cell<bool>,
}

pub struct ModuleInfo {
    /// Dotted path, e.g. `std.io` or `geometrie.formes`.
    pub path: String,
    pub file: FileId,
    pub is_std: bool,
    /// Items declared in this module: name -> (item, declaration span, is_pub).
    pub items: HashMap<String, (ItemRef, Span, bool)>,
    /// Names brought in by `import`.
    pub imports: HashMap<String, ImportEntry>,
    pub ast: Option<ast::Module>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum LangItem {
    String,
    Vec,
    Option,
    Result,
}

impl LangItem {
    pub fn from_name(s: &str) -> Option<LangItem> {
        Some(match s {
            "string" => LangItem::String,
            "vec" => LangItem::Vec,
            "option" => LangItem::Option,
            "result" => LangItem::Result,
            _ => return None,
        })
    }
}

#[derive(Default, Clone, Copy)]
pub struct LangItems {
    pub string: Option<AdtId>,
    pub vec: Option<AdtId>,
    pub option: Option<AdtId>,
    pub result: Option<AdtId>,
}

pub struct FieldDef {
    pub name: String,
    pub ty: Ty,
    pub is_pub: bool,
    pub indirect: bool,
    pub span: Span,
}

pub struct VariantDef {
    pub name: String,
    pub fields: Vec<Ty>,
    pub indirect: bool,
    pub span: Span,
}

pub enum AdtKind {
    Struct { fields: Vec<FieldDef> },
    Enum { variants: Vec<VariantDef> },
}

pub struct AdtDef {
    pub name: String,
    pub module: ModId,
    pub span: Span,
    pub is_pub: bool,
    pub generics: Vec<String>,
    pub kind: AdtKind,
    /// Methods and associated functions, by name.
    pub methods: HashMap<String, FnId>,
    pub drop_fn: Option<FnId>,
    pub index_fn: Option<FnId>,
    pub lang: Option<LangItem>,
}

impl AdtDef {
    pub fn is_enum(&self) -> bool {
        matches!(self.kind, AdtKind::Enum { .. })
    }

    pub fn fields(&self) -> &[FieldDef] {
        match &self.kind {
            AdtKind::Struct { fields } => fields,
            AdtKind::Enum { .. } => &[],
        }
    }

    pub fn variants(&self) -> &[VariantDef] {
        match &self.kind {
            AdtKind::Enum { variants } => variants,
            AdtKind::Struct { .. } => &[],
        }
    }

    pub fn variant_index(&self, name: &str) -> Option<u32> {
        self.variants().iter().position(|v| v.name == name).map(|i| i as u32)
    }

    pub fn field_index(&self, name: &str) -> Option<u32> {
        self.fields().iter().position(|f| f.name == name).map(|i| i as u32)
    }
}

pub struct ParamDef {
    pub name: String,
    pub conv: Convention,
    pub ty: Ty,
    pub span: Span,
}

pub enum FnKind {
    User(Rc<ast::FnDecl>),
    Extern { abi: String, link: Option<String> },
    Intrinsic(String),
}

pub struct FnDef {
    pub name: String,
    pub module: ModId,
    pub span: Span,
    pub is_pub: bool,
    /// The type whose `impl` block declares this function.
    pub owner: Option<AdtId>,
    /// Owner generics first, then the function's own generics.
    pub generics: Vec<String>,
    pub n_owner_generics: usize,
    pub params: Vec<ParamDef>,
    pub has_self: bool,
    pub ret: Ty,
    pub is_unsafe: bool,
    pub kind: FnKind,
    pub is_test: bool,
    /// Panics inside report the caller's location (`@track_caller`, std only).
    pub track_caller: bool,
    /// `@export("symbol")`: the function is visible to other languages under this
    /// symbol name, with the C calling convention (spec §8.4).
    pub export: Option<String>,
    /// Code generation hints: `cold`, `inline`, `no_builtins`.
    pub hints: Vec<String>,
}

impl FnDef {
    pub fn is_extern(&self) -> bool {
        matches!(self.kind, FnKind::Extern { .. })
    }

    pub fn intrinsic(&self) -> Option<&str> {
        match &self.kind {
            FnKind::Intrinsic(n) => Some(n),
            _ => None,
        }
    }

    pub fn qualified_name(&self, p: &Program) -> String {
        match self.owner {
            Some(a) => format!("{}.{}", p.adts[a.idx()].name, self.name),
            None => self.name.clone(),
        }
    }
}

pub struct ConstDef {
    pub name: String,
    pub module: ModId,
    pub span: Span,
    pub is_pub: bool,
    pub ty: Ty,
    pub ast: Rc<ast::ConstDecl>,
}

/// `static` and `static var` items (spec §3.9).
pub struct StaticDef {
    pub name: String,
    pub module: ModId,
    pub span: Span,
    pub is_pub: bool,
    pub mutable: bool,
    pub ty: Ty,
    pub ast: Rc<ast::StaticDecl>,
}

/// The initial value of a static variable, computed at compile time.
#[derive(Clone, Debug, PartialEq)]
pub enum StaticValue {
    Scalar(ConstValue),
    /// The null pointer.
    Null,
    /// Elements of an array, tuple or structure (in field order).
    Aggregate(Vec<StaticValue>),
    /// `[value; count]`
    Repeat(Box<StaticValue>, u64),
    /// A fieldless enum variant (its index).
    Variant(u32),
}

pub struct AliasDef {
    pub name: String,
    pub module: ModId,
    pub span: Span,
    pub is_pub: bool,
    pub ty: Ty,
}

/// Compile-time constant values.
#[derive(Clone, Debug, PartialEq)]
pub enum ConstValue {
    Int(i128),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(String),
    Unit,
}

/// Everything known about the program being compiled.
pub struct Program {
    pub tcx: TyCtx,
    pub modules: Vec<ModuleInfo>,
    pub adts: Vec<AdtDef>,
    pub fns: Vec<FnDef>,
    pub consts: Vec<ConstDef>,
    pub aliases: Vec<AliasDef>,
    pub statics: Vec<StaticDef>,
    pub static_values: HashMap<StaticId, StaticValue>,
    pub lang: LangItems,
    /// Functions of the runtime used by the code generator (`@lang("name")` on a
    /// function of the standard library).
    pub lang_fns: HashMap<String, FnId>,
    /// Types of the runtime used by the code generator (`@lang("name")` on a struct or
    /// enum of the standard library other than the prelude types).
    pub lang_adts: HashMap<String, AdtId>,
    /// Target and build mode seen by `@cfg`.
    pub cfg: crate::target::Cfg,
    pub prelude: HashMap<String, ItemRef>,
    pub root: ModId,
    pub main_fn: Option<FnId>,
    pub bodies: HashMap<FnId, thir::Body>,
    pub const_values: HashMap<ConstId, ConstValue>,
}

impl Program {
    pub fn new() -> Program {
        Program {
            tcx: TyCtx::new(),
            modules: Vec::new(),
            adts: Vec::new(),
            fns: Vec::new(),
            consts: Vec::new(),
            aliases: Vec::new(),
            statics: Vec::new(),
            static_values: HashMap::new(),
            lang: LangItems::default(),
            lang_fns: HashMap::new(),
            lang_adts: HashMap::new(),
            cfg: crate::target::Cfg::default(),
            prelude: HashMap::new(),
            root: ModId(0),
            main_fn: None,
            bodies: HashMap::new(),
            const_values: HashMap::new(),
        }
    }

    pub fn adt(&self, id: AdtId) -> &AdtDef {
        &self.adts[id.idx()]
    }

    pub fn func(&self, id: FnId) -> &FnDef {
        &self.fns[id.idx()]
    }

    /// A runtime function of the standard library (`@lang("name")`).
    pub fn lang_fn(&self, name: &str) -> Option<FnId> {
        self.lang_fns.get(name).copied()
    }

    pub fn is_string(&self, t: Ty) -> bool {
        matches!(&*self.tcx.kind(t), TyKind::Adt(id, _) if Some(*id) == self.lang.string)
    }

    pub fn string_ty(&self) -> Ty {
        match self.lang.string {
            Some(s) => self.tcx.adt(s, vec![]),
            None => self.tcx.error,
        }
    }

    /// `Option[t]`
    pub fn option_ty(&self, t: Ty) -> Ty {
        match self.lang.option {
            Some(o) => self.tcx.adt(o, vec![t]),
            None => self.tcx.error,
        }
    }

    /// Human-readable rendering of a type, for diagnostics.
    pub fn ty_str(&self, t: Ty) -> String {
        self.ty_str_with(t, &[])
    }

    /// Like [`ty_str`], naming generic parameters with `names`.
    pub fn ty_str_with(&self, t: Ty, names: &[String]) -> String {
        let tcx = &self.tcx;
        match &*tcx.kind(t) {
            TyKind::Int(i) => i.name().to_string(),
            TyKind::Float(f) => f.name().to_string(),
            TyKind::Bool => "bool".into(),
            TyKind::Char => "char".into(),
            TyKind::Unit => "()".into(),
            TyKind::Never => "Never".into(),
            TyKind::Adt(id, args) => {
                let name = &self.adts[id.idx()].name;
                if args.is_empty() {
                    name.clone()
                } else {
                    let a: Vec<String> = args.iter().map(|&x| self.ty_str_with(x, names)).collect();
                    format!("{}[{}]", name, a.join(", "))
                }
            }
            TyKind::Tuple(elems) => {
                let a: Vec<String> = elems.iter().map(|&x| self.ty_str_with(x, names)).collect();
                if a.len() == 1 {
                    format!("({},)", a[0])
                } else {
                    format!("({})", a.join(", "))
                }
            }
            TyKind::Array(e, n) => format!("[{}; {}]", self.ty_str_with(*e, names), n),
            TyKind::Ptr(e) => format!("Ptr[{}]", self.ty_str_with(*e, names)),
            TyKind::Fn(params, ret) => {
                let a: Vec<String> = params
                    .iter()
                    .map(|&(c, x)| match c {
                        Convention::Borrow => self.ty_str_with(x, names),
                        _ => format!("{} {}", c.keyword(), self.ty_str_with(x, names)),
                    })
                    .collect();
                if *ret == tcx.unit {
                    format!("fn({})", a.join(", "))
                } else {
                    format!("fn({}) -> {}", a.join(", "), self.ty_str_with(*ret, names))
                }
            }
            TyKind::Param(i) => names.get(*i as usize).cloned().unwrap_or_else(|| format!("T{}", i)),
            TyKind::Infer(_) => "_".into(),
            TyKind::IntVar(_) => "{integer}".into(),
            TyKind::FloatVar(_) => "{float}".into(),
            TyKind::Error => "{error}".into(),
        }
    }

    /// Spec §3.7: a type is copyable when it owns no resource. Generic parameters are
    /// treated as resources.
    pub fn is_copy(&self, t: Ty) -> bool {
        let mut visiting = Vec::new();
        self.is_copy_rec(t, &mut visiting)
    }

    fn is_copy_rec(&self, t: Ty, visiting: &mut Vec<Ty>) -> bool {
        match &*self.tcx.kind(t) {
            TyKind::Int(_) | TyKind::Float(_) | TyKind::Bool | TyKind::Char | TyKind::Unit | TyKind::Never => true,
            TyKind::Ptr(_) | TyKind::Fn(..) => true,
            TyKind::IntVar(_) | TyKind::FloatVar(_) | TyKind::Error => true,
            TyKind::Param(_) | TyKind::Infer(_) => false,
            TyKind::Tuple(elems) => elems.iter().all(|&e| self.is_copy_rec(e, visiting)),
            TyKind::Array(e, _) => self.is_copy_rec(*e, visiting),
            TyKind::Adt(id, args) => {
                if visiting.contains(&t) {
                    // Recursive types always contain `indirect` storage.
                    return false;
                }
                let adt = &self.adts[id.idx()];
                if adt.drop_fn.is_some() {
                    return false;
                }
                visiting.push(t);
                let r = match &adt.kind {
                    AdtKind::Struct { fields } => {
                        fields.iter().all(|f| !f.indirect && self.is_copy_rec(self.tcx.subst(f.ty, args), visiting))
                    }
                    AdtKind::Enum { variants } => variants
                        .iter()
                        .all(|v| !v.indirect && v.fields.iter().all(|&f| self.is_copy_rec(self.tcx.subst(f, args), visiting))),
                };
                visiting.pop();
                r
            }
        }
    }

    /// Does dropping a value of this (fully concrete) type do anything?
    pub fn needs_drop(&self, t: Ty) -> bool {
        !self.is_copy(t)
    }
}

impl Default for Program {
    fn default() -> Self {
        Self::new()
    }
}

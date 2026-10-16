//! Abstract syntax tree. It mirrors the grammar of spec chapter 02 and keeps the exact
//! span of every node. No name or type information lives here.

use crate::source::Span;
use crate::token::FormatSpec;

#[derive(Clone, Debug, PartialEq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Module {
    pub decl: Option<(Vec<Ident>, Span)>,
    pub imports: Vec<Import>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub struct Import {
    pub path: Vec<Ident>,
    pub kind: ImportKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ImportKind {
    /// `import a.b` or `import a.b as c`
    Module { alias: Option<Ident> },
    /// `import a.b.{x, y as z}`
    Items(Vec<(Ident, Option<Ident>)>),
}

#[derive(Clone, Debug)]
pub struct Attr {
    pub name: Ident,
    pub args: Vec<(Lit, Span)>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub attrs: Vec<Attr>,
    pub is_pub: bool,
    pub kind: ItemKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ItemKind {
    Fn(FnDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Impl(ImplDecl),
    Const(ConstDecl),
    TypeAlias(TypeAliasDecl),
    ExternFn(ExternFnDecl),
    Static(StaticDecl),
    Trait(TraitDecl),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Convention {
    /// Default: read-only borrow.
    Borrow,
    Inout,
    Owned,
}

impl Convention {
    pub fn keyword(self) -> &'static str {
        match self {
            Convention::Borrow => "",
            Convention::Inout => "inout",
            Convention::Owned => "owned",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ParamKind {
    SelfParam,
    Named { name: Ident, ty: TypeExpr },
}

#[derive(Clone, Debug)]
pub struct Param {
    pub conv: Convention,
    pub kind: ParamKind,
    pub span: Span,
}

/// Trait bounds on a type parameter: `T: Ord + Display` (inline or in `where`).
#[derive(Clone, Debug)]
pub struct BoundDecl {
    pub param: Ident,
    pub traits: Vec<TypeExpr>,
}

#[derive(Clone, Debug)]
pub struct FnDecl {
    pub name: Ident,
    pub generics: Vec<Ident>,
    /// Bounds written `[T: Trait]` or `where T: Trait`.
    pub bounds: Vec<BoundDecl>,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub body: Option<Block>,
    pub is_unsafe: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct FieldDecl {
    pub is_pub: bool,
    pub indirect: bool,
    pub name: Ident,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct StructDecl {
    pub name: Ident,
    pub generics: Vec<Ident>,
    pub fields: Vec<FieldDecl>,
}

#[derive(Clone, Debug)]
pub struct VariantDecl {
    pub indirect: bool,
    pub name: Ident,
    pub fields: Vec<TypeExpr>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct EnumDecl {
    pub name: Ident,
    pub generics: Vec<Ident>,
    pub variants: Vec<VariantDecl>,
}

/// `impl Type { ... }` or `impl Trait for Type { ... }`; for a generic type, the
/// parameters may be listed with bounds: `impl Display for Pair[A: Display, B: Display]`.
#[derive(Clone, Debug)]
pub struct ImplDecl {
    pub ty: TypeExpr,
    pub trait_: Option<TypeExpr>,
    pub bounds: Vec<BoundDecl>,
    pub items: Vec<Item>,
}

/// `trait Name: Super1 + Super2 { fn method(self) -> T  fn with_default(self) { ... } }`
#[derive(Clone, Debug)]
pub struct TraitDecl {
    pub name: Ident,
    pub supertraits: Vec<TypeExpr>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub struct ConstDecl {
    pub name: Ident,
    pub ty: TypeExpr,
    pub value: Expr,
}

/// `static NAME: T = value` (immutable) or `static var NAME: T = value` (mutable,
/// accessed only in `unsafe` code).
#[derive(Clone, Debug)]
pub struct StaticDecl {
    pub name: Ident,
    pub mutable: bool,
    pub ty: TypeExpr,
    pub value: Expr,
}

#[derive(Clone, Debug)]
pub struct TypeAliasDecl {
    pub name: Ident,
    pub ty: TypeExpr,
}

#[derive(Clone, Debug)]
pub struct ExternFnDecl {
    pub abi: String,
    pub abi_span: Span,
    pub name: Ident,
    pub params: Vec<(Ident, TypeExpr)>,
    pub ret: Option<TypeExpr>,
}

// ---------------------------------------------------------------------------
// Types

#[derive(Clone, Debug)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TypeExprKind {
    /// `a.b.C[args]`
    Path { segments: Vec<Ident>, args: Vec<TypeExpr> },
    SelfType,
    Array(Box<TypeExpr>, Box<Expr>),
    Unit,
    Tuple(Vec<TypeExpr>),
    Fn(Vec<(Convention, TypeExpr)>, Option<Box<TypeExpr>>),
    /// `dyn Trait`: a value of any type implementing the trait (dynamic dispatch).
    Dyn(Box<TypeExpr>),
    Error,
}

// ---------------------------------------------------------------------------
// Statements

#[derive(Clone, Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    Compound(BinOp),
}

#[derive(Clone, Debug)]
pub enum ForIter {
    Range { start: Expr, end: Expr, inclusive: bool },
    /// `for x in v` or, with `mutable`, `for x in &v` (elements modifiable in place).
    Expr { expr: Expr, mutable: bool },
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Let { pat: Pattern, ty: Option<TypeExpr>, init: Expr },
    Var { name: Ident, ty: Option<TypeExpr>, init: Option<Expr> },
    Assign { op: AssignOp, op_span: Span, target: Expr, value: Expr },
    Expr(Expr),
    While { label: Option<Ident>, cond: Expr, body: Block },
    For { label: Option<Ident>, pat: Pattern, iter: ForIter, body: Block },
}

// ---------------------------------------------------------------------------
// Expressions

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    AddWrap,
    SubWrap,
    MulWrap,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        use BinOp::*;
        match self {
            Add => "+",
            Sub => "-",
            Mul => "*",
            Div => "/",
            Rem => "%",
            AddWrap => "+%",
            SubWrap => "-%",
            MulWrap => "*%",
            BitAnd => "&",
            BitOr => "|",
            BitXor => "^",
            Shl => "<<",
            Shr => ">>",
            Eq => "==",
            Ne => "!=",
            Lt => "<",
            Le => "<=",
            Gt => ">",
            Ge => ">=",
            And => "and",
            Or => "or",
        }
    }

    pub fn is_comparison(self) -> bool {
        use BinOp::*;
        matches!(self, Eq | Ne | Lt | Le | Gt | Ge)
    }

    pub fn is_arithmetic(self) -> bool {
        use BinOp::*;
        matches!(self, Add | Sub | Mul | Div | Rem | AddWrap | SubWrap | MulWrap)
    }

    pub fn is_bitwise(self) -> bool {
        use BinOp::*;
        matches!(self, BitAnd | BitOr | BitXor)
    }

    pub fn is_shift(self) -> bool {
        matches!(self, BinOp::Shl | BinOp::Shr)
    }

    pub fn is_logical(self) -> bool {
        matches!(self, BinOp::And | BinOp::Or)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Lit {
    Int(u128),
    Float(f64),
    Char(char),
    Str(String),
    Bool(bool),
}

#[derive(Clone, Debug)]
pub struct Arg {
    pub label: Option<Ident>,
    /// Span of the `&` marking an `inout` argument.
    pub inout: Option<Span>,
    pub expr: Expr,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum StrPartAst {
    Text(String),
    Interp { expr: Box<Expr>, spec: Option<FormatSpec> },
}

#[derive(Clone, Debug)]
pub enum Cond {
    Expr(Box<Expr>),
    /// `if let P = e` or, with the flag, `if let P = &e` (bindings modify `e` in place).
    Let(Box<Pattern>, Box<Expr>, bool),
}

#[derive(Clone, Debug)]
pub struct Arm {
    pub pat: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Lit(Lit),
    /// String literal with interpolations.
    Interp(Vec<StrPartAst>),
    Ident(String),
    /// Edition 1: `.Name`, a variant of the type the context expects (ADR 0010).
    Variant(Ident),
    SelfValue,
    SelfType,
    Unit,
    Paren(Box<Expr>),
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    ArrayRepeat(Box<Expr>, Box<Expr>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Cast(Box<Expr>, TypeExpr),
    Field(Box<Expr>, Ident),
    TupleField(Box<Expr>, u32, Span),
    Call(Box<Expr>, Vec<Arg>),
    /// `a[i]` or a generic instantiation `Vec[i32]`.
    Index(Box<Expr>, Vec<Expr>),
    Try(Box<Expr>),
    /// `value else fallback` (edition 1): the value of an `Option` or a `Result`, else the
    /// fallback (a value, or a block that leaves) when it is empty or an error.
    Fallback(Box<Expr>, Box<Expr>),
    Move(Box<Expr>),
    Return(Option<Box<Expr>>),
    Break(Option<Ident>),
    Continue(Option<Ident>),
    If { cond: Cond, then: Block, else_: Option<Box<Expr>> },
    /// `match e { ... }` or, with `mutable`, `match &e { ... }`.
    Match { scrutinee: Box<Expr>, arms: Vec<Arm>, mutable: bool },
    /// A block appearing as a branch body (`else { ... }`, `=> { ... }`).
    Block(Block),
    Unsafe(Block),
    /// A type written where an expression was parsed (generic argument such as `fn(i32)`).
    Type(TypeExpr),
    /// `fn(x, y: i32) -> T { ... }` or `fn(x) => expr`: an anonymous function that may
    /// use the variables around it (spec §4.12).
    Closure(Box<ClosureExpr>),
    Error,
}

#[derive(Clone, Debug)]
pub struct ClosureParam {
    pub conv: Convention,
    pub name: Ident,
    pub ty: Option<TypeExpr>,
}

#[derive(Clone, Debug)]
pub enum ClosureBody {
    Block(Block),
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub struct ClosureExpr {
    pub params: Vec<ClosureParam>,
    pub ret: Option<TypeExpr>,
    pub body: ClosureBody,
}

// ---------------------------------------------------------------------------
// Patterns

#[derive(Clone, Debug)]
pub struct Pattern {
    pub kind: PatKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PatLit {
    Int(i128),
    Char(char),
    Str(String),
    Bool(bool),
}

#[derive(Clone, Debug)]
pub enum PatArg {
    Positional(Pattern),
    Named(Ident, Pattern),
    Rest(Span),
}

#[derive(Clone, Debug)]
pub enum PatKind {
    Wild,
    /// A binding, or a fieldless variant / constant (decided by name resolution).
    Ident(String),
    Lit(PatLit),
    Range(PatLit, PatLit, bool),
    Tuple(Vec<Pattern>),
    /// `Path(args)`: an enum variant or a struct pattern.
    Ctor { path: Vec<Ident>, args: Vec<PatArg> },
    /// `a.b.C` without arguments (qualified variant or constant).
    Path(Vec<Ident>),
    Or(Vec<Pattern>),
    Error,
}

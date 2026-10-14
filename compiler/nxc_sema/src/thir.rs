//! THIR: the typed tree produced by the type checker.
//!
//! Every name is resolved, every expression carries its (fully inferred) type, and
//! implicit operations are explicit: method calls name their target function, the
//! parameter convention of every argument is recorded, struct literals list their fields
//! in declaration order.

use crate::defs::{AdtId, Builtin, ConstId, ConstValue, FnId, StaticId};
use crate::ty::Ty;
use nxc_syntax::ast::{BinOp, Convention, UnOp};
use nxc_syntax::token::FormatSpec;
use nxc_syntax::Span;

pub type LocalId = u32;
pub type LoopId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalKind {
    /// A function parameter with its passing convention.
    Param(Convention),
    /// `let` binding (immutable, owns its value).
    Let,
    /// `var` binding (mutable, owns its value).
    Var,
    /// A read-only alias of a place: `for` element or pattern binding on a place.
    Ref,
    /// A modifiable alias of a place: binding of `match &x`, `if let ... = &x`, `for e in &v`.
    RefMut,
    /// Compiler-introduced.
    Hidden,
}

#[derive(Clone, Debug)]
pub struct LocalDecl {
    pub name: String,
    pub ty: Ty,
    pub kind: LocalKind,
    pub span: Span,
}

impl LocalDecl {
    pub fn is_mutable(&self) -> bool {
        matches!(self.kind, LocalKind::Var | LocalKind::RefMut | LocalKind::Param(Convention::Inout) | LocalKind::Param(Convention::Owned))
    }

    /// Is this local an alias of storage owned elsewhere (passed by reference)?
    pub fn is_ref(&self) -> bool {
        matches!(self.kind, LocalKind::Ref | LocalKind::RefMut | LocalKind::Param(Convention::Borrow) | LocalKind::Param(Convention::Inout))
    }
}

#[derive(Clone, Debug)]
pub struct Body {
    pub fn_id: FnId,
    pub locals: Vec<LocalDecl>,
    pub params: Vec<LocalId>,
    pub block: Block,
    pub ret_ty: Ty,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    /// Final expression giving the block its value (branch bodies used as values).
    pub tail: Option<Box<Expr>>,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum IterKind {
    Array(u64),
    /// A type with an `@index` method (`Vec`): length field index and index function.
    Indexed { len_field: u32, index_fn: FnId, type_args: Vec<Ty> },
    Str,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Let { pat: Pat, init: Expr },
    Var { local: LocalId, init: Option<Expr> },
    Assign { place: Expr, value: Expr },
    CompoundAssign { op: BinOp, place: Expr, value: Expr },
    Expr(Expr),
    While { id: LoopId, cond: Expr, body: Block },
    ForRange { id: LoopId, var: Option<LocalId>, start: Expr, end: Expr, inclusive: bool, body: Block },
    ForEach { id: LoopId, pat: Pat, iter: Expr, kind: IterKind, elem_ty: Ty, body: Block, mutable: bool },
}

#[derive(Clone, Debug)]
pub enum Callee {
    Fn(FnId, Vec<Ty>),
    Builtin(Builtin),
    /// Methods and associated constants of primitive types and `Ptr` (by name).
    Prim(PrimOp),
    Indirect(Box<Expr>),
}

/// Operations implemented directly by the compiler on primitive types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimOp {
    // Integers
    IntAbs,
    IntMin,
    IntMax,
    IntPow,
    IntLeadingZeros,
    IntTrailingZeros,
    IntCountOnes,
    IntRotateLeft,
    IntRotateRight,
    IntSwapBytes,
    // Floats
    FloatUnary(&'static str),
    FloatBinary(&'static str),
    FloatIsNan,
    FloatIsInfinite,
    FloatIsFinite,
    /// The IEEE 754 bit pattern (`u32` for `f32`, `u64` for `f64`).
    FloatToBits,
    // Characters
    CharPredicate(&'static str),
    CharToUpper,
    CharToLower,
    CharLenUtf8,
    // Formatting
    ToString,
    // Pointers
    PtrNull,
    PtrIsNull,
    PtrOffset,
    /// `p.sub(n)`: the address `n` elements before `p`.
    PtrSub,
    /// `Ptr[T].from_addr(address)`
    PtrFromAddr,
    PtrRead,
    PtrWrite,
    PtrCast,
    PtrAddr,
}

#[derive(Clone, Debug)]
pub struct CallArg {
    pub conv: Convention,
    pub expr: Expr,
}

#[derive(Clone, Debug)]
pub enum InterpPart {
    Text(String),
    Value(Expr, Option<FormatSpec>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TryKind {
    Option,
    Result,
}

#[derive(Clone, Debug)]
pub struct Arm {
    pub pat: Pat,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Lit(ConstValue),
    Local(LocalId),
    Const(ConstId),
    /// A `static` variable (a place).
    Static(StaticId),
    FnRef(FnId),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Cast(Box<Expr>),
    /// Field `n` of a struct or tuple.
    Field(Box<Expr>, u32),
    /// Built-in array indexing.
    Index(Box<Expr>, Box<Expr>),
    /// Indexing through an `@index` method (`Vec`).
    IndexFn { base: Box<Expr>, index: Box<Expr>, index_fn: FnId, type_args: Vec<Ty> },
    Call { callee: Callee, args: Vec<CallArg> },
    Adt { adt: AdtId, variant: Option<u32>, type_args: Vec<Ty>, fields: Vec<Expr> },
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    ArrayRepeat(Box<Expr>, u64),
    If { cond: Box<Expr>, then: Block, else_: Option<Box<Expr>> },
    IfLet { pat: Pat, scrutinee: Box<Expr>, then: Block, else_: Option<Box<Expr>>, mutable: bool },
    Match { scrutinee: Box<Expr>, arms: Vec<Arm>, mutable: bool },
    Block(Block),
    Return(Option<Box<Expr>>),
    Break(LoopId),
    Continue(LoopId),
    Try(Box<Expr>, TryKind),
    Move(Box<Expr>),
    Interp(Vec<InterpPart>),
    /// An anonymous function: its code (`f`, whose first parameters receive the
    /// captured variables) and the variables it uses, with "modified" flags.
    Closure { f: FnId, captures: Vec<(LocalId, bool)> },
    /// A function value passed where an anonymous function is accepted.
    FnToClosure(Box<Expr>),
    /// A value moved to the heap as a `dyn Trait` (the expression's type).
    ToDyn(Box<Expr>),
    Error,
}

impl Expr {
    /// Is this expression a place (it designates existing storage)?
    pub fn is_place(&self) -> bool {
        match &self.kind {
            ExprKind::Local(_) | ExprKind::Static(_) => true,
            ExprKind::Field(b, _) | ExprKind::Index(b, _) => b.is_place(),
            ExprKind::IndexFn { base, .. } => base.is_place(),
            _ => false,
        }
    }

    /// The static variable at the root of a place expression.
    pub fn static_root(&self) -> Option<StaticId> {
        match &self.kind {
            ExprKind::Static(s) => Some(*s),
            ExprKind::Field(b, _) | ExprKind::Index(b, _) => b.static_root(),
            ExprKind::IndexFn { base, .. } => base.static_root(),
            _ => None,
        }
    }

    /// The local at the root of a place expression.
    pub fn place_root(&self) -> Option<LocalId> {
        match &self.kind {
            ExprKind::Local(l) => Some(*l),
            ExprKind::Field(b, _) | ExprKind::Index(b, _) => b.place_root(),
            ExprKind::IndexFn { base, .. } => base.place_root(),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Pat {
    pub kind: PatKind,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum PatKind {
    Wild,
    Binding(LocalId),
    Lit(ConstValue),
    Range(ConstValue, ConstValue, bool),
    Tuple(Vec<Pat>),
    Variant { adt: AdtId, variant: u32, type_args: Vec<Ty>, fields: Vec<Pat> },
    /// Struct pattern with one sub-pattern per field, in declaration order.
    Struct { adt: AdtId, type_args: Vec<Ty>, fields: Vec<Pat> },
    Or(Vec<Pat>),
    Error,
}

impl Pat {
    pub fn bindings(&self, out: &mut Vec<LocalId>) {
        match &self.kind {
            PatKind::Binding(l) => out.push(*l),
            PatKind::Tuple(ps) | PatKind::Or(ps) => ps.iter().for_each(|p| p.bindings(out)),
            PatKind::Variant { fields, .. } | PatKind::Struct { fields, .. } => fields.iter().for_each(|p| p.bindings(out)),
            _ => {}
        }
    }

    /// Can this pattern fail to match?
    pub fn is_refutable(&self) -> bool {
        match &self.kind {
            PatKind::Wild | PatKind::Binding(_) | PatKind::Error => false,
            PatKind::Tuple(ps) => ps.iter().any(|p| p.is_refutable()),
            PatKind::Struct { fields, .. } => fields.iter().any(|p| p.is_refutable()),
            _ => true,
        }
    }
}

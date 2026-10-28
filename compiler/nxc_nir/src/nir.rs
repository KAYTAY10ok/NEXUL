//! NIR — NEXUL Intermediate Representation (spec §10.2).
//!
//! A function body is a list of typed locals and a control-flow graph of basic blocks.
//! Every implicit operation of the source is explicit here: temporaries, drops,
//! bounds checks and argument passing by reference.

use nxc_sema::defs::{AdtId, Builtin, ConstValue, FnId, StaticId};
use nxc_sema::thir::PrimOp;
use nxc_sema::ty::Ty;
use nxc_syntax::ast::{BinOp, UnOp};
use nxc_syntax::token::FormatSpec;
use nxc_syntax::Span;
use std::fmt;

pub type Local = u32;
pub type BlockId = u32;

/// Local 0 is the return place.
pub const RETURN: Local = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalKind {
    Return,
    /// A parameter; `by_ref` parameters have type `Ptr[T]`.
    Arg { by_ref: bool },
    User,
    Temp,
    DropFlag,
}

#[derive(Clone, Debug)]
pub struct LocalDecl {
    pub ty: Ty,
    pub name: Option<String>,
    pub kind: LocalKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Proj {
    /// Through a `Ptr[T]` local.
    Deref,
    /// Field of a struct or tuple (indirect fields are dereferenced implicitly).
    Field(u32),
    /// Field `i` of enum variant `v`.
    VariantField(u32, u32),
    /// Element of a fixed array; the local holds an in-bounds `usize`.
    Index(Local),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub local: Local,
    pub proj: Vec<Proj>,
}

impl Place {
    pub fn local(l: Local) -> Place {
        Place { local: l, proj: Vec::new() }
    }
    pub fn project(&self, p: Proj) -> Place {
        let mut x = self.clone();
        x.proj.push(p);
        x
    }
    pub fn is_whole_local(&self) -> bool {
        self.proj.is_empty()
    }
}

#[derive(Clone, Debug)]
pub enum Operand {
    Copy(Place),
    Move(Place),
    Const(ConstValue, Ty),
    FnRef(FnId),
}

#[derive(Clone, Debug)]
pub enum AggKind {
    Tuple,
    Array,
    Adt { adt: AdtId, variant: Option<u32>, type_args: Vec<Ty> },
}

#[derive(Clone, Debug)]
pub enum Rvalue {
    Use(Operand),
    /// Address of a place (type `Ptr[T]`).
    Ref(Place),
    Binary(BinOp, Operand, Operand),
    Unary(UnOp, Operand),
    /// Checked conversion (spec §3.6) to the type of the destination.
    Cast(Operand, Ty),
    Aggregate(AggKind, Vec<Operand>),
    Repeat(Operand, u64),
    Discriminant(Place),
    /// Address of a static variable (type `Ptr[T]`).
    StaticAddr(StaticId),
    /// An anonymous function value: its code and an environment holding the addresses
    /// of the captured variables (pointers, in order).
    /// An anonymous function: its code and the addresses of its captured variables. With
    /// `heap`, the array of addresses is allocated on the heap (a task launched by
    /// `std.tache`, which outlives the frame that creates it, frees it, ADR 0013).
    Closure { f: FnId, env: Vec<Operand>, heap: bool },
    /// A function pointer used as an anonymous function value.
    FnToClosure(Operand),
    /// A value of the given type moved to the heap as a `dyn Trait`.
    ToDyn(Operand, Ty),
}

/// Functions of the C runtime called directly by generated code.
#[derive(Clone, Debug, PartialEq)]
pub enum RtFn {
    StrEmpty,
    StrPushStr,
    StrConcat,
    StrEq,
    StrCmp,
    StrDecodeChar,
    Print { newline: bool, stderr: bool },
    PanicStr,
    PanicMsg(String),
    /// Appends a formatted value to a string: (dest: Ptr[String], value, spec).
    Fmt { spec: Option<FormatSpec> },
}

#[derive(Clone, Debug)]
pub enum Callee {
    Fn(FnId, Vec<Ty>),
    Builtin(Builtin),
    Prim(PrimOp),
    Indirect(Operand),
    Runtime(RtFn),
}

#[derive(Clone, Debug)]
pub enum BoxRef {
    Field(u32),
    Variant(u32),
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Assign(Place, Rvalue),
    /// Destroys the value in the place (no-op for copyable types).
    Drop(Place),
    /// Scope-end destruction of a local; resolved by drop elaboration.
    DropIfInit(Local),
    /// The local's value has been taken apart by a pattern: it must not be dropped.
    MarkMoved(Local),
    /// `dest = index` as `usize`, panicking unless `0 <= index < len`.
    BoundsCheck { index: Operand, len: Operand, dest: Local },
    /// Frees the heap storage of an `indirect` field or variant without dropping its
    /// content (which has been moved out).
    FreeBox(Place, BoxRef),
    Nop,
}

#[derive(Clone, Debug)]
pub struct Statement {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Terminator {
    Goto(BlockId),
    /// Jumps to the target whose value equals `discr`, else to `otherwise`.
    Switch { discr: Operand, targets: Vec<(u128, BlockId)>, otherwise: BlockId },
    Call { callee: Callee, args: Vec<Operand>, dest: Option<Place>, target: Option<BlockId> },
    Return,
    Unreachable,
}

#[derive(Clone, Debug)]
pub struct BasicBlock {
    pub stmts: Vec<Statement>,
    pub term: Terminator,
    pub term_span: Span,
}

#[derive(Clone, Debug)]
pub struct Body {
    pub fn_id: FnId,
    pub locals: Vec<LocalDecl>,
    pub arg_count: usize,
    pub blocks: Vec<BasicBlock>,
    pub span: Span,
}

impl Body {
    pub fn successors(&self, b: BlockId) -> Vec<BlockId> {
        match &self.blocks[b as usize].term {
            Terminator::Goto(t) => vec![*t],
            Terminator::Switch { targets, otherwise, .. } => {
                let mut v: Vec<BlockId> = targets.iter().map(|(_, t)| *t).collect();
                v.push(*otherwise);
                v
            }
            Terminator::Call { target, .. } => target.iter().copied().collect(),
            Terminator::Return | Terminator::Unreachable => vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// Textual form, for debugging (`nxc build --emit nir`).

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("_{}", self.local);
        for p in &self.proj {
            s = match p {
                Proj::Deref => format!("(*{})", s),
                Proj::Field(i) => format!("{}.{}", s, i),
                Proj::VariantField(v, i) => format!("({} as #{}).{}", s, v, i),
                Proj::Index(l) => format!("{}[_{}]", s, l),
            };
        }
        f.write_str(&s)
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operand::Copy(p) => write!(f, "copy {}", p),
            Operand::Move(p) => write!(f, "move {}", p),
            Operand::Const(c, _) => write!(f, "const {:?}", c),
            Operand::FnRef(id) => write!(f, "fn#{}", id.0),
        }
    }
}

pub fn pretty(body: &Body) -> String {
    let mut out = String::new();
    for (i, l) in body.locals.iter().enumerate() {
        out.push_str(&format!("  let _{}: ty#{} // {:?} {}\n", i, l.ty.0, l.kind, l.name.clone().unwrap_or_default()));
    }
    for (i, b) in body.blocks.iter().enumerate() {
        out.push_str(&format!("  bb{}:\n", i));
        for s in &b.stmts {
            let t = match &s.kind {
                StmtKind::Assign(p, rv) => format!("{} = {}", p, match rv {
                    Rvalue::Use(o) => o.to_string(),
                    Rvalue::Ref(p) => format!("&{}", p),
                    Rvalue::Binary(op, a, b) => format!("{} {} {}", a, op.symbol(), b),
                    Rvalue::Unary(op, a) => format!("{:?} {}", op, a),
                    Rvalue::Cast(a, t) => format!("{} as ty#{}", a, t.0),
                    Rvalue::Aggregate(k, ops) => format!("{:?}({})", k, ops.iter().map(|o| o.to_string()).collect::<Vec<_>>().join(", ")),
                    Rvalue::Repeat(o, n) => format!("[{}; {}]", o, n),
                    Rvalue::Discriminant(p) => format!("discriminant({})", p),
                    Rvalue::StaticAddr(st) => format!("&static#{}", st.0),
                    Rvalue::Closure { f, env, heap } => format!("closure{} fn#{}({})", if *heap { " heap" } else { "" }, f.0, env.iter().map(|o| o.to_string()).collect::<Vec<_>>().join(", ")),
                    Rvalue::FnToClosure(o) => format!("closure_of({})", o),
                    Rvalue::ToDyn(o, t) => format!("to_dyn({}: ty#{})", o, t.0),
                }),
                StmtKind::Drop(p) => format!("drop({})", p),
                StmtKind::DropIfInit(l) => format!("drop_if_init(_{})", l),
                StmtKind::MarkMoved(l) => format!("mark_moved(_{})", l),
                StmtKind::BoundsCheck { index, len, dest } => format!("_{} = bounds_check({}, {})", dest, index, len),
                StmtKind::FreeBox(p, b) => format!("free_box({}, {:?})", p, b),
                StmtKind::Nop => "nop".into(),
            };
            out.push_str(&format!("    {}\n", t));
        }
        let t = match &b.term {
            Terminator::Goto(t) => format!("goto bb{}", t),
            Terminator::Switch { discr, targets, otherwise } => format!(
                "switch {} [{}] otherwise bb{}",
                discr,
                targets.iter().map(|(v, t)| format!("{} => bb{}", v, t)).collect::<Vec<_>>().join(", "),
                otherwise
            ),
            Terminator::Call { callee, args, dest, target } => format!(
                "{}call {:?}({}) -> {}",
                dest.as_ref().map(|d| format!("{} = ", d)).unwrap_or_default(),
                callee,
                args.iter().map(|o| o.to_string()).collect::<Vec<_>>().join(", "),
                target.map(|t| format!("bb{}", t)).unwrap_or_else(|| "!".into())
            ),
            Terminator::Return => "return".into(),
            Terminator::Unreachable => "unreachable".into(),
        };
        out.push_str(&format!("    {}\n", t));
    }
    out
}

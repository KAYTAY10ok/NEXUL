//! Types: interned representation, display and substitution.

use crate::defs::{AdtId, TraitId};
use nxc_syntax::ast::Convention;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// An interned type. Equality of `Ty` values is structural equality of types.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ty(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum IntTy {
    I8,
    I16,
    I32,
    I64,
    Isize,
    U8,
    U16,
    U32,
    U64,
    Usize,
}

impl IntTy {
    pub const ALL: [IntTy; 10] =
        [IntTy::I8, IntTy::I16, IntTy::I32, IntTy::I64, IntTy::Isize, IntTy::U8, IntTy::U16, IntTy::U32, IntTy::U64, IntTy::Usize];

    pub fn name(self) -> &'static str {
        match self {
            IntTy::I8 => "i8",
            IntTy::I16 => "i16",
            IntTy::I32 => "i32",
            IntTy::I64 => "i64",
            IntTy::Isize => "isize",
            IntTy::U8 => "u8",
            IntTy::U16 => "u16",
            IntTy::U32 => "u32",
            IntTy::U64 => "u64",
            IntTy::Usize => "usize",
        }
    }

    pub fn signed(self) -> bool {
        matches!(self, IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64 | IntTy::Isize)
    }

    /// Width in bits. `isize`/`usize` are 64 bits for range checks at compile time on
    /// 64-bit targets; the C backend uses the real target width.
    pub fn bits(self) -> u32 {
        match self {
            IntTy::I8 | IntTy::U8 => 8,
            IntTy::I16 | IntTy::U16 => 16,
            IntTy::I32 | IntTy::U32 => 32,
            IntTy::I64 | IntTy::U64 | IntTy::Isize | IntTy::Usize => 64,
        }
    }

    pub fn min(self) -> i128 {
        if self.signed() {
            -(1i128 << (self.bits() - 1))
        } else {
            0
        }
    }

    pub fn max(self) -> i128 {
        if self.signed() {
            (1i128 << (self.bits() - 1)) - 1
        } else {
            (1i128 << self.bits()) - 1
        }
    }

    pub fn from_name(s: &str) -> Option<IntTy> {
        IntTy::ALL.iter().copied().find(|t| t.name() == s)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FloatTy {
    F32,
    F64,
}

impl FloatTy {
    pub fn name(self) -> &'static str {
        match self {
            FloatTy::F32 => "f32",
            FloatTy::F64 => "f64",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TyKind {
    Int(IntTy),
    Float(FloatTy),
    Bool,
    Char,
    Unit,
    Never,
    Adt(AdtId, Vec<Ty>),
    Tuple(Vec<Ty>),
    Array(Ty, u64),
    Ptr(Ty),
    Fn(Vec<(Convention, Ty)>, Ty),
    /// A function parameter that accepts anonymous functions using the variables of
    /// their caller (code and environment); it cannot be stored or returned.
    Closure(Vec<(Convention, Ty)>, Ty),
    /// `dyn Trait`: a value of any type implementing the trait, on the heap.
    Dyn(TraitId),
    /// Generic parameter number `n` of the current definition.
    Param(u32),
    /// Inference variable (only meaningful inside the function being checked).
    Infer(u32),
    IntVar(u32),
    FloatVar(u32),
    Error,
}

struct Interner {
    kinds: Vec<Rc<TyKind>>,
    map: HashMap<Rc<TyKind>, Ty>,
}

/// The type context: interner and well-known types.
pub struct TyCtx {
    inner: RefCell<Interner>,
    pub unit: Ty,
    pub bool: Ty,
    pub char: Ty,
    pub never: Ty,
    pub error: Ty,
    pub f32: Ty,
    pub f64: Ty,
    pub i8: Ty,
    pub i16: Ty,
    pub i32: Ty,
    pub i64: Ty,
    pub isize: Ty,
    pub u8: Ty,
    pub u16: Ty,
    pub u32: Ty,
    pub u64: Ty,
    pub usize: Ty,
}

impl Default for TyCtx {
    fn default() -> Self {
        Self::new()
    }
}

impl TyCtx {
    pub fn new() -> TyCtx {
        let mut it = Interner { kinds: Vec::new(), map: HashMap::new() };
        let mut mk = |k: TyKind| {
            let rc = Rc::new(k);
            let t = Ty(it.kinds.len() as u32);
            it.kinds.push(rc.clone());
            it.map.insert(rc, t);
            t
        };
        let unit = mk(TyKind::Unit);
        let bool = mk(TyKind::Bool);
        let char = mk(TyKind::Char);
        let never = mk(TyKind::Never);
        let error = mk(TyKind::Error);
        let f32 = mk(TyKind::Float(FloatTy::F32));
        let f64 = mk(TyKind::Float(FloatTy::F64));
        let i8 = mk(TyKind::Int(IntTy::I8));
        let i16 = mk(TyKind::Int(IntTy::I16));
        let i32 = mk(TyKind::Int(IntTy::I32));
        let i64 = mk(TyKind::Int(IntTy::I64));
        let isize = mk(TyKind::Int(IntTy::Isize));
        let u8 = mk(TyKind::Int(IntTy::U8));
        let u16 = mk(TyKind::Int(IntTy::U16));
        let u32 = mk(TyKind::Int(IntTy::U32));
        let u64 = mk(TyKind::Int(IntTy::U64));
        let usize = mk(TyKind::Int(IntTy::Usize));
        TyCtx {
            inner: RefCell::new(it),
            unit,
            bool,
            char,
            never,
            error,
            f32,
            f64,
            i8,
            i16,
            i32,
            i64,
            isize,
            u8,
            u16,
            u32,
            u64,
            usize,
        }
    }

    pub fn mk(&self, k: TyKind) -> Ty {
        let mut it = self.inner.borrow_mut();
        if let Some(&t) = it.map.get(&k) {
            return t;
        }
        let rc = Rc::new(k);
        let t = Ty(it.kinds.len() as u32);
        it.kinds.push(rc.clone());
        it.map.insert(rc, t);
        t
    }

    pub fn kind(&self, t: Ty) -> Rc<TyKind> {
        self.inner.borrow().kinds[t.0 as usize].clone()
    }

    pub fn int(&self, i: IntTy) -> Ty {
        self.mk(TyKind::Int(i))
    }

    pub fn float(&self, f: FloatTy) -> Ty {
        match f {
            FloatTy::F32 => self.f32,
            FloatTy::F64 => self.f64,
        }
    }

    pub fn adt(&self, id: AdtId, args: Vec<Ty>) -> Ty {
        self.mk(TyKind::Adt(id, args))
    }

    pub fn tuple(&self, elems: Vec<Ty>) -> Ty {
        if elems.is_empty() {
            self.unit
        } else {
            self.mk(TyKind::Tuple(elems))
        }
    }

    pub fn ptr(&self, t: Ty) -> Ty {
        self.mk(TyKind::Ptr(t))
    }

    pub fn param(&self, i: u32) -> Ty {
        self.mk(TyKind::Param(i))
    }

    pub fn is_error(&self, t: Ty) -> bool {
        t == self.error
    }

    pub fn is_never(&self, t: Ty) -> bool {
        t == self.never
    }

    pub fn as_int(&self, t: Ty) -> Option<IntTy> {
        match *self.kind(t) {
            TyKind::Int(i) => Some(i),
            _ => None,
        }
    }

    pub fn as_float(&self, t: Ty) -> Option<FloatTy> {
        match *self.kind(t) {
            TyKind::Float(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_adt(&self, t: Ty) -> Option<(AdtId, Vec<Ty>)> {
        match &*self.kind(t) {
            TyKind::Adt(id, args) => Some((*id, args.clone())),
            _ => None,
        }
    }

    /// Does the type mention an inference variable?
    pub fn has_infer(&self, t: Ty) -> bool {
        match &*self.kind(t) {
            TyKind::Infer(_) | TyKind::IntVar(_) | TyKind::FloatVar(_) => true,
            TyKind::Adt(_, a) | TyKind::Tuple(a) => a.iter().any(|&t| self.has_infer(t)),
            TyKind::Array(e, _) | TyKind::Ptr(e) => self.has_infer(*e),
            TyKind::Fn(p, r) | TyKind::Closure(p, r) => p.iter().any(|&(_, t)| self.has_infer(t)) || self.has_infer(*r),
            _ => false,
        }
    }

    /// Does the type mention a closure type?
    pub fn has_closure(&self, t: Ty) -> bool {
        match &*self.kind(t) {
            TyKind::Closure(..) => true,
            TyKind::Adt(_, a) | TyKind::Tuple(a) => a.iter().any(|&t| self.has_closure(t)),
            TyKind::Array(e, _) | TyKind::Ptr(e) => self.has_closure(*e),
            _ => false,
        }
    }

    /// Does the type mention a generic parameter?
    pub fn has_param(&self, t: Ty) -> bool {
        match &*self.kind(t) {
            TyKind::Param(_) => true,
            TyKind::Adt(_, a) | TyKind::Tuple(a) => a.iter().any(|&t| self.has_param(t)),
            TyKind::Array(e, _) | TyKind::Ptr(e) => self.has_param(*e),
            TyKind::Fn(p, r) | TyKind::Closure(p, r) => p.iter().any(|&(_, t)| self.has_param(t)) || self.has_param(*r),
            _ => false,
        }
    }

    pub fn has_error(&self, t: Ty) -> bool {
        match &*self.kind(t) {
            TyKind::Error => true,
            TyKind::Adt(_, a) | TyKind::Tuple(a) => a.iter().any(|&t| self.has_error(t)),
            TyKind::Array(e, _) | TyKind::Ptr(e) => self.has_error(*e),
            TyKind::Fn(p, r) | TyKind::Closure(p, r) => p.iter().any(|&(_, t)| self.has_error(t)) || self.has_error(*r),
            _ => false,
        }
    }

    /// Replaces `Param(i)` by `args[i]`.
    pub fn subst(&self, t: Ty, args: &[Ty]) -> Ty {
        if args.is_empty() {
            return t;
        }
        self.map(t, &mut |k| match k {
            TyKind::Param(i) => args.get(*i as usize).copied(),
            _ => None,
        })
    }

    /// Generic bottom-up rewriting: `f` may replace a leaf type.
    pub fn map(&self, t: Ty, f: &mut dyn FnMut(&TyKind) -> Option<Ty>) -> Ty {
        let k = self.kind(t);
        if let Some(r) = f(&k) {
            return r;
        }
        match &*k {
            TyKind::Adt(id, a) => {
                let a2: Vec<Ty> = a.iter().map(|&x| self.map(x, f)).collect();
                if &a2 == a {
                    t
                } else {
                    self.adt(*id, a2)
                }
            }
            TyKind::Tuple(a) => {
                let a2: Vec<Ty> = a.iter().map(|&x| self.map(x, f)).collect();
                if &a2 == a {
                    t
                } else {
                    self.mk(TyKind::Tuple(a2))
                }
            }
            TyKind::Array(e, n) => {
                let e2 = self.map(*e, f);
                if e2 == *e {
                    t
                } else {
                    self.mk(TyKind::Array(e2, *n))
                }
            }
            TyKind::Ptr(e) => {
                let e2 = self.map(*e, f);
                if e2 == *e {
                    t
                } else {
                    self.ptr(e2)
                }
            }
            TyKind::Fn(p, r) => {
                let p2: Vec<(Convention, Ty)> = p.iter().map(|&(c, x)| (c, self.map(x, f))).collect();
                let r2 = self.map(*r, f);
                self.mk(TyKind::Fn(p2, r2))
            }
            TyKind::Closure(p, r) => {
                let p2: Vec<(Convention, Ty)> = p.iter().map(|&(c, x)| (c, self.map(x, f))).collect();
                let r2 = self.map(*r, f);
                self.mk(TyKind::Closure(p2, r2))
            }
            _ => t,
        }
    }
}

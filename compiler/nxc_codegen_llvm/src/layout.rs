//! Memory layout of NEXUL types (spec §3.8): size, alignment and field offsets.
//!
//! The rules are those of the C ABI of the supported targets (natural alignment, fields
//! in declaration order, padding to the alignment of each field, size rounded to the
//! alignment of the whole), so that they match LLVM's layout of the generated types.
//! Enumerations are a 32-bit tag followed by a payload area large and aligned enough for
//! every variant.

use nxc_sema::defs::{AdtKind, Program};
use nxc_sema::ty::{FloatTy, IntTy, Ty, TyKind};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub size: u64,
    pub align: u64,
}

impl Layout {
    pub const ZERO: Layout = Layout { size: 0, align: 1 };

    fn scalar(bytes: u64) -> Layout {
        Layout { size: bytes, align: bytes }
    }
}

pub fn round_up(x: u64, align: u64) -> u64 {
    x.div_ceil(align) * align
}

/// The payload area of an enumeration.
#[derive(Copy, Clone, Debug)]
pub struct EnumShape {
    /// Size of the largest payload, rounded to `align`.
    pub payload_size: u64,
    /// Largest payload alignment (at least 1).
    pub payload_align: u64,
    /// Offset of the payload area (after the tag).
    pub payload_offset: u64,
    pub has_payload: bool,
}

pub struct LayoutCx<'a> {
    pub p: &'a Program,
    pub ptr_bytes: u64,
    cache: RefCell<HashMap<Ty, Layout>>,
}

impl<'a> LayoutCx<'a> {
    pub fn new(p: &'a Program, ptr_bytes: u64) -> LayoutCx<'a> {
        LayoutCx { p, ptr_bytes, cache: RefCell::new(HashMap::new()) }
    }

    pub fn int_bytes(&self, i: IntTy) -> u64 {
        match i {
            IntTy::I8 | IntTy::U8 => 1,
            IntTy::I16 | IntTy::U16 => 2,
            IntTy::I32 | IntTy::U32 => 4,
            IntTy::I64 | IntTy::U64 => 8,
            IntTy::Isize | IntTy::Usize => self.ptr_bytes,
        }
    }

    pub fn layout(&self, t: Ty) -> Layout {
        if let Some(l) = self.cache.borrow().get(&t) {
            return *l;
        }
        let l = self.compute(t);
        self.cache.borrow_mut().insert(t, l);
        l
    }

    fn compute(&self, t: Ty) -> Layout {
        let tcx = &self.p.tcx;
        match &*tcx.kind(t) {
            TyKind::Int(i) => Layout::scalar(self.int_bytes(*i)),
            TyKind::Float(FloatTy::F32) => Layout::scalar(4),
            TyKind::Float(FloatTy::F64) => Layout::scalar(8),
            TyKind::Bool => Layout::scalar(1),
            TyKind::Char => Layout::scalar(4),
            TyKind::Unit | TyKind::Never | TyKind::Error => Layout::ZERO,
            TyKind::Ptr(_) | TyKind::Fn(..) => Layout::scalar(self.ptr_bytes),
            TyKind::Tuple(elems) => self.struct_layout(elems),
            TyKind::Array(e, n) => {
                let el = self.layout(*e);
                Layout { size: el.size * n, align: el.align }
            }
            TyKind::Adt(id, args) => {
                let adt = self.p.adt(*id);
                match &adt.kind {
                    AdtKind::Struct { fields } => {
                        let tys: Vec<Ty> = fields.iter().map(|f| if f.indirect { tcx.ptr(tcx.unit) } else { tcx.subst(f.ty, args) }).collect();
                        self.struct_layout(&tys)
                    }
                    AdtKind::Enum { .. } => {
                        let s = self.enum_shape(t);
                        let align = s.payload_align.max(4);
                        let size = if s.has_payload { round_up(s.payload_offset + s.payload_size, align) } else { 4 };
                        Layout { size, align }
                    }
                }
            }
            TyKind::Param(_) | TyKind::Infer(_) | TyKind::IntVar(_) | TyKind::FloatVar(_) => {
                panic!("internal: layout of a non-concrete type {}", self.p.ty_str(t))
            }
        }
    }

    pub fn struct_layout(&self, fields: &[Ty]) -> Layout {
        let mut size = 0u64;
        let mut align = 1u64;
        for &f in fields {
            let l = self.layout(f);
            size = round_up(size, l.align) + l.size;
            align = align.max(l.align);
        }
        Layout { size: round_up(size, align), align }
    }

    pub fn struct_offsets(&self, fields: &[Ty]) -> Vec<u64> {
        let mut size = 0u64;
        let mut out = Vec::new();
        for &f in fields {
            let l = self.layout(f);
            size = round_up(size, l.align);
            out.push(size);
            size += l.size;
        }
        out
    }

    /// Field types of a variant's payload as stored (the payload struct).
    pub fn variant_fields(&self, t: Ty, v: u32) -> Vec<Ty> {
        let tcx = &self.p.tcx;
        let (id, args) = tcx.as_adt(t).expect("enum type");
        self.p.adt(id).variants()[v as usize].fields.iter().map(|&f| tcx.subst(f, &args)).collect()
    }

    pub fn enum_shape(&self, t: Ty) -> EnumShape {
        let tcx = &self.p.tcx;
        let (id, _) = tcx.as_adt(t).expect("enum type");
        let variants = self.p.adt(id).variants();
        let mut size = 0u64;
        let mut align = 1u64;
        let mut has_payload = false;
        for (vi, v) in variants.iter().enumerate() {
            if v.fields.is_empty() {
                continue;
            }
            has_payload = true;
            let l = if v.indirect { Layout::scalar(self.ptr_bytes) } else { self.struct_layout(&self.variant_fields(t, vi as u32)) };
            size = size.max(l.size);
            align = align.max(l.align);
        }
        let payload_size = round_up(size, align);
        EnumShape { payload_size, payload_align: align, payload_offset: round_up(4, align), has_payload }
    }
}

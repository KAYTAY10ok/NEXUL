//! The C calling convention of each target for `@repr("C")` structures passed by value
//! (spec §8.2): how a C compiler passes and returns them. The rules are those clang
//! applies to the same structures; `compiler/nxc/tests/c_abi.rs` compares the signatures
//! produced here with clang's, on every target, and runs the programs where it can.
//!
//! A structure is described by its *leaves*: its scalar fields, nested structures and
//! arrays flattened, each with its offset.

use nxc_sema::target::{Arch, Os, Target};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Leaf {
    /// An integer of this many bytes.
    Int(u64),
    Ptr(u64),
    F32,
    F64,
}

impl Leaf {
    fn size(self) -> u64 {
        match self {
            Leaf::Int(n) | Leaf::Ptr(n) => n,
            Leaf::F32 => 4,
            Leaf::F64 => 8,
        }
    }

    fn is_float(self) -> bool {
        matches!(self, Leaf::F32 | Leaf::F64)
    }

    fn llty(self) -> String {
        match self {
            Leaf::Int(n) => format!("i{}", n * 8),
            Leaf::Ptr(_) => "ptr".into(),
            Leaf::F32 => "float".into(),
            Leaf::F64 => "double".into(),
        }
    }
}

/// A `@repr("C")` structure as the calling conventions see it.
#[derive(Clone, Debug)]
pub struct Shape {
    pub size: u64,
    pub align: u64,
    /// (offset, leaf), in increasing offsets.
    pub leaves: Vec<(u64, Leaf)>,
    /// The LLVM type of the structure (`%"P#12"`), for `byval`, `sret` and the
    /// structures passed or returned as they are.
    pub llty: String,
}

impl Shape {
    /// A homogeneous floating-point aggregate: one to four leaves of the same float type.
    fn hfa(&self) -> Option<Leaf> {
        let first = self.leaves.first()?.1;
        let ok = first.is_float() && self.leaves.len() <= 4 && self.leaves.iter().all(|&(_, l)| l == first);
        ok.then_some(first)
    }
}

/// How a value is passed to a C function, or received from C.
#[derive(Clone, Debug, PartialEq)]
pub enum ArgAbi {
    /// A scalar, as NEXUL passes it (with its extension attribute).
    Direct,
    /// Separate LLVM arguments, each loaded from this offset of the structure.
    Pieces(Vec<(u64, String)>),
    /// One LLVM argument of this type, loaded from the start of the structure, with these
    /// extra attributes (`alignstack(8)`).
    Agg(String, &'static str),
    /// A pointer to a copy: `byval` (the copy is on the stack of the call) or a plain
    /// pointer to a copy made by the caller.
    Indirect { byval: bool, align: u64 },
}

/// How a value is returned by a C function.
#[derive(Clone, Debug, PartialEq)]
pub enum RetAbi {
    Direct,
    /// In registers, as this LLVM type, stored at the start of the structure.
    Agg(String),
    /// In memory the caller provides, through a hidden first parameter.
    Sret { align: u64 },
}

/// What a parameter or result is, for the classification of a whole signature.
#[derive(Clone, Debug)]
pub enum Value {
    /// A scalar: an integer or pointer (`false`) or a float (`true`).
    Scalar { float: bool },
    Struct(Shape),
}

/// The convention of a signature: one `ArgAbi` per parameter, and the result.
pub fn classify(t: &Target, params: &[Value], ret: Option<&Value>) -> (Vec<ArgAbi>, RetAbi) {
    let ret_abi = match ret {
        Some(Value::Struct(s)) => classify_ret(t, s),
        _ => RetAbi::Direct,
    };
    let sret = matches!(ret_abi, RetAbi::Sret { .. });
    // Registers left, for the conventions that pass a structure in registers only when
    // all its pieces fit (x86-64 System V, the floating-point convention of RISC-V).
    let (mut ints, mut floats) = match t.arch {
        Arch::X86_64 => (6, 8),
        Arch::Riscv64 | Arch::Riscv32 => (8, if t.hard_float64 { 8 } else { 0 }),
        _ => (0, 0),
    };
    if sret {
        ints -= 1;
    }
    let mut out = Vec::new();
    for p in params {
        match p {
            Value::Scalar { float } => {
                if *float && floats > 0 {
                    floats -= 1;
                } else if ints > 0 {
                    ints -= 1;
                }
                out.push(ArgAbi::Direct);
            }
            Value::Struct(s) => out.push(classify_arg(t, s, &mut ints, &mut floats)),
        }
    }
    (out, ret_abi)
}

fn windows_x64(t: &Target) -> bool {
    t.os == Os::Windows && t.arch == Arch::X86_64
}

/// ARM with the hard-float ABI: C passes floats in VFP registers.
pub fn arm_hard_float(t: &Target) -> bool {
    t.arch == Arch::Arm && t.llvm_triple.ends_with("eabihf")
}

fn classify_arg(t: &Target, s: &Shape, ints: &mut u32, floats: &mut u32) -> ArgAbi {
    match t.arch {
        Arch::X86_64 if windows_x64(t) => match s.size {
            1 | 2 | 4 | 8 => ArgAbi::Agg(format!("i{}", s.size * 8), ""),
            _ => ArgAbi::Indirect { byval: false, align: s.align },
        },
        Arch::X86_64 => match sysv_pieces(s) {
            Some(pieces) => {
                let ni = pieces.iter().filter(|(_, sse, _)| !sse).count() as u32;
                let nf = pieces.len() as u32 - ni;
                if ni <= *ints && nf <= *floats {
                    *ints -= ni;
                    *floats -= nf;
                    ArgAbi::Pieces(pieces.into_iter().map(|(o, _, ty)| (o, ty)).collect())
                } else {
                    sysv_memory(s, *ints)
                }
            }
            None => sysv_memory(s, *ints),
        },
        Arch::Aarch64 => {
            if let Some(f) = s.hfa() {
                // Linux passes the array aligned to 8 on the stack; Apple and Windows do not.
                let attr = if t.os == Os::MacOs || t.os == Os::Windows { "" } else { " alignstack(8)" };
                ArgAbi::Agg(format!("[{} x {}]", s.leaves.len(), f.llty()), attr)
            } else if s.size <= 8 {
                ArgAbi::Agg("i64".into(), "")
            } else if s.size <= 16 {
                ArgAbi::Agg("[2 x i64]".into(), "")
            } else {
                ArgAbi::Indirect { byval: false, align: s.align }
            }
        }
        Arch::Riscv64 | Arch::Riscv32 => {
            let xlen = if t.arch == Arch::Riscv64 { 8 } else { 4 };
            if s.size > 2 * xlen {
                return ArgAbi::Indirect { byval: false, align: s.align };
            }
            if let Some((nf, ni)) = riscv_fp_needs(t, s, xlen) {
                if nf <= *floats && ni <= *ints {
                    *floats -= nf;
                    *ints -= ni;
                    return ArgAbi::Pieces(s.leaves.iter().map(|&(o, l)| (o, l.llty())).collect());
                }
            }
            *ints = ints.saturating_sub(s.size.div_ceil(xlen) as u32);
            ArgAbi::Agg(riscv_int(s, xlen), "")
        }
        Arch::Wasm32 => match single_leaf(s) {
            Some(l) => ArgAbi::Agg(l.llty(), ""),
            None => ArgAbi::Indirect { byval: true, align: s.align },
        },
        Arch::Arm => {
            if arm_hard_float(t) && s.hfa().is_some() {
                ArgAbi::Agg(s.llty.clone(), "")
            } else if s.size > 64 {
                ArgAbi::Indirect { byval: true, align: s.align }
            } else if s.align >= 8 {
                ArgAbi::Agg(format!("[{} x i64]", s.size.div_ceil(8)), "")
            } else {
                ArgAbi::Agg(format!("[{} x i32]", s.size.div_ceil(4)), "")
            }
        }
    }
}

fn classify_ret(t: &Target, s: &Shape) -> RetAbi {
    let sret = RetAbi::Sret { align: s.align };
    match t.arch {
        Arch::X86_64 if windows_x64(t) => match s.size {
            1 | 2 | 4 | 8 => RetAbi::Agg(format!("i{}", s.size * 8)),
            _ => sret,
        },
        Arch::X86_64 => match sysv_pieces(s) {
            Some(p) if p.len() == 1 => RetAbi::Agg(p[0].2.clone()),
            Some(p) => RetAbi::Agg(format!("{{ {} }}", p.iter().map(|x| x.2.clone()).collect::<Vec<_>>().join(", "))),
            None => sret,
        },
        Arch::Aarch64 => {
            if s.hfa().is_some() {
                RetAbi::Agg(s.llty.clone())
            } else if s.size <= 8 {
                RetAbi::Agg(format!("i{}", s.size * 8))
            } else if s.size <= 16 {
                RetAbi::Agg("[2 x i64]".into())
            } else {
                sret
            }
        }
        Arch::Riscv64 | Arch::Riscv32 => {
            let xlen = if t.arch == Arch::Riscv64 { 8 } else { 4 };
            if s.size > 2 * xlen {
                return sret;
            }
            if riscv_fp_needs(t, s, xlen).is_some() {
                let tys: Vec<String> = s.leaves.iter().map(|&(_, l)| l.llty()).collect();
                return RetAbi::Agg(if tys.len() == 1 { tys[0].clone() } else { format!("{{ {} }}", tys.join(", ")) });
            }
            RetAbi::Agg(riscv_int(s, xlen))
        }
        Arch::Wasm32 => match single_leaf(s) {
            Some(l) => RetAbi::Agg(l.llty()),
            None => sret,
        },
        Arch::Arm => {
            if arm_hard_float(t) && s.hfa().is_some() {
                RetAbi::Agg(s.llty.clone())
            } else if s.size <= 4 {
                RetAbi::Agg(match s.size {
                    1 => "i8".into(),
                    2 => "i16".into(),
                    _ => "i32".into(),
                })
            } else {
                sret
            }
        }
    }
}

/// x86-64 System V, a structure passed in memory. With no integer register left, clang
/// passes one of at most 8 bytes as an integer of its size, which lands on the stack where
/// the structure would (`byval` otherwise).
fn sysv_memory(s: &Shape, ints: u32) -> ArgAbi {
    if ints == 0 && s.size <= 8 && s.align <= 8 {
        ArgAbi::Agg(format!("i{}", s.size * 8), "")
    } else {
        ArgAbi::Indirect { byval: true, align: s.align.max(8) }
    }
}

/// x86-64 System V: the one or two eightbytes of a structure of at most 16 bytes, each
/// (offset, in an SSE register, LLVM type); `None` when it goes in memory.
fn sysv_pieces(s: &Shape) -> Option<Vec<(u64, bool, String)>> {
    if s.size > 16 {
        return None;
    }
    let mut out = Vec::new();
    let mut off = 0;
    while off < s.size {
        let end = (off + 8).min(s.size);
        let inside: Vec<(u64, Leaf)> = s.leaves.iter().filter(|(o, _)| *o >= off && *o < end).copied().collect();
        if !inside.is_empty() {
            let sse = inside.iter().all(|(_, l)| l.is_float());
            let data_end = inside.iter().map(|&(o, l)| o + l.size()).max().unwrap_or(off);
            let ty = if !sse {
                match inside.as_slice() {
                    // A single scalar at the start of its eightbyte, the rest padding: its
                    // own type (clang: `{ i8, double }`, `{ ptr, i32 }`).
                    [(o, l)] if *o == off && (matches!(l, Leaf::Ptr(_)) || matches!(l.size(), 1 | 2 | 4 | 8)) && (end - off == 8 || l.size() == end - off) => l.llty(),
                    _ => format!("i{}", (end - off) * 8),
                }
            } else if inside.iter().any(|&(_, l)| l == Leaf::F64) {
                "double".to_string()
            } else if data_end - off <= 4 {
                "float".to_string()
            } else {
                "<2 x float>".to_string()
            };
            out.push((off, sse, ty));
        }
        off += 8;
    }
    Some(out)
}

/// RISC-V: the floating-point registers and integer registers a structure takes under the
/// floating-point convention (one float, two floats, or one float and one integer, each
/// fitting its register), if it qualifies.
fn riscv_fp_needs(t: &Target, s: &Shape, xlen: u64) -> Option<(u32, u32)> {
    if !t.hard_float64 {
        return None;
    }
    let l = &s.leaves;
    let fits = |x: Leaf| if x.is_float() { x.size() <= 8 } else { x.size() <= xlen };
    match l.as_slice() {
        [(_, a)] if a.is_float() => Some((1, 0)),
        [(_, a), (_, b)] if fits(*a) && fits(*b) && (a.is_float() || b.is_float()) => {
            let nf = a.is_float() as u32 + b.is_float() as u32;
            Some((nf, 2 - nf))
        }
        _ => None,
    }
}

/// RISC-V integer convention: one register, or two.
fn riscv_int(s: &Shape, xlen: u64) -> String {
    if s.size <= xlen {
        format!("i{}", xlen * 8)
    } else if s.align == 2 * xlen {
        format!("i{}", xlen * 16)
    } else {
        format!("[2 x i{}]", xlen * 8)
    }
}

/// The only scalar of a structure that has one (WebAssembly passes it as that scalar).
fn single_leaf(s: &Shape) -> Option<Leaf> {
    match s.leaves.as_slice() {
        [(_, l)] if l.size() == s.size => Some(*l),
        _ => None,
    }
}

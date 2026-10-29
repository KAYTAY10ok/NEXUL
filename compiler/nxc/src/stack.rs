//! The bound of the stack of a program with real-time tasks (ADR 0018 §6), computed on the
//! linked executable, in machine code: after optimization and inlining, as it runs.
//!
//! - The frame of a function is what its Thumb-2 code takes from `sp`: `push`, `vpush`,
//!   `str rX, [sp, #-n]!`, `sub sp, #n`.
//! - Its callees are the targets of its `bl`, and of its `b.w` that leave it (tail calls).
//! - Any other write to `sp`, a call through a register (`blx rX`, `bx rX` other than a
//!   return) or a cycle makes the bound unknown: an error (E0607), never an estimate.
//!
//! The bound is the frames of the startup chain up to `rt.run()`, plus, for each level of
//! priority, the deepest of its handlers and the frame the core pushes on an exception,
//! plus SysTick: a handler interrupts only handlers of lower levels (ADR 0018 §3).

use nxc_codegen_llvm::StackSpec;
use std::collections::HashMap;

pub struct Report {
    pub bound: u64,
    pub lines: Vec<String>,
}

struct Elf<'a> {
    data: &'a [u8],
    /// Executable sections: (address, file offset, size).
    code: Vec<(u32, u32, u32)>,
    /// Functions: name → (start, size).
    funcs: HashMap<String, (u32, u32)>,
    starts: HashMap<u32, String>,
    /// `$t` (code) and `$d` (data) markers, by address.
    marks: Vec<(u32, bool)>,
}

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn parse(data: &[u8]) -> Option<Elf<'_>> {
    if data.len() < 52 || &data[0..4] != b"\x7fELF" || data[4] != 1 {
        return None;
    }
    let shoff = u32le(data, 0x20) as usize;
    let shentsize = u16le(data, 0x2E) as usize;
    let shnum = u16le(data, 0x30) as usize;
    let sec = |i: usize| -> Option<[u32; 10]> {
        let o = shoff + i * shentsize;
        if o + 40 > data.len() {
            return None;
        }
        let mut v = [0u32; 10];
        for (k, x) in v.iter_mut().enumerate() {
            *x = u32le(data, o + 4 * k);
        }
        Some(v)
    };
    let mut elf = Elf { data, code: Vec::new(), funcs: HashMap::new(), starts: HashMap::new(), marks: Vec::new() };
    for i in 0..shnum {
        let s = sec(i)?;
        // SHF_EXECINSTR
        if s[2] & 4 != 0 && s[1] == 1 {
            elf.code.push((s[3], s[4], s[5]));
        }
        if s[1] == 2 {
            let strtab = sec(s[6] as usize)?;
            let (off, size) = (s[4] as usize, s[5] as usize);
            for k in 0..size / 16 {
                let o = off + 16 * k;
                let (name_off, value, sz, info) = (u32le(data, o), u32le(data, o + 4), u32le(data, o + 8), data[o + 12]);
                let n0 = strtab[4] as usize + name_off as usize;
                let end = data[n0..].iter().position(|&c| c == 0).map(|p| n0 + p)?;
                let name = String::from_utf8_lossy(&data[n0..end]).to_string();
                if name.starts_with("$t") || name.starts_with("$d") {
                    elf.marks.push((value & !1, name.starts_with("$d")));
                } else if info & 0xF == 2 {
                    elf.starts.insert(value & !1, name.clone());
                    elf.funcs.insert(name, (value & !1, sz));
                }
            }
        }
    }
    elf.marks.sort();
    Some(elf)
}

impl<'a> Elf<'a> {
    fn halfword(&self, addr: u32) -> Option<u16> {
        self.code.iter().find(|(a, _, s)| addr >= *a && addr + 2 <= a + s).map(|(a, o, _)| u16le(self.data, (o + addr - a) as usize))
    }

    fn is_data(&self, addr: u32) -> bool {
        match self.marks.partition_point(|(a, _)| *a <= addr) {
            0 => false,
            i => self.marks[i - 1].1,
        }
    }
}

/// What a function does to the stack and whom it calls.
struct Code {
    frame: u64,
    calls: Vec<u32>,
    unknown: Option<String>,
}

fn decode(elf: &Elf, start: u32, size: u32) -> Code {
    let mut c = Code { frame: 0, calls: Vec::new(), unknown: None };
    let end = start + size;
    let mut a = start;
    while a < end {
        if elf.is_data(a) {
            a += 2;
            continue;
        }
        let Some(hw) = elf.halfword(a) else { break };
        let wide = matches!(hw >> 11, 0b11101..=0b11111);
        if !wide {
            if hw & 0xFE00 == 0xB400 {
                c.frame += 4 * ((hw & 0xFF).count_ones() as u64 + ((hw >> 8) & 1) as u64);
            } else if hw & 0xFF80 == 0xB080 {
                c.frame += 4 * (hw & 0x7F) as u64;
            } else if hw & 0xFF87 == 0x4685 {
                // `mov sp, rM`: restoring the frame pointer (r7) is an epilogue.
                if (hw >> 3) & 0xF != 7 {
                    c.unknown.get_or_insert(format!("`mov sp, r{}` at {:#x}", (hw >> 3) & 0xF, a));
                }
            } else if hw & 0xFF87 == 0x4485 {
                c.unknown.get_or_insert(format!("`add sp, r{}` at {:#x}", (hw >> 3) & 0xF, a));
            } else if hw & 0xFF87 == 0x4780 {
                c.unknown.get_or_insert(format!("a call through a register (`blx r{}`) at {:#x}", (hw >> 3) & 0xF, a));
            } else if hw & 0xFF87 == 0x4700 && (hw >> 3) & 0xF != 14 {
                c.unknown.get_or_insert(format!("a jump through a register (`bx r{}`) at {:#x}", (hw >> 3) & 0xF, a));
            }
            a += 2;
            continue;
        }
        let Some(hw2) = elf.halfword(a + 2) else { break };
        if hw == 0xE92D {
            c.frame += 4 * (hw2 & 0x5FFF).count_ones() as u64;
        } else if hw == 0xF84D && hw2 & 0x0F00 == 0x0D00 {
            c.frame += (hw2 & 0xFF) as u64;
        } else if hw & 0xFFBF == 0xED2D && hw2 & 0x0E00 == 0x0A00 {
            c.frame += 4 * (hw2 & 0xFF) as u64;
        } else if hw & 0xFBEF == 0xF1AD && hw2 & 0x8F00 == 0x0D00 {
            c.frame += thumb_expand(((hw as u32 >> 10) & 1) << 11 | ((hw2 as u32 >> 12) & 7) << 8 | (hw2 as u32 & 0xFF)) as u64;
        } else if hw & 0xFBFF == 0xF2AD && hw2 & 0x8F00 == 0x0D00 {
            c.frame += (((hw as u32 >> 10) & 1) << 11 | ((hw2 as u32 >> 12) & 7) << 8 | (hw2 as u32 & 0xFF)) as u64;
        } else if (hw & 0xFFEF == 0xEBAD || hw & 0xFFEF == 0xEA4F || hw & 0xFFEF == 0xEB0D) && hw2 & 0x0F00 == 0x0D00 {
            c.unknown.get_or_insert(format!("a write to `sp` from a register at {:#x}", a));
        } else if hw & 0xF800 == 0xF000 && hw2 & 0xD000 == 0xC000 {
            c.unknown.get_or_insert(format!("a call to ARM code (`blx`) at {:#x}", a));
        } else if hw & 0xF800 == 0xF000 && (hw2 & 0xD000 == 0xD000 || hw2 & 0xD000 == 0x9000) {
            let s = ((hw >> 10) & 1) as u32;
            let i1 = !(((hw2 >> 13) & 1) as u32 ^ s) & 1;
            let i2 = !(((hw2 >> 11) & 1) as u32 ^ s) & 1;
            let imm = (s << 24) | (i1 << 23) | (i2 << 22) | ((hw as u32 & 0x3FF) << 12) | ((hw2 as u32 & 0x7FF) << 1);
            let off = ((imm << 7) as i32) >> 7;
            let target = (a as i64 + 4 + off as i64) as u32;
            let is_bl = hw2 & 0xD000 == 0xD000;
            if is_bl || target < start || target >= end {
                c.calls.push(target);
            }
        } else if hw & 0xFFF0 == 0xF8D0 && hw2 >> 12 == 0xF {
            c.unknown.get_or_insert(format!("a jump through memory (`ldr pc`) at {:#x}", a));
        }
        a += 4;
    }
    c
}

/// `ThumbExpandImm` of a 12-bit modified immediate.
fn thumb_expand(imm12: u32) -> u32 {
    if imm12 >> 10 == 0 {
        let b = imm12 & 0xFF;
        match (imm12 >> 8) & 3 {
            0 => b,
            1 => b << 16 | b,
            2 => b << 24 | b << 8,
            _ => b << 24 | b << 16 | b << 8 | b,
        }
    } else {
        let v = 0x80 | (imm12 & 0x7F);
        v.rotate_right(imm12 >> 7)
    }
}

struct Analysis<'a> {
    elf: &'a Elf<'a>,
    memo: HashMap<String, u64>,
    stops: &'a [String],
}

impl<'a> Analysis<'a> {
    fn frame(&self, name: &str) -> Result<u64, String> {
        let (s, z) = self.elf.funcs[name];
        Ok(decode(self.elf, s, z).frame)
    }

    /// The deepest stack a function can need, with its callees.
    fn depth(&mut self, name: &str, path: &mut Vec<String>) -> Result<u64, String> {
        if let Some(&d) = self.memo.get(name) {
            return Ok(d);
        }
        // A panic stops the program; if it overflows, the guard stops it (ADR 0018 §6).
        if self.stops.binary_search_by(|s| s.as_str().cmp(name)).is_ok() {
            return Ok(0);
        }
        if path.iter().any(|p| p == name) {
            return Err(format!("the code of `{}` calls itself again (through {})", name, path.join(" → ")));
        }
        let Some(&(s, z)) = self.elf.funcs.get(name) else {
            return Err(format!("`{}` is not in the executable", name));
        };
        let code = decode(self.elf, s, z);
        if let Some(u) = code.unknown {
            return Err(format!("in `{}`: {}", name, u));
        }
        path.push(name.to_string());
        let mut deepest = 0;
        for t in code.calls {
            let Some(callee) = self.elf.starts.get(&t).cloned() else {
                path.pop();
                return Err(format!("`{}` calls {:#x}, which is not the start of a function", name, t));
            };
            deepest = deepest.max(self.depth(&callee, path)?);
        }
        path.pop();
        let d = code.frame + deepest;
        self.memo.insert(name.to_string(), d);
        Ok(d)
    }
}

/// Checks the bound of the stack of `exe` against the reserve (ADR 0018 §6).
pub fn check(spec: &StackSpec, exe: &std::path::Path) -> Result<Report, String> {
    let data = std::fs::read(exe).map_err(|e| format!("cannot read `{}`: {}", exe.display(), e))?;
    let elf = parse(&data).ok_or("error[E0607]: the executable cannot be read to bound its stack")?;
    let mut an = Analysis { elf: &elf, memo: HashMap::new(), stops: &spec.stops };
    let unknown = |e: String| format!("error[E0607]: the bound of the stack of the tasks is unknown: {}\n  = note: the compiler refuses to estimate it (ADR 0018 §6)", e);
    let mut lines = Vec::new();
    // The startup chain: only the frames (each function is paused in a call).
    let mut chain = 0;
    for f in spec.chain.iter().filter(|f| elf.funcs.contains_key(*f)) {
        chain += an.frame(f).map_err(unknown)?;
    }
    lines.push(format!("  startup up to `rt.run()`: {} bytes", chain));
    let mut bound = chain;
    for (level, roots) in &spec.levels {
        let mut deepest = 0;
        for r in roots {
            deepest = deepest.max(an.depth(r, &mut Vec::new()).map_err(unknown)?);
        }
        lines.push(format!("  level {}: {} bytes + {} for the interruption", level, deepest, spec.exception));
        bound += deepest + spec.exception;
    }
    if let Some(st) = &spec.systick {
        let d = an.depth(st, &mut Vec::new()).map_err(unknown)?;
        lines.push(format!("  SysTick: {} bytes + {} for the interruption", d, spec.exception));
        bound += d + spec.exception;
    }
    lines.push(format!("  bound: {} bytes, reserve: {} bytes", bound, spec.reserve));
    if bound > spec.reserve {
        return Err(format!(
            "error[E0607]: the stack of the tasks can need {} bytes, more than the reserve of {} bytes\n{}\n  = help: raise the reserve with `@stack(n)` on `main`, or make the tasks use less stack",
            bound,
            spec.reserve,
            lines.join("\n")
        ));
    }
    Ok(Report { bound, lines })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modified_immediates() {
        assert_eq!(thumb_expand(0x0FF), 0xFF);
        // 0xFF rotated right by 9.
        assert_eq!(thumb_expand(0x4FF), 0x7F80_0000);
        // `sub.w sp, sp, #1024`: 0x80 rotated right by 29.
        assert_eq!(thumb_expand(0xE80), 1024);
    }
}

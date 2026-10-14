//! Global symbols defined by an object file (COFF, ELF), for the symbol table of static
//! libraries.

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn u64le(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

fn cstr(b: &[u8], o: usize) -> String {
    let end = b[o..].iter().position(|&c| c == 0).map(|p| o + p).unwrap_or(b.len());
    String::from_utf8_lossy(&b[o..end]).to_string()
}

pub fn defined_symbols(obj: &[u8]) -> Vec<String> {
    if obj.len() > 64 && &obj[0..4] == b"\x7fELF" {
        return elf_symbols(obj).unwrap_or_default();
    }
    coff_symbols(obj).unwrap_or_default()
}

fn coff_symbols(b: &[u8]) -> Option<Vec<String>> {
    if b.len() < 20 {
        return None;
    }
    let symtab = u32le(b, 8) as usize;
    let nsyms = u32le(b, 12) as usize;
    let strtab = symtab + nsyms * 18;
    let mut out = Vec::new();
    let mut i = 0;
    while i < nsyms {
        let o = symtab + i * 18;
        if o + 18 > b.len() {
            break;
        }
        let section = u16le(b, o + 12) as i16;
        let class = b[o + 16];
        let aux = b[o + 17] as usize;
        if class == 2 && section > 0 {
            let name = if u32le(b, o) == 0 { cstr(b, strtab + u32le(b, o + 4) as usize) } else { cstr(&b[o..o + 8], 0) };
            out.push(name);
        }
        i += 1 + aux;
    }
    Some(out)
}

fn elf_symbols(b: &[u8]) -> Option<Vec<String>> {
    let is64 = b[4] == 2;
    let (shoff, shentsize, shnum) = if is64 { (u64le(b, 0x28) as usize, u16le(b, 0x3A) as usize, u16le(b, 0x3C) as usize) } else { (u32le(b, 0x20) as usize, u16le(b, 0x2E) as usize, u16le(b, 0x30) as usize) };
    let mut out = Vec::new();
    for s in 0..shnum {
        let h = shoff + s * shentsize;
        let sh_type = u32le(b, h + 4);
        if sh_type != 2 {
            continue; // SHT_SYMTAB
        }
        let (off, size, link, entsize) = if is64 {
            (u64le(b, h + 0x18) as usize, u64le(b, h + 0x20) as usize, u32le(b, h + 0x28) as usize, u64le(b, h + 0x38) as usize)
        } else {
            (u32le(b, h + 0x10) as usize, u32le(b, h + 0x14) as usize, u32le(b, h + 0x18) as usize, u32le(b, h + 0x24) as usize)
        };
        let sh = shoff + link * shentsize;
        let stroff = if is64 { u64le(b, sh + 0x18) as usize } else { u32le(b, sh + 0x10) as usize };
        let n = size / entsize.max(1);
        for i in 1..n {
            let e = off + i * entsize;
            let (name, info, shndx) = if is64 { (u32le(b, e) as usize, b[e + 4], u16le(b, e + 6)) } else { (u32le(b, e) as usize, b[e + 12], u16le(b, e + 14)) };
            let bind = info >> 4;
            if (bind == 1 || bind == 2) && shndx != 0 {
                out.push(cstr(b, stroff + name));
            }
        }
    }
    Some(out)
}

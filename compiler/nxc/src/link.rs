//! Linking (spec §10.6) with LLD, the LLVM linker. NEXUL programs do not use a C
//! library: on Windows the system DLLs are reached through import libraries written by
//! `nxc` itself; on Linux executables are static and call the kernel directly; WebAssembly
//! modules import the WASI functions.

use crate::toolchain::Toolchain;
use nxc_sema::target::{Arch, Os, Target};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputKind {
    Executable,
    /// A shared library (`.dll`, `.so`, `.dylib`).
    SharedLibrary,
    /// A static library (`.lib`, `.a`).
    StaticLibrary,
}

pub struct LinkJob<'a> {
    pub target: &'a Target,
    pub objects: Vec<PathBuf>,
    pub out: PathBuf,
    pub kind: OutputKind,
    /// (library, symbol) pairs: DLL imports on Windows.
    pub imports: &'a [(String, String)],
    /// Libraries named by `@link` (other targets).
    pub links: &'a [String],
    pub exports: &'a [String],
    pub debug: bool,
    pub optimize: bool,
    /// Directory for intermediate files.
    pub work_dir: PathBuf,
    /// Additional arguments for the linker (`--link-arg`).
    pub extra: &'a [String],
}

fn run(cmd: &mut Command, what: &str) -> Result<(), String> {
    match cmd.output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(format!("{} failed:\n{}{}", what, String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))),
        Err(e) => Err(format!("cannot run {}: {}", what, e)),
    }
}

pub fn link(tc: &Toolchain, job: &LinkJob) -> Result<(), String> {
    if job.kind == OutputKind::StaticLibrary {
        return write_static_library(&job.objects, &job.out, job.target);
    }
    match job.target.os {
        Os::Windows => link_windows(tc, job),
        Os::Wasi | Os::Web => link_wasm(tc, job),
        Os::MacOs => link_macos(tc, job),
        Os::Linux => link_elf(tc, job),
        Os::None => link_bare(tc, job),
    }
}

fn link_windows(tc: &Toolchain, job: &LinkJob) -> Result<(), String> {
    let lld = tc.linker("lld-link")?;
    let machine = if job.target.arch == Arch::Aarch64 { "arm64" } else { "x64" };
    let mut cmd = Command::new(lld);
    cmd.arg("/nologo").arg(format!("/machine:{}", machine)).arg("/nodefaultlib").arg("/incremental:no");
    match job.kind {
        OutputKind::Executable => {
            cmd.arg("/subsystem:console").arg("/entry:mainCRTStartup").arg("/stack:8388608");
        }
        _ => {
            cmd.arg("/dll").arg("/noentry").arg(format!("/implib:{}", job.out.with_extension("lib").display()));
            for e in job.exports {
                cmd.arg(format!("/export:{}", e));
            }
        }
    }
    cmd.arg(format!("/out:{}", job.out.display()));
    if job.debug {
        // The executable records only the file name of its `.pdb`: debuggers find it
        // next to the executable wherever it is copied.
        cmd.arg("/debug:full").arg(format!("/pdb:{}", job.out.with_extension("pdb").display())).arg("/pdbaltpath:%_PDB%");
    } else {
        cmd.arg("/debug:none").arg("/opt:ref").arg("/opt:icf");
    }
    for o in &job.objects {
        cmd.arg(o);
    }
    // One import library per DLL, generated from the declarations of the program.
    let mut by_dll: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (dll, sym) in job.imports {
        let list = by_dll.entry(dll.to_ascii_lowercase()).or_default();
        if !list.contains(sym) {
            list.push(sym.clone());
        }
    }
    for (dll, syms) in &by_dll {
        let dll_name = if dll.ends_with(".dll") { dll.clone() } else { format!("{}.dll", dll) };
        let lib = job.work_dir.join(format!("{}.nx-implib.lib", dll_name.trim_end_matches(".dll")));
        std::fs::write(&lib, import_library(&dll_name, syms, job.target.arch)).map_err(|e| format!("cannot write `{}`: {}", lib.display(), e))?;
        cmd.arg(&lib);
    }
    for l in job.links {
        if !by_dll.contains_key(&l.to_ascii_lowercase()) {
            cmd.arg(format!("{}.lib", l));
        }
    }
    cmd.args(job.extra);
    run(&mut cmd, "the linker (lld-link)")
}

fn link_elf(tc: &Toolchain, job: &LinkJob) -> Result<(), String> {
    let lld = tc.linker("ld.lld")?;
    let mut cmd = Command::new(lld);
    cmd.arg("-o").arg(&job.out).arg("-z").arg("noexecstack").arg("--gc-sections").arg("--build-id=none");
    match job.kind {
        OutputKind::Executable => {
            cmd.arg("-static").arg("-e").arg("_start");
        }
        _ => {
            cmd.arg("-shared");
        }
    }
    if !job.debug {
        cmd.arg("--strip-all").arg("-O2");
    }
    for o in &job.objects {
        cmd.arg(o);
    }
    for l in job.links {
        cmd.arg(format!("-l{}", l));
    }
    cmd.args(job.extra);
    run(&mut cmd, "the linker (ld.lld)")
}

fn link_wasm(tc: &Toolchain, job: &LinkJob) -> Result<(), String> {
    let lld = tc.linker("wasm-ld")?;
    let mut cmd = Command::new(lld);
    // A downward-growing stack must underflow out of linear memory before it
    // can overwrite static data (ADR 0011, T3). Applies to WASI and the browser.
    cmd.arg("-o").arg(&job.out).arg("--gc-sections").arg("--stack-first").arg("-z").arg("stack-size=8388608");
    if job.target.os == Os::Web {
        cmd.arg("--export-memory");
    }
    match job.kind {
        OutputKind::Executable => {}
        _ => {
            cmd.arg("--no-entry");
            for e in job.exports {
                cmd.arg(format!("--export={}", e));
            }
        }
    }
    if !job.debug {
        cmd.arg("--strip-all").arg("-O2");
    }
    for o in &job.objects {
        cmd.arg(o);
    }
    cmd.args(job.extra);
    run(&mut cmd, "the linker (wasm-ld)")
}

fn link_macos(tc: &Toolchain, job: &LinkJob) -> Result<(), String> {
    let lld = tc.linker("ld64.lld")?;
    let arch = if job.target.arch == Arch::Aarch64 { "arm64" } else { "x86_64" };
    // A text stub of the system library lists the functions used: no SDK is needed.
    let stub = job.work_dir.join("libSystem.nx.tbd");
    let mut syms: Vec<String> = job.imports.iter().map(|(_, s)| format!("_{}", s)).collect();
    // The lazy binding of the imports goes through this function of the loader.
    syms.push("dyld_stub_binder".into());
    syms.sort();
    syms.dedup();
    let tbd = format!(
        "--- !tapi-tbd\ntbd-version: 4\ntargets: [ {}-macos ]\ninstall-name: '/usr/lib/libSystem.B.dylib'\nexports:\n  - targets: [ {}-macos ]\n    symbols: [ {} ]\n...\n",
        arch,
        arch,
        syms.join(", ")
    );
    std::fs::write(&stub, tbd).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(lld);
    cmd.arg("-arch").arg(arch).arg("-platform_version").arg("macos").arg("11.0.0").arg("11.0.0").arg("-o").arg(&job.out);
    if job.kind == OutputKind::Executable {
        cmd.arg("-e").arg("_main");
    } else {
        cmd.arg("-dylib");
        // Found through the run paths of the program that uses it, not at the place where
        // it was linked.
        if let Some(name) = job.out.file_name() {
            cmd.arg("-install_name").arg(format!("@rpath/{}", name.to_string_lossy()));
        }
    }
    for o in &job.objects {
        cmd.arg(o);
    }
    cmd.arg(&stub);
    cmd.args(job.extra);
    run(&mut cmd, "the linker (ld64.lld)")
}

/// Bare metal: a linker script places the vector table, the code and the data in the
/// memory of the board.
fn link_bare(tc: &Toolchain, job: &LinkJob) -> Result<(), String> {
    let lld = tc.linker("ld.lld")?;
    let script = job.work_dir.join("nexul-bare.ld");
    std::fs::write(&script, bare_linker_script(job.target)).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(lld);
    cmd.arg("-o").arg(&job.out).arg("-T").arg(&script).arg("--gc-sections").arg("-static");
    for o in &job.objects {
        cmd.arg(o);
    }
    cmd.args(job.extra);
    run(&mut cmd, "the linker (ld.lld)")
}

/// Memory map of the reference boards emulated by QEMU (spec §7.1): `mps2-an386`
/// (Cortex-M4), `virt` (RISC-V and AArch64).
pub fn bare_linker_script(t: &Target) -> String {
    let (flash, ram) = match t.arch {
        Arch::Arm => (("0x00000000", "4M"), ("0x20000000", "4M")),
        Arch::Riscv32 | Arch::Riscv64 => (("0x80000000", "8M"), ("0x80800000", "56M")),
        _ => (("0x40000000", "8M"), ("0x40800000", "56M")),
    };
    format!(
        "MEMORY {{
  FLASH (rx) : ORIGIN = {}, LENGTH = {}
  RAM (rwx) : ORIGIN = {}, LENGTH = {}
}}
ENTRY(nx_vector_entry)
SECTIONS {{
  .text : {{
    KEEP(*(.vectors))
    *(.text .text.*)
    *(.rodata .rodata.*)
  }} > FLASH
  .data : {{
    __data_start = .;
    *(.data .data.* .sdata .sdata.*)
    __data_end = .;
  }} > RAM AT> FLASH
  __data_load = LOADADDR(.data);
  .bss (NOLOAD) : {{
    __bss_start = .;
    *(.bss .bss.* .sbss .sbss.* COMMON)
    __bss_end = .;
  }} > RAM
  . = ALIGN(16);
  __heap_start = .;
  __stack_top = ORIGIN(RAM) + LENGTH(RAM);
  /DISCARD/ : {{ *(.ARM.exidx*) *(.comment) }}
}}
",
        flash.0, flash.1, ram.0, ram.1
    )
}

// ---------------------------------------------------------------------------
// Archives

fn ar_header(name: &str, size: usize) -> Vec<u8> {
    let h = format!("{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n", name, "0", "0", "0", "644", size);
    debug_assert_eq!(h.len(), 60);
    h.into_bytes()
}

/// An archive with a symbol table: `members` are (name, data, symbols).
fn write_archive(members: &[(String, Vec<u8>, Vec<String>)]) -> Vec<u8> {
    // Long member names go to the "//" member.
    let mut longnames = Vec::new();
    let mut names = Vec::new();
    for (n, _, _) in members {
        if n.len() < 16 {
            names.push(format!("{}/", n));
        } else {
            names.push(format!("/{}", longnames.len()));
            longnames.extend_from_slice(n.as_bytes());
            longnames.extend_from_slice(b"/\n");
        }
    }
    let nsyms: usize = members.iter().map(|m| m.2.len()).sum();
    let strtab: usize = members.iter().flat_map(|m| m.2.iter()).map(|s| s.len() + 1).sum();
    let symtab_size = 4 + 4 * nsyms + strtab;
    let pad = |n: usize| n + (n & 1);
    let mut offset = 8 + 60 + pad(symtab_size);
    if !longnames.is_empty() {
        offset += 60 + pad(longnames.len());
    }
    let mut offsets = Vec::new();
    for (_, data, _) in members {
        offsets.push(offset);
        offset += 60 + pad(data.len());
    }
    let mut out = b"!<arch>\n".to_vec();
    out.extend(ar_header("/", symtab_size));
    out.extend((nsyms as u32).to_be_bytes());
    for (i, m) in members.iter().enumerate() {
        for _ in &m.2 {
            out.extend((offsets[i] as u32).to_be_bytes());
        }
    }
    for m in members {
        for s in &m.2 {
            out.extend(s.as_bytes());
            out.push(0);
        }
    }
    if symtab_size % 2 == 1 {
        out.push(b'\n');
    }
    if !longnames.is_empty() {
        out.extend(ar_header("//", longnames.len()));
        out.extend(&longnames);
        if longnames.len() % 2 == 1 {
            out.push(b'\n');
        }
    }
    for (i, (_, data, _)) in members.iter().enumerate() {
        out.extend(ar_header(&names[i], data.len()));
        out.extend(data);
        if data.len() % 2 == 1 {
            out.push(b'\n');
        }
    }
    out
}

/// A Windows import library made of short import objects (one per function): the linker
/// builds the import tables of the executable from them.
pub fn import_library(dll: &str, symbols: &[String], arch: Arch) -> Vec<u8> {
    let machine: u16 = if arch == Arch::Aarch64 { 0xAA64 } else { 0x8664 };
    let mut members = Vec::new();
    for s in symbols {
        let mut data = Vec::new();
        let size = (s.len() + 1 + dll.len() + 1) as u32;
        data.extend(0u16.to_le_bytes()); // Sig1: IMAGE_FILE_MACHINE_UNKNOWN
        data.extend(0xFFFFu16.to_le_bytes()); // Sig2
        data.extend(0u16.to_le_bytes()); // Version
        data.extend(machine.to_le_bytes());
        data.extend(0u32.to_le_bytes()); // TimeDateStamp
        data.extend(size.to_le_bytes());
        data.extend(0u16.to_le_bytes()); // Ordinal / hint
        // Type IMPORT_OBJECT_CODE (0) | NameType IMPORT_OBJECT_NAME (1) << 2
        data.extend((1u16 << 2).to_le_bytes());
        data.extend(s.as_bytes());
        data.push(0);
        data.extend(dll.as_bytes());
        data.push(0);
        members.push((dll.to_string(), data, vec![format!("__imp_{}", s), s.clone()]));
    }
    write_archive(&members)
}

/// A static library (`ar` archive) of object files, readable by every linker.
fn write_static_library(objects: &[PathBuf], out: &Path, target: &Target) -> Result<(), String> {
    let mut members = Vec::new();
    for o in objects {
        let data = std::fs::read(o).map_err(|e| format!("cannot read `{}`: {}", o.display(), e))?;
        let name = o.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "nexul.o".into());
        // The symbol table lists every global symbol defined by the object.
        let syms = crate::objsyms::defined_symbols(&data);
        members.push((name, data, syms));
    }
    // The linker of Apple reads only the BSD format of archives.
    let archive = if target.os == Os::MacOs { write_bsd_archive(&members) } else { write_archive(&members) };
    std::fs::write(out, archive).map_err(|e| format!("cannot write `{}`: {}", out.display(), e))
}

/// An archive in the BSD format of Apple: names written as `#1/<length>` at the start of
/// the member, a `__.SYMDEF` symbol table (little-endian `ranlib` entries), and the content
/// of each member at an offset that is a multiple of 8 (the linker requires it).
fn write_bsd_archive(members: &[(String, Vec<u8>, Vec<String>)]) -> Vec<u8> {
    // A member at `at`: the length of its padded name, its size (padded name, content
    // padded to 8 bytes), and its whole length with the header.
    fn layout(at: usize, name: &str, content: usize) -> (usize, usize, usize) {
        let mut name_len = name.len() + 1;
        while (at + 60 + name_len) % 8 != 0 {
            name_len += 1;
        }
        let mut size = name_len + content;
        while size % 8 != 0 {
            size += 1;
        }
        (name_len, size, 60 + size)
    }
    fn push(out: &mut Vec<u8>, name: &str, content: &[u8]) {
        let (name_len, size, _) = layout(out.len(), name, content.len());
        let header = format!("{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n", format!("#1/{}", name_len), "0", "0", "0", "644", size);
        debug_assert_eq!(header.len(), 60);
        out.extend(header.as_bytes());
        let start = out.len();
        out.extend(name.as_bytes());
        out.resize(start + name_len, 0);
        out.extend(content);
        out.resize(start + size, 0);
    }
    let symdef = "__.SYMDEF";
    let mut strtab = Vec::new();
    let mut entries = Vec::new();
    for (i, m) in members.iter().enumerate() {
        for s in &m.2 {
            entries.push((strtab.len() as u32, i));
            strtab.extend_from_slice(s.as_bytes());
            strtab.push(0);
        }
    }
    while strtab.len() % 8 != 0 {
        strtab.push(0);
    }
    let table_len = 4 + 8 * entries.len() + 4 + strtab.len();
    let mut at = 8 + layout(8, symdef, table_len).2;
    let mut offsets = Vec::new();
    for (n, data, _) in members {
        offsets.push(at);
        at += layout(at, n, data.len()).2;
    }
    let mut table = Vec::new();
    table.extend(((8 * entries.len()) as u32).to_le_bytes());
    for (strx, member) in &entries {
        table.extend(strx.to_le_bytes());
        table.extend((offsets[*member] as u32).to_le_bytes());
    }
    table.extend((strtab.len() as u32).to_le_bytes());
    table.extend(&strtab);
    let mut out = b"!<arch>\n".to_vec();
    push(&mut out, symdef, &table);
    for (n, data, _) in members {
        push(&mut out, n, data);
    }
    debug_assert_eq!(out.len(), at);
    out
}

#[cfg(test)]
mod bsd_archive_tests {
    use super::write_bsd_archive;

    /// Each member: its header, its name, then its content at a multiple of 8; the symbol
    /// table points at the header of the member that defines each symbol.
    #[test]
    fn members_are_aligned_and_indexed() {
        let members = vec![
            ("a.o".to_string(), vec![1u8; 13], vec!["_f".to_string(), "_g".to_string()]),
            ("a_much_longer_name.o".to_string(), vec![2u8; 8], vec!["_h".to_string()]),
        ];
        let ar = write_bsd_archive(&members);
        assert_eq!(&ar[..8], b"!<arch>\n");
        let mut at = 8;
        let mut found = Vec::new();
        while at < ar.len() {
            let header = std::str::from_utf8(&ar[at..at + 60]).unwrap();
            assert_eq!(&header[58..], "`\n");
            let name_len: usize = header[3..16].trim().parse().unwrap();
            let size: usize = header[48..58].trim().parse().unwrap();
            let name = &ar[at + 60..at + 60 + name_len];
            let name = std::str::from_utf8(name).unwrap().trim_end_matches('\0').to_string();
            assert_eq!((at + 60 + name_len) % 8, 0, "content of {} not on 8 bytes", name);
            found.push((name, at, name_len));
            at += 60 + size;
            assert_eq!(at % 2, 0);
        }
        assert_eq!(at, ar.len());
        let names: Vec<&str> = found.iter().map(|(n, _, _)| n.as_str()).collect();
        assert_eq!(names, ["__.SYMDEF", "a.o", "a_much_longer_name.o"]);
        // The table: ranlib entries (string index, member offset), then the strings.
        let t = found[0].1 + 60 + found[0].2;
        let u = |o: usize| u32::from_le_bytes(ar[o..o + 4].try_into().unwrap()) as usize;
        assert_eq!(u(t), 24);
        let strings = t + 4 + 24 + 4;
        let entries: Vec<(String, usize)> = (0..3)
            .map(|i| {
                let s = strings + u(t + 4 + 8 * i);
                let end = ar[s..].iter().position(|&c| c == 0).unwrap();
                (String::from_utf8(ar[s..s + end].to_vec()).unwrap(), u(t + 8 + 8 * i))
            })
            .collect();
        assert_eq!(entries, [("_f".to_string(), found[1].1), ("_g".to_string(), found[1].1), ("_h".to_string(), found[2].1)]);
    }
}

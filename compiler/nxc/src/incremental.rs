//! The incremental build (ADR 0016): at `-O0`, the program is cut into units
//! (`nxc_codegen_llvm::units`), and the object of each unit is cached by the text of
//! its IR. After a change, LLVM only compiles the units whose text changed; the others
//! come from the cache. The objects of the program are then linked as before.
//!
//! The key of an object covers the text of its unit, the build of `nxc` (a hash of its
//! executable, which embeds the standard library), the LLVM library, the target and the
//! options of the code generator. The cache is `objects/v1` in the build cache; an
//! object is written under a temporary name and then renamed, so that a build
//! interrupted, or two builds at once, never leave a partial object under a key.

use crate::toolchain::Toolchain;
use crate::{cache_dir, code_model, llvm, unique_suffix, BuildOptions};
use nxc_codegen_llvm::units;
use nxc_codegen_llvm::Output;
use crate::OutputKind;
use nxc_sema::target::{Arch, Os};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Is this build cut into units? At `-O0` only (a single module keeps the optimizations
/// across functions), for executables of Linux on x86-64 and AArch64 (ADR 0016,
/// decision 5); not for programs whose stack is bounded. `NEXUL_INCREMENTAL=0`
/// disables it.
pub fn eligible(output: &Output, opts: &BuildOptions) -> bool {
    if std::env::var_os("NEXUL_INCREMENTAL").is_some_and(|v| v == "0") {
        return false;
    }
    let t = &opts.target;
    opts.level() == '0'
        && opts.kind == OutputKind::Executable
        && t.os == Os::Linux
        && matches!(t.arch, Arch::X86_64 | Arch::Aarch64)
        && output.stack.is_none()
}

/// `NEXUL_INCREMENTAL=1`: an error of the incremental build is an error of the build,
/// instead of a fallback to one module.
pub fn strict() -> bool {
    std::env::var_os("NEXUL_INCREMENTAL").is_some_and(|v| v == "1")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// The identity of a file: its path, size and date of modification.
fn stamp(p: &Path) -> String {
    let m = std::fs::metadata(p).ok();
    let size = m.as_ref().map(|m| m.len()).unwrap_or(0);
    let date = m
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}|{}|{}", p.display(), size, date)
}

/// SHA-256 of the executable of `nxc`, computed once per build of `nxc` (remembered in
/// the cache by its path, size and date).
fn compiler_id(cache: Option<&Path>) -> String {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let Ok(exe) = std::env::current_exe() else {
            return format!("nxc {}", env!("CARGO_PKG_VERSION"));
        };
        let memo = cache.map(|d| d.join("ids").join(hex(&crate::pkg::sha256(stamp(&exe).as_bytes()))[..32].to_string()));
        if let Some(m) = &memo {
            if let Ok(s) = std::fs::read_to_string(m) {
                if s.len() == 64 {
                    return s;
                }
            }
        }
        let id = match std::fs::read(&exe) {
            Ok(bytes) => hex(&crate::pkg::sha256(&bytes)),
            Err(_) => return format!("nxc {} {}", env!("CARGO_PKG_VERSION"), stamp(&exe)),
        };
        if let Some(m) = &memo {
            if let Some(d) = m.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let tmp = m.with_extension(format!("tmp-{}", unique_suffix()));
            if std::fs::write(&tmp, &id).is_ok() && std::fs::rename(&tmp, m).is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
        }
        id
    })
    .clone()
}

/// Does `p` start like an ELF object, and is it longer than its header?
fn is_object(p: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(p) else { return false };
    let mut head = [0u8; 4];
    f.read_exact(&mut head).is_ok() && head == *b"\x7fELF" && f.metadata().map(|m| m.len() > 64).unwrap_or(false)
}

/// Keeps the `keep` most recent objects of `dir` once it holds more than `max`.
fn trim(dir: &Path, max: usize, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if entries.len() <= max {
        return;
    }
    entries.sort();
    for (_, p) in &entries[..entries.len() - keep] {
        let _ = std::fs::remove_file(p);
    }
}

/// The objects of the program: one per unit, compiled by LLVM when the cache does not
/// have it, in parallel.
pub fn objects(output: &Output, opts: &BuildOptions, work: &Path) -> Result<Vec<PathBuf>, String> {
    let tc = Toolchain::find();
    let t = &opts.target;
    let cg = llvm::CodegenOptions {
        triple: &t.llvm_triple,
        cpu: &t.cpu,
        features: &t.features,
        opt_level: opts.level(),
        pic: opts.kind != OutputKind::Executable && t.os != Os::Windows,
        code_model: code_model(t),
    };
    let t0 = std::time::Instant::now();
    let units = units::split(&output.parts);
    let t_split = t0.elapsed();
    let cache = cache_dir().map(|d| d.join("objects").join("v1")).filter(|d| std::fs::create_dir_all(d).is_ok());
    let context = format!(
        "nexul-object-v1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
        compiler_id(cache.as_deref().and_then(|d| d.parent()).and_then(|d| d.parent())),
        tc.llvm_lib.as_deref().map(stamp).unwrap_or_default(),
        cg.triple,
        cg.cpu,
        cg.features,
        cg.opt_level,
        cg.pic,
        cg.code_model,
    );
    // The keys, in parallel: SHA-256 of the context and of the text of each unit.
    let keys: Vec<String> = match &cache {
        Some(_) => std::thread::scope(|s| {
            let hs: Vec<_> = units
                .chunks(units.len().div_ceil(4).max(1))
                .map(|chunk| {
                    let context = &context;
                    s.spawn(move || {
                        chunk
                            .iter()
                            .map(|u| {
                                let mut keyed = context.clone().into_bytes();
                                keyed.extend_from_slice(u.ir.as_bytes());
                                hex(&crate::pkg::sha256(&keyed)[..16])
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            hs.into_iter().flat_map(|h| h.join().expect("hashing a unit")).collect()
        }),
        None => Vec::new(),
    };
    let mut paths = Vec::with_capacity(units.len());
    let mut todo: Vec<(usize, PathBuf)> = Vec::new();
    for (i, _) in units.iter().enumerate() {
        let path = match &cache {
            Some(d) => d.join(format!("{}.o", keys[i])),
            None => work.join(format!("unit{}.o", i)),
        };
        // An object that is not one (truncated, overwritten) is compiled again.
        let hit = cache.is_some() && is_object(&path);
        if hit {
            // Recently used objects are the last ones removed.
            let _ = std::fs::File::options().write(true).open(&path).and_then(|f| f.set_modified(std::time::SystemTime::now()));
        } else {
            todo.push((i, path.clone()));
        }
        paths.push(path);
    }
    let t_keys = t0.elapsed() - t_split;
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(todo.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let errors: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let j = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some((i, path)) = todo.get(j) else { break };
                let tmp = if cache.is_some() { path.with_extension(format!("tmp-{}", unique_suffix())) } else { path.clone() };
                let r = llvm::compile(&tc, &units[*i].ir, &tmp, &cg, false).and_then(|_| {
                    if cache.is_some() {
                        std::fs::rename(&tmp, path).map_err(|e| {
                            let _ = std::fs::remove_file(&tmp);
                            format!("cannot write `{}`: {}", path.display(), e)
                        })
                    } else {
                        Ok(())
                    }
                });
                if let Err(e) = r {
                    let _ = std::fs::remove_file(&tmp);
                    errors.lock().unwrap().push(format!("unit `{}`: {}", units[*i].name, e));
                }
            });
        }
    });
    if let Some(e) = errors.into_inner().unwrap().into_iter().next() {
        return Err(e);
    }
    if std::env::var_os("NEXUL_INCREMENTAL_REPORT").is_some() {
        let t_llvm = t0.elapsed() - t_split - t_keys;
        eprintln!(
            "incremental: {} units, {} compiled (cut {} ms, keys {} ms, LLVM {} ms)",
            units.len(),
            todo.len(),
            t_split.as_millis(),
            t_keys.as_millis(),
            t_llvm.as_millis()
        );
    }
    if let Some(d) = &cache {
        trim(d, 20_000, 15_000);
    }
    Ok(paths)
}

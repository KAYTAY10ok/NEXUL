//! Locating the code generation infrastructure: the LLVM library and the LLD linkers
//! (spec §10.6). Search order:
//!
//! 1. the environment variable `NEXUL_LLVM` (a directory containing the LLVM library and
//!    the linkers, or its `bin` and `lib` subdirectories);
//! 2. a toolchain installed with NEXUL: `<nxc directory>/llvm/` or `<nxc directory>/../llvm/`;
//! 3. the usual installation folders of LLVM (`C:\Program Files\LLVM`, `/usr/lib/llvm-*`,
//!    Homebrew) and the `PATH`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Debug, Default)]
pub struct Toolchain {
    /// The LLVM C library (`LLVM-C.dll`, `libLLVM.so`, `libLLVM.dylib`).
    pub llvm_lib: Option<PathBuf>,
    /// Directory of the LLD linkers (`lld-link`, `ld.lld`, `wasm-ld`, `ld64.lld`).
    pub bin: Option<PathBuf>,
}

const LIB_NAMES: &[&str] = if cfg!(windows) {
    &["LLVM-C.dll"]
} else if cfg!(target_os = "macos") {
    &["libLLVM.dylib", "libLLVM-C.dylib"]
} else {
    &["libLLVM.so", "libLLVM.so.1"]
};

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    }
}

/// Candidate directories, in order of preference.
fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = std::env::var_os("NEXUL_LLVM") {
        let d = PathBuf::from(d);
        dirs.push(d.join("bin"));
        dirs.push(d.join("lib"));
        dirs.push(d);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for base in [dir.join("llvm"), dir.join("..").join("llvm")] {
                dirs.push(base.join("bin"));
                dirs.push(base.join("lib"));
            }
        }
    }
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramW6432"] {
            if let Some(pf) = std::env::var_os(var) {
                dirs.push(Path::new(&pf).join("LLVM").join("bin"));
            }
        }
        dirs.push(PathBuf::from(r"C:\Program Files\LLVM\bin"));
    } else {
        for v in (14..=40).rev() {
            dirs.push(PathBuf::from(format!("/usr/lib/llvm-{}/lib", v)));
            dirs.push(PathBuf::from(format!("/usr/lib/llvm-{}/bin", v)));
            dirs.push(PathBuf::from(format!("/usr/lib/llvm{}/lib", v)));
        }
        dirs.push(PathBuf::from("/opt/homebrew/opt/llvm/lib"));
        dirs.push(PathBuf::from("/opt/homebrew/opt/llvm/bin"));
        dirs.push(PathBuf::from("/usr/local/opt/llvm/lib"));
        dirs.push(PathBuf::from("/usr/local/opt/llvm/bin"));
        dirs.push(PathBuf::from("/usr/lib"));
        dirs.push(PathBuf::from("/usr/lib64"));
        dirs.push(PathBuf::from("/usr/lib/x86_64-linux-gnu"));
        dirs.push(PathBuf::from("/usr/lib/aarch64-linux-gnu"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs
}

impl Toolchain {
    pub fn find() -> Toolchain {
        static FOUND: OnceLock<Toolchain> = OnceLock::new();
        FOUND.get_or_init(Toolchain::search).clone()
    }

    fn search() -> Toolchain {
        let dirs = candidate_dirs();
        let mut tc = Toolchain::default();
        'lib: for d in &dirs {
            for n in LIB_NAMES {
                let p = d.join(n);
                if p.is_file() {
                    tc.llvm_lib = Some(p);
                    break 'lib;
                }
            }
            // Versioned names on Linux: libLLVM-18.so, libLLVM.so.18.1...
            if !cfg!(windows) {
                if let Ok(rd) = std::fs::read_dir(d) {
                    let mut found: Vec<PathBuf> = rd
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| p.file_name().map_or(false, |n| n.to_string_lossy().starts_with("libLLVM") && n.to_string_lossy().contains(".so")))
                        .collect();
                    found.sort();
                    if let Some(p) = found.pop() {
                        tc.llvm_lib = Some(p);
                        break 'lib;
                    }
                }
            }
        }
        // The linkers: preferably next to the library.
        let mut bins: Vec<PathBuf> = Vec::new();
        if let Some(lib) = &tc.llvm_lib {
            if let Some(dir) = lib.parent() {
                bins.push(dir.to_path_buf());
                bins.push(dir.join("..").join("bin"));
            }
        }
        bins.extend(dirs.iter().cloned());
        for d in bins {
            if d.join(exe("ld.lld")).is_file() || d.join(exe("lld-link")).is_file() || d.join(exe("lld")).is_file() {
                tc.bin = Some(d);
                break;
            }
        }
        tc
    }

    pub fn missing_llvm() -> String {
        "LLVM was not found: NEXUL uses LLVM as its code generation infrastructure.\n\
         Install LLVM (https://llvm.org, version 17 or later) or set NEXUL_LLVM to its directory."
            .to_string()
    }

    /// The path of an LLD linker flavour (`lld-link`, `ld.lld`, `wasm-ld`, `ld64.lld`).
    pub fn linker(&self, flavour: &str) -> Result<PathBuf, String> {
        let Some(bin) = &self.bin else {
            return Err("the LLD linker was not found; install LLVM (with LLD) or set NEXUL_LLVM".into());
        };
        let direct = bin.join(exe(flavour));
        if direct.is_file() {
            return Ok(direct);
        }
        Err(format!("the linker `{}` was not found in `{}`", flavour, bin.display()))
    }
}

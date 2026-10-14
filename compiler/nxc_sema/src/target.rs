//! Compilation targets (spec §10.5) and conditional compilation (`@cfg`, spec §6.8).
//!
//! A target is chosen by a short name (`x86_64-windows`, `aarch64-linux`, `wasm32-wasi`,
//! `thumbv7em-none`...) or by its LLVM triple. The front end uses it to evaluate `@cfg`
//! predicates and to select the system layer of the standard library (`std.sys`); the
//! back end uses it for the data layout and the object format.

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Os {
    Windows,
    Linux,
    MacOs,
    /// WebAssembly System Interface.
    Wasi,
    /// No operating system (firmware, bare metal).
    None,
}

impl Os {
    pub fn name(self) -> &'static str {
        match self {
            Os::Windows => "windows",
            Os::Linux => "linux",
            Os::MacOs => "macos",
            Os::Wasi => "wasi",
            Os::None => "none",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Arch {
    X86_64,
    Aarch64,
    Riscv64,
    /// 32-bit ARM (Thumb-2, Cortex-M).
    Arm,
    Riscv32,
    Wasm32,
}

impl Arch {
    pub fn name(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
            Arch::Riscv64 => "riscv64",
            Arch::Arm => "arm",
            Arch::Riscv32 => "riscv32",
            Arch::Wasm32 => "wasm32",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ObjectFormat {
    Coff,
    Elf,
    MachO,
    Wasm,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// Canonical short name, e.g. `x86_64-windows`.
    pub name: String,
    pub llvm_triple: String,
    pub os: Os,
    pub arch: Arch,
    pub ptr_bits: u32,
    /// LLVM CPU name and feature string.
    pub cpu: String,
    pub features: String,
    /// Hardware double-precision floating point (otherwise a software implementation is
    /// linked from `std.builtins`).
    pub hard_float64: bool,
    /// Native 64-bit integer division (otherwise `std.builtins` provides it).
    pub native_div64: bool,
}

/// The targets officially known by `nxc` (spec §10.5). Each entry: short name, LLVM
/// triple, OS, architecture, pointer width, CPU, features, hard f64, native 64-bit div.
const TARGETS: &[(&str, &str, Os, Arch, u32, &str, &str, bool, bool)] = &[
    ("x86_64-windows", "x86_64-pc-windows-msvc", Os::Windows, Arch::X86_64, 64, "x86-64", "", true, true),
    ("aarch64-windows", "aarch64-pc-windows-msvc", Os::Windows, Arch::Aarch64, 64, "generic", "+neon", true, true),
    ("x86_64-linux", "x86_64-unknown-linux-gnu", Os::Linux, Arch::X86_64, 64, "x86-64", "", true, true),
    ("aarch64-linux", "aarch64-unknown-linux-gnu", Os::Linux, Arch::Aarch64, 64, "generic", "+neon", true, true),
    ("riscv64-linux", "riscv64-unknown-linux-gnu", Os::Linux, Arch::Riscv64, 64, "generic-rv64", "+m,+a,+f,+d,+c", true, true),
    ("x86_64-macos", "x86_64-apple-macosx11.0.0", Os::MacOs, Arch::X86_64, 64, "x86-64", "", true, true),
    ("aarch64-macos", "arm64-apple-macosx11.0.0", Os::MacOs, Arch::Aarch64, 64, "apple-m1", "", true, true),
    ("wasm32-wasi", "wasm32-unknown-wasi", Os::Wasi, Arch::Wasm32, 32, "generic", "+bulk-memory,+mutable-globals,+sign-ext,+nontrapping-fptoint", true, true),
    // Microcontrollers and bare-metal boards (spec §7).
    ("thumbv7em-none", "thumbv7em-none-eabihf", Os::None, Arch::Arm, 32, "cortex-m4", "+vfp4d16sp,+thumb-mode", false, false),
    ("thumbv7m-none", "thumbv7m-none-eabi", Os::None, Arch::Arm, 32, "cortex-m3", "+soft-float,+thumb-mode", false, false),
    ("riscv32-none", "riscv32-unknown-none-elf", Os::None, Arch::Riscv32, 32, "generic-rv32", "+m,+a,+c", false, false),
    ("aarch64-none", "aarch64-unknown-none", Os::None, Arch::Aarch64, 64, "cortex-a53", "+neon", true, true),
    ("riscv64-none", "riscv64-unknown-none-elf", Os::None, Arch::Riscv64, 64, "generic-rv64", "+m,+a,+f,+d,+c", true, true),
];

/// Every `@cfg` predicate name (spec §6.8).
pub const CFG_NAMES: &[&str] = &[
    "windows", "linux", "macos", "wasi", "bare", "unix", "x86_64", "aarch64", "arm", "riscv64", "riscv32", "riscv", "wasm32", "wasm",
    "ptr32", "ptr64", "debug", "release", "test", "hard_float", "soft_float",
];

impl Target {
    fn from_entry(e: &(&str, &str, Os, Arch, u32, &str, &str, bool, bool)) -> Target {
        Target {
            name: e.0.to_string(),
            llvm_triple: e.1.to_string(),
            os: e.2,
            arch: e.3,
            ptr_bits: e.4,
            cpu: e.5.to_string(),
            features: e.6.to_string(),
            hard_float64: e.7,
            native_div64: e.8,
        }
    }

    /// The target of the machine running the compiler.
    pub fn host() -> Target {
        let name = match (std::env::consts::ARCH, std::env::consts::OS) {
            ("x86_64", "windows") => "x86_64-windows",
            ("aarch64", "windows") => "aarch64-windows",
            ("x86_64", "linux") => "x86_64-linux",
            ("aarch64", "linux") => "aarch64-linux",
            ("riscv64", "linux") => "riscv64-linux",
            ("x86_64", "macos") => "x86_64-macos",
            ("aarch64", "macos") => "aarch64-macos",
            _ => "x86_64-linux",
        };
        Target::from_name(name).unwrap()
    }

    /// Finds a target by short name (`aarch64-linux`) or by LLVM-style triple
    /// (`aarch64-unknown-linux-gnu`, `x86_64-linux-gnu`, `wasm32-wasi`).
    pub fn from_name(s: &str) -> Result<Target, String> {
        let s = s.trim();
        if let Some(e) = TARGETS.iter().find(|e| e.0 == s || e.1 == s) {
            return Ok(Target::from_entry(e));
        }
        // Accept common spellings: <arch>-<vendor>-<os>[-<env>] and <arch>-<os>[-<env>].
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() >= 2 {
            let arch = match parts[0] {
                "x86_64" | "amd64" | "x64" => "x86_64",
                "aarch64" | "arm64" => "aarch64",
                "riscv64" | "riscv64gc" => "riscv64",
                "riscv32" | "riscv32imac" | "riscv32imc" => "riscv32",
                "wasm32" => "wasm32",
                "thumbv7em" => "thumbv7em",
                "thumbv7m" => "thumbv7m",
                _ => "",
            };
            let os = if parts.iter().any(|p| *p == "windows") {
                "windows"
            } else if parts.iter().any(|p| *p == "linux") {
                "linux"
            } else if parts.iter().any(|p| p.starts_with("macos") || p.starts_with("darwin") || *p == "apple") {
                "macos"
            } else if parts.iter().any(|p| *p == "wasi") {
                "wasi"
            } else if parts.iter().any(|p| *p == "none" || *p == "elf" || *p == "eabi" || *p == "eabihf") {
                "none"
            } else {
                ""
            };
            if !arch.is_empty() && !os.is_empty() {
                let name = format!("{}-{}", arch, os);
                if let Some(e) = TARGETS.iter().find(|e| e.0 == name) {
                    return Ok(Target::from_entry(e));
                }
            }
        }
        Err(format!("unknown target `{}`; known targets: {}", s, Target::known_names().join(", ")))
    }

    pub fn known_names() -> Vec<&'static str> {
        TARGETS.iter().map(|e| e.0).collect()
    }

    pub fn object_format(&self) -> ObjectFormat {
        match self.os {
            Os::Windows => ObjectFormat::Coff,
            Os::MacOs => ObjectFormat::MachO,
            Os::Wasi => ObjectFormat::Wasm,
            _ if self.arch == Arch::Wasm32 => ObjectFormat::Wasm,
            _ => ObjectFormat::Elf,
        }
    }

    pub fn ptr_bytes(&self) -> u64 {
        (self.ptr_bits / 8) as u64
    }

    /// Executable file name for this target.
    pub fn exe_name(&self, stem: &str) -> String {
        match self.os {
            Os::Windows => format!("{}.exe", stem),
            Os::Wasi => format!("{}.wasm", stem),
            Os::None => format!("{}.elf", stem),
            _ => stem.to_string(),
        }
    }
}

/// The configuration seen by `@cfg`: target and build mode.
#[derive(Clone, Debug)]
pub struct Cfg {
    pub target: Target,
    pub release: bool,
    pub test: bool,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg { target: Target::host(), release: false, test: false }
    }
}

impl Cfg {
    /// Evaluates one predicate name (without negation).
    fn atom(&self, name: &str) -> Option<bool> {
        let t = &self.target;
        Some(match name {
            "windows" => t.os == Os::Windows,
            "linux" => t.os == Os::Linux,
            "macos" => t.os == Os::MacOs,
            "wasi" => t.os == Os::Wasi,
            "bare" => t.os == Os::None,
            "unix" => matches!(t.os, Os::Linux | Os::MacOs),
            "x86_64" => t.arch == Arch::X86_64,
            "aarch64" => t.arch == Arch::Aarch64,
            "arm" => t.arch == Arch::Arm,
            "riscv64" => t.arch == Arch::Riscv64,
            "riscv32" => t.arch == Arch::Riscv32,
            "riscv" => matches!(t.arch, Arch::Riscv32 | Arch::Riscv64),
            "wasm32" | "wasm" => t.arch == Arch::Wasm32,
            "ptr32" => t.ptr_bits == 32,
            "ptr64" => t.ptr_bits == 64,
            "debug" => !self.release,
            "release" => self.release,
            "test" => self.test,
            "hard_float" => t.hard_float64,
            "soft_float" => !t.hard_float64,
            _ => return None,
        })
    }

    /// Evaluates a predicate string: `name`, `!name`, or alternatives `a|b|!c`.
    /// Returns `Err(name)` for an unknown name.
    pub fn eval(&self, pred: &str) -> Result<bool, String> {
        let mut any = false;
        for alt in pred.split('|') {
            let alt = alt.trim();
            let (neg, name) = match alt.strip_prefix('!') {
                Some(n) => (true, n.trim()),
                None => (false, alt),
            };
            match self.atom(name) {
                Some(v) => any |= v != neg,
                None => return Err(name.to_string()),
            }
        }
        Ok(any)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_triples() {
        assert_eq!(Target::from_name("x86_64-linux-gnu").unwrap().name, "x86_64-linux");
        assert_eq!(Target::from_name("aarch64-unknown-linux-gnu").unwrap().name, "aarch64-linux");
        assert_eq!(Target::from_name("wasm32-wasi").unwrap().os, Os::Wasi);
        assert_eq!(Target::from_name("x86_64-pc-windows-msvc").unwrap().name, "x86_64-windows");
        assert!(Target::from_name("sparc-solaris").is_err());
    }

    #[test]
    fn cfg_predicates() {
        let c = Cfg { target: Target::from_name("x86_64-linux").unwrap(), release: true, test: false };
        assert_eq!(c.eval("linux"), Ok(true));
        assert_eq!(c.eval("!windows"), Ok(true));
        assert_eq!(c.eval("windows|unix"), Ok(true));
        assert_eq!(c.eval("debug"), Ok(false));
        assert_eq!(c.eval("plan9"), Err("plan9".to_string()));
    }
}

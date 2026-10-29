//! The NEXUL compiler driver: front end, NIR, LLVM IR back end, code generation with LLVM
//! and linking with LLD.

pub mod codes;
pub mod ide;
pub mod link;
pub mod llvm;
pub mod migrate;
pub mod lsp;
mod objsyms;
pub mod pkg;
mod stack;
pub mod toolchain;

pub use link::OutputKind;
pub use nxc_codegen_llvm::{Entry, Output};
pub use nxc_sema::target::{Arch, Cfg, Os, Target};

use nxc_sema::defs::{FnId, ItemRef};
use nxc_sema::ProgramSource;
use nxc_syntax::diag::render_all;
use nxc_syntax::{DiagSink, Diagnostic, Level, SourceMap, Span};
use std::path::{Path, PathBuf};
use toolchain::Toolchain;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Analysis only.
    Check,
    /// A program with `main`.
    Program,
    /// The `@test` functions.
    Tests,
    /// A library: the `@export` functions.
    Library,
}

/// What to compile, and how.
#[derive(Clone, Debug)]
pub struct CompileOptions {
    pub mode: Mode,
    pub color: bool,
    pub target: Target,
    pub release: bool,
    /// Set `@cfg(test)` without requesting test code generation (used by `nxc lsp`).
    pub test_cfg: bool,
    pub debug_info: bool,
    pub profile: bool,
    /// Dependencies (`nxpm`): package name and source directory.
    pub deps: Vec<(String, PathBuf)>,
    /// Unsaved imported sources, keyed by their filesystem path.
    pub overlays: Vec<(PathBuf, String)>,
    /// The syntax of the root file (ADR 0010): `--edition 0.2` (default) or `--edition 1`.
    pub edition: nxc_syntax::Edition,
    /// The syntax of the other files of its project without `// edition:` (`None`: `edition`).
    pub project_edition: Option<nxc_syntax::Edition>,
    /// The syntax of each dependency (its `nexul.toml`).
    pub dep_editions: Vec<(String, nxc_syntax::Edition)>,
}

impl CompileOptions {
    pub fn new(mode: Mode) -> CompileOptions {
        CompileOptions {
            mode,
            color: false,
            target: Target::host(),
            release: false,
            test_cfg: false,
            debug_info: true,
            profile: false,
            deps: Vec::new(),
            overlays: Vec::new(),
            edition: nxc_syntax::Edition::V0_2,
            project_edition: None,
            dep_editions: Vec::new(),
        }
    }
}

pub struct Compilation {
    /// Rendered diagnostics (errors and warnings).
    pub diagnostics: String,
    pub errors: usize,
    pub warnings: usize,
    /// The generated module (when there is no error and code was requested).
    pub output: Option<Output>,
    /// Names of the test functions, in the order of their index.
    pub tests: Vec<String>,
    /// Those of them that pass when they panic (`expects panic`).
    pub expects_panic: Vec<String>,
    /// The same diagnostics as a JSON array (for editors and tools).
    pub json: String,
    /// Versioned diagnostic JSON (schema `nexul.diagnostics`, version 1).
    pub json_v1: String,
}

/// Runs the whole front end and generates LLVM IR (unless `mode` is `Check`).
pub fn compile_file(path: &Path, opts: &CompileOptions) -> Compilation {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "main.nx".into());
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            let msg = format!("cannot read `{}`: {}", path.display(), e);
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "main.nx".into());
            let mut sm = SourceMap::new();
            let file = sm.add(name, "");
            let diagnostic = Diagnostic::error("", msg.clone())
                .primary(Span::new(file, 0, 0), "cannot read source file");
            return Compilation {
                diagnostics: format!("error: {}\n", msg),
                errors: 1,
                warnings: 0,
                output: None,
                tests: vec![],
                expects_panic: vec![],
                json: format!(
                    "[{{\"level\":\"error\",\"code\":\"\",\"message\":{},\"file\":\"\",\"line\":0,\"col\":0,\"end_line\":0,\"end_col\":0,\"labels\":[],\"notes\":[]}}]",
                    json_str(&msg)
                ),
                json_v1: diagnostics_json_v1(&[diagnostic], &sm, path.parent(), &opts.target),
            };
        }
    };
    let root_dir = path.parent().map(|p| {
        if p.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            p.to_path_buf()
        }
    });
    compile_source(&name, bytes, root_dir, opts)
}

pub fn compile_source(
    name: &str,
    bytes: Vec<u8>,
    root_dir: Option<PathBuf>,
    opts: &CompileOptions,
) -> Compilation {
    let diagnostic_root_dir = root_dir.clone();
    let mut sm = SourceMap::new();
    let diags = DiagSink::new();
    let color = opts.color;
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => {
            let pos = e.utf8_error().valid_up_to();
            let lossy = String::from_utf8_lossy(e.as_bytes()).to_string();
            let f = sm.add(name, lossy);
            diags.emit(
                Diagnostic::error("E0001", "source file is not valid UTF-8")
                    .primary(
                        Span::new(f, pos as u32, pos as u32 + 1),
                        "invalid byte sequence",
                    )
                    .help("save the file with the UTF-8 encoding"),
            );
            return finish(
                &sm,
                &diags,
                None,
                vec![],
                color,
                diagnostic_root_dir.as_deref(),
                &opts.target,
            );
        }
    };
    let text = text
        .strip_prefix('\u{feff}')
        .map(|s| s.to_string())
        .unwrap_or(text);
    let mut src = ProgramSource::new(name.to_string(), text, root_dir.clone());
    src.deps = opts.deps.clone();
    src.edition = opts.edition;
    src.project_edition = opts.project_edition;
    src.dep_editions = opts.dep_editions.clone();
    src.overlays = opts.overlays.clone();
    src.cfg = Cfg {
        target: opts.target.clone(),
        release: opts.release,
        test: opts.test_cfg || opts.mode == Mode::Tests,
    };
    let prog = nxc_sema::analyze(src, &mut sm, &diags);
    if diags.has_errors() {
        return finish(
            &sm,
            &diags,
            None,
            vec![],
            color,
            diagnostic_root_dir.as_deref(),
            &opts.target,
        );
    }
    let bodies = nxc_nir::lower_program(&prog, &sm, &diags);
    if diags.has_errors() || opts.mode == Mode::Check {
        return finish(
            &sm,
            &diags,
            None,
            vec![],
            color,
            diagnostic_root_dir.as_deref(),
            &opts.target,
        );
    }
    let entry = match opts.mode {
        Mode::Program => match prog.main_fn {
            Some(f) => Entry::Main(f),
            None => {
                let root_file = prog.modules[prog.root.idx()].file;
                diags.emit(
                    Diagnostic::error("E0230", "no `main` function")
                        .primary(Span::new(root_file, 0, 0), "the program starts with `fn main()` in this file")
                        .help("add `fn main() { ... }`, use `nxc test` to run `@test` functions, or `--lib` to build a library"),
                );
                return finish(
                    &sm,
                    &diags,
                    None,
                    vec![],
                    color,
                    diagnostic_root_dir.as_deref(),
                    &opts.target,
                );
            }
        },
        Mode::Tests => {
            let mut tests: Vec<FnId> = Vec::new();
            for m in prog.modules.iter().filter(|m| !m.is_std) {
                let mut fns: Vec<(u32, FnId)> = m
                    .items
                    .values()
                    .filter_map(|(it, span, _)| match it {
                        ItemRef::Fn(f) if prog.func(*f).is_test => Some((span.lo, *f)),
                        _ => None,
                    })
                    .collect();
                fns.sort();
                tests.extend(fns.into_iter().map(|(_, f)| f));
            }
            Entry::Tests(tests)
        }
        Mode::Library => Entry::Library,
        Mode::Check => unreachable!(),
    };
    let test_name = |f: FnId| {
        let fd = prog.func(f);
        let m = &prog.modules[fd.module.idx()].path;
        format!("{}.{}", m, fd.name)
    };
    let (test_names, expects_panic) = match &entry {
        Entry::Tests(t) => (t.iter().map(|&f| test_name(f)).collect(), t.iter().filter(|&&f| prog.func(f).expects_panic).map(|&f| test_name(f)).collect()),
        _ => (vec![], vec![]),
    };
    let root = root_dir
        .and_then(|d| std::fs::canonicalize(&d).ok().or(Some(d)))
        .map(|d| {
            d.to_string_lossy()
                .trim_start_matches(r"\\?\")
                .replace('\\', "/")
        })
        .unwrap_or_default();
    let gen_opts = nxc_codegen_llvm::Options {
        target: opts.target.clone(),
        debug_info: opts.debug_info,
        optimize: opts.release,
        root_dir: root,
        profile: opts.profile,
    };
    let out = nxc_codegen_llvm::generate(&prog, &sm, &bodies, &entry, &gen_opts);
    let mut c = finish(
        &sm,
        &diags,
        Some(out),
        test_names,
        color,
        diagnostic_root_dir.as_deref(),
        &opts.target,
    );
    c.expects_panic = expects_panic;
    c
}

fn finish(
    sm: &SourceMap,
    diags: &DiagSink,
    output: Option<Output>,
    tests: Vec<String>,
    color: bool,
    root_dir: Option<&Path>,
    target: &Target,
) -> Compilation {
    let mut list = diags.take_sorted();
    let errors = list.iter().filter(|d| d.level == Level::Error).count();
    // Warnings are often consequences of errors (code skipped by recovery): show them
    // once the errors are fixed.
    if errors > 0 {
        list.retain(|d| d.level == Level::Error);
    }
    let warnings = list.len() - errors;
    let json = diagnostics_json(&list, sm);
    let json_v1 = diagnostics_json_v1(&list, sm, root_dir, target);
    Compilation {
        diagnostics: render_all(&list, sm, color),
        errors,
        warnings,
        output: if errors == 0 { output } else { None },
        tests,
        expects_panic: Vec::new(),
        json,
        json_v1,
    }
}

/// A JSON string literal.
pub fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Diagnostics as JSON: one object per diagnostic, with 1-based positions of the primary
/// span (`end_col` is exclusive) and of every secondary label.
pub fn diagnostics_json(list: &[Diagnostic], sm: &SourceMap) -> String {
    let pos = |span: Span| -> (String, usize, usize, usize, usize) {
        match sm.try_get(span.file) {
            Some(f) if !span.is_dummy() => {
                let (l1, c1) = f.line_col(span.lo);
                let (l2, c2) = f.line_col(span.hi.max(span.lo));
                (f.name.clone(), l1, c1, l2, c2)
            }
            _ => (String::new(), 0, 0, 0, 0),
        }
    };
    let mut items = Vec::new();
    for d in list {
        let primary = d.primary_span().unwrap_or(Span::DUMMY);
        let (file, line, col, end_line, end_col) = pos(primary);
        let mut labels = Vec::new();
        for l in &d.labels {
            let (lf, ll, lc, lel, lec) = pos(l.span);
            labels.push(format!(
                "{{\"primary\":{},\"message\":{},\"file\":{},\"line\":{},\"col\":{},\"end_line\":{},\"end_col\":{}}}",
                l.primary,
                json_str(&l.message),
                json_str(&lf),
                ll,
                lc,
                lel,
                lec
            ));
        }
        let notes: Vec<String> = d
            .notes
            .iter()
            .map(|n| json_str(&format!("note: {}", n)))
            .chain(d.helps.iter().map(|h| json_str(&format!("help: {}", h))))
            .collect();
        items.push(format!(
            "{{\"level\":\"{}\",\"code\":{},\"message\":{},\"file\":{},\"line\":{},\"col\":{},\"end_line\":{},\"end_col\":{},\"labels\":[{}],\"notes\":[{}]}}",
            if d.level == Level::Error { "error" } else { "warning" },
            json_str(d.code),
            json_str(&d.message),
            json_str(&file),
            line,
            col,
            end_line,
            end_col,
            labels.join(","),
            notes.join(",")
        ));
    }
    format!("[{}]", items.join(","))
}

fn absolute_display(path: &Path, root_dir: Option<&Path>) -> String {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root_dir.unwrap_or_else(|| Path::new(".")).join(path)
    };
    let absolute = std::fs::canonicalize(&joined).unwrap_or_else(|_| {
        if joined.is_absolute() {
            joined
        } else {
            std::env::current_dir().unwrap_or_default().join(joined)
        }
    });
    let rendered = absolute.to_string_lossy();
    if let Some(unc) = rendered.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", unc)
    } else {
        rendered
            .strip_prefix(r"\\?\")
            .unwrap_or(&rendered)
            .to_string()
    }
}

fn diagnostic_path(name: &str, root_dir: Option<&Path>, target: &Target) -> String {
    if let Some(module) = name.strip_prefix("<std.").and_then(|s| s.strip_suffix('>')) {
        let module = if module == "sys" {
            match target.os {
                Os::Windows => "sys/windows",
                Os::Linux | Os::MacOs => "sys/linux",
                Os::Wasi => "sys/wasi",
                Os::Web => "sys/web",
                Os::None => "sys/bare",
            }
        } else {
            module
        };
        return format!("nexul-std:///{}.nx", module.replace('.', "/"));
    }
    absolute_display(Path::new(name), root_dir)
}

fn range_json(sm: &SourceMap, span: Span) -> String {
    match sm.try_get(span.file) {
        Some(file) if !span.is_dummy() => {
            let (line, col) = file.line_col(span.lo);
            let (end_line, end_col) = file.line_col(span.hi.max(span.lo));
            format!(
                "{{\"line\":{},\"col\":{},\"end_line\":{},\"end_col\":{}}}",
                line, col, end_line, end_col
            )
        }
        _ => "{\"line\":0,\"col\":0,\"end_line\":0,\"end_col\":0}".into(),
    }
}

/// Diagnostics in the stable, versioned v1 envelope.
pub fn diagnostics_json_v1(
    list: &[Diagnostic],
    sm: &SourceMap,
    root_dir: Option<&Path>,
    target: &Target,
) -> String {
    let items: Vec<String> = list.iter().map(|d| {
        let primary = d.primary_span().unwrap_or(Span::DUMMY);
        let path = sm.try_get(primary.file).filter(|_| !primary.is_dummy()).map(|f| diagnostic_path(&f.name, root_dir, target)).unwrap_or_default();
        let labels: Vec<String> = d.labels.iter().map(|label| {
            let label_path = sm.try_get(label.span.file).filter(|_| !label.span.is_dummy()).map(|f| diagnostic_path(&f.name, root_dir, target)).unwrap_or_default();
            format!("{{\"primary\":{},\"message\":{},\"path\":{},\"range\":{}}}", label.primary, json_str(&label.message), json_str(&label_path), range_json(sm, label.span))
        }).collect();
        let notes: Vec<String> = d.notes.iter().map(|n| format!("{{\"kind\":\"note\",\"message\":{}}}", json_str(n)))
            .chain(d.helps.iter().map(|h| format!("{{\"kind\":\"help\",\"message\":{}}}", json_str(h)))).collect();
        format!("{{\"level\":\"{}\",\"code\":{},\"message\":{},\"path\":{},\"range\":{},\"labels\":[{}],\"notes\":[{}]}}",
            if d.level == Level::Error { "error" } else { "warning" }, json_str(d.code), json_str(&d.message), json_str(&path), range_json(sm, primary), labels.join(","), notes.join(","))
    }).collect();
    format!(
        "{{\"schema\":\"nexul.diagnostics\",\"version\":1,\"compiler\":{},\"diagnostics\":[{}]}}",
        json_str(env!("CARGO_PKG_VERSION")),
        items.join(",")
    )
}

// ---------------------------------------------------------------------------
// Code generation and linking

pub struct BuildOptions {
    pub target: Target,
    pub release: bool,
    /// `0`-`3`, `s` or `z`; default `0` in debug and `2` with `--release`.
    pub opt_level: Option<char>,
    pub debug_info: bool,
    pub kind: OutputKind,
    pub link_args: Vec<String>,
    /// `--stack-report`: print the bound of the stack of the real-time tasks.
    pub stack_report: bool,
}

impl BuildOptions {
    pub fn new(target: Target, release: bool) -> BuildOptions {
        BuildOptions {
            target,
            release,
            opt_level: None,
            debug_info: !release,
            kind: OutputKind::Executable,
            link_args: Vec::new(),
            stack_report: false,
        }
    }

    fn level(&self) -> char {
        self.opt_level
            .unwrap_or(if self.release { '2' } else { '0' })
    }
}

/// The folder of the build cache: `NEXUL_CACHE_DIR`, else `%LOCALAPPDATA%\nexul\cache`
/// (Windows), `$XDG_CACHE_HOME/nexul` or `~/.cache/nexul`. `None` disables the cache
/// (also with `NEXUL_NO_CACHE`).
fn cache_dir() -> Option<PathBuf> {
    if std::env::var_os("NEXUL_NO_CACHE").is_some() {
        return None;
    }
    let dir = if let Some(d) = std::env::var_os("NEXUL_CACHE_DIR") {
        PathBuf::from(d)
    } else if let Some(d) = std::env::var_os("LOCALAPPDATA") {
        Path::new(&d).join("nexul").join("cache")
    } else if let Some(d) = std::env::var_os("XDG_CACHE_HOME") {
        Path::new(&d).join("nexul")
    } else {
        Path::new(&std::env::var_os("HOME")?)
            .join(".cache")
            .join("nexul")
    };
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// A 128-bit key for the build cache. The IR contains the whole program, including the
/// runtime, so equal keys mean equal outputs.
fn cache_key(parts: &[&str]) -> String {
    use std::hash::{DefaultHasher, Hasher};
    let mut out = String::new();
    for seed in [0x4e45_5855_4c00_0002u64, 0x9e37_79b9_7f4a_7c15] {
        let mut h = DefaultHasher::new();
        h.write_u64(seed);
        for p in parts {
            h.write_usize(p.len());
            h.write(p.as_bytes());
        }
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

/// Keeps the most recent cached outputs only.
fn trim_cache(dir: &Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("nx-"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if entries.len() <= keep {
        return;
    }
    entries.sort();
    for (_, p) in &entries[..entries.len() - keep] {
        let _ = std::fs::remove_file(p);
    }
}

fn unique_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}

/// Object file extension of a target.
fn obj_ext(t: &Target) -> &'static str {
    if t.os == Os::Windows {
        "obj"
    } else {
        "o"
    }
}

/// RISC-V 64 programs may live above 2 GiB (bare-metal boards): `medany` addressing.
fn code_model(t: &Target) -> i32 {
    if t.arch == nxc_sema::target::Arch::Riscv64 {
        5
    } else {
        0
    }
}

/// Compiles the generated module to an object file.
pub fn emit_object(output: &Output, obj: &Path, opts: &BuildOptions) -> Result<(), String> {
    let tc = Toolchain::find();
    let t = &opts.target;
    let cg = llvm::CodegenOptions {
        triple: &t.llvm_triple,
        cpu: &t.cpu,
        features: &t.features,
        opt_level: opts.level(),
        // A static library too: C compilers make position-independent executables by
        // default, which cannot link code that is not.
        pic: opts.kind != OutputKind::Executable && t.os != Os::Windows,
        code_model: code_model(t),
    };
    llvm::compile(&tc, &output.ir, obj, &cg, false).map(|_| ())
}

/// The module after LLVM's optimizations, as text (`--emit llvm-ir`).
pub fn optimized_ir(output: &Output, opts: &BuildOptions) -> Result<String, String> {
    let tc = Toolchain::find();
    let t = &opts.target;
    let cg = llvm::CodegenOptions {
        triple: &t.llvm_triple,
        cpu: &t.cpu,
        features: &t.features,
        opt_level: opts.level(),
        pic: false,
        code_model: code_model(t),
    };
    llvm::compile(&tc, &output.ir, Path::new(""), &cg, true).map(|s| s.unwrap_or_default())
}

/// Produces an executable or a library from the generated module.
///
/// Outputs are cached by content: building the same program again (same IR, options,
/// target and libraries) copies the cached file instead of running LLVM and the linker.
pub fn build(output: &Output, out: &Path, opts: &BuildOptions) -> Result<(), String> {
    let dir = out
        .parent()
        .map(|p| {
            if p.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                p.to_path_buf()
            }
        })
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create `{}`: {}", dir.display(), e))?;
    let ext = out
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let imports: Vec<String> = output
        .imports
        .iter()
        .map(|(a, b)| format!("{}:{}", a, b))
        .collect();
    let cached = cache_dir().map(|d| {
        let key = cache_key(&[
            env!("CARGO_PKG_VERSION"),
            &opts.target.name,
            &opts.target.llvm_triple,
            &opts.target.cpu,
            &opts.target.features,
            &opts.level().to_string(),
            if opts.debug_info { "g" } else { "" },
            &format!("{:?}", opts.kind),
            &output.links.join(","),
            &imports.join(","),
            &opts.link_args.join(" "),
            // With debug information, the executable names its `.pdb` file.
            &if opts.debug_info {
                out.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            } else {
                String::new()
            },
            &output.ir,
        ]);
        (d.clone(), d.join(format!("nx-{}{}", key, ext)))
    });
    if let Some((_, file)) = &cached {
        if file.is_file() && std::fs::copy(file, out).is_ok() {
            for ext in companions(opts) {
                let c = file.with_extension(ext);
                if c.is_file() {
                    let _ = std::fs::copy(&c, out.with_extension(ext));
                }
            }
            // Recently used outputs are the last ones removed from the cache.
            let _ = std::fs::File::options()
                .write(true)
                .open(file)
                .and_then(|f| f.set_modified(std::time::SystemTime::now()));
            return check_stack(output, out, opts);
        }
    }
    let work = std::env::temp_dir().join(format!("nexul-build-{}", unique_suffix()));
    std::fs::create_dir_all(&work)
        .map_err(|e| format!("cannot create `{}`: {}", work.display(), e))?;
    let result = (|| -> Result<(), String> {
        let obj = work.join(format!("program.{}", obj_ext(&opts.target)));
        emit_object(output, &obj, opts)?;
        let target_path = match &cached {
            Some(_) => work.join(
                out.file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("program")),
            ),
            None => out.to_path_buf(),
        };
        let tc = Toolchain::find();
        let job = link::LinkJob {
            target: &opts.target,
            objects: vec![obj],
            out: target_path.clone(),
            kind: opts.kind,
            imports: &output.imports,
            links: &output.links,
            exports: &output.exports,
            debug: opts.debug_info,
            optimize: opts.release,
            work_dir: work.clone(),
            extra: &opts.link_args,
        };
        link::link(&tc, &job)?;
        // An executable whose stack is not bounded is not kept, nor cached.
        if let Err(e) = check_stack(output, &target_path, opts) {
            let _ = std::fs::remove_file(&target_path);
            return Err(e);
        }
        if let Some((d, file)) = &cached {
            let _ = std::fs::copy(&target_path, file);
            for ext in companions(opts) {
                let c = target_path.with_extension(ext);
                if c.is_file() {
                    let _ = std::fs::copy(&c, file.with_extension(ext));
                    let _ = std::fs::copy(&c, out.with_extension(ext));
                }
            }
            std::fs::copy(&target_path, out)
                .map_err(|e| format!("cannot write `{}`: {}", out.display(), e))?;
            trim_cache(d, 200);
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// The bound of the stack of a program with real-time tasks (ADR 0018 §6), on the linked
/// executable: refused above the reserve (E0607); printed with `--stack-report`.
fn check_stack(output: &Output, exe: &Path, opts: &BuildOptions) -> Result<(), String> {
    let Some(spec) = &output.stack else { return Ok(()) };
    let report = stack::check(spec, exe)?;
    if opts.stack_report {
        eprintln!("stack of the real-time tasks ({} bytes):\n{}", report.bound, report.lines.join("\n"));
    }
    Ok(())
}

/// The files the linker writes next to the output, by extension: the debug information
/// (`.pdb`) and, for a DLL, its import library (`.lib`), which C programs link with.
fn companions(opts: &BuildOptions) -> &'static [&'static str] {
    if opts.target.os == Os::Windows && opts.kind == OutputKind::SharedLibrary {
        &["pdb", "lib"]
    } else {
        &["pdb"]
    }
}

pub fn exe_name(stem: &str, target: &Target) -> String {
    target.exe_name(stem)
}

/// File name of a library for a target.
pub fn lib_name(stem: &str, target: &Target, kind: OutputKind) -> String {
    match (target.os, kind) {
        (Os::Windows, OutputKind::SharedLibrary) => format!("{}.dll", stem),
        (Os::Windows, _) => format!("{}.lib", stem),
        (Os::MacOs, OutputKind::SharedLibrary) => format!("lib{}.dylib", stem),
        (Os::Wasi | Os::Web, _) => format!("{}.wasm", stem),
        (_, OutputKind::SharedLibrary) => format!("lib{}.so", stem),
        _ => format!("lib{}.a", stem),
    }
}

//! Module loading: from the root file, follow `import` declarations, parse every
//! reachable module once, drop the items excluded by `@cfg`, and register each module in
//! the program.

use crate::defs::{ImportEntry, ModId, ModuleInfo, Program};
use crate::target::{Cfg, Os, CFG_NAMES};
use nxc_syntax::ast::{self, ImportKind, ItemKind, Lit};
use nxc_syntax::{DiagSink, Diagnostic, SourceMap, Span};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

/// The standard library, embedded in the compiler (spec §09). The last field marks the
/// internal modules (runtime and system layer), not listed in diagnostics.
pub const STD_MODULES: &[(&str, &str, bool)] = &[
    ("std.core", include_str!("../../../lib/std/core.nx"), true),
    ("std.string", include_str!("../../../lib/std/string.nx"), false),
    ("std.vec", include_str!("../../../lib/std/vec.nx"), false),
    ("std.mem", include_str!("../../../lib/std/mem.nx"), false),
    ("std.rt", include_str!("../../../lib/std/rt.nx"), true),
    ("std.fmt", include_str!("../../../lib/std/fmt.nx"), false),
    ("std.builtins", include_str!("../../../lib/std/builtins.nx"), true),
    ("std.heap", include_str!("../../../lib/std/heap.nx"), true),
    ("std.io", include_str!("../../../lib/std/io.nx"), false),
    ("std.system", include_str!("../../../lib/std/system.nx"), false),
    ("std.time", include_str!("../../../lib/std/time.nx"), false),
    ("std.random", include_str!("../../../lib/std/random.nx"), false),
    ("std.math", include_str!("../../../lib/std/math.nx"), false),
    ("std.web", include_str!("../../../lib/std/web.nx"), true),
    ("std.wasm", include_str!("../../../lib/std/wasm.nx"), true),
    ("std.collections", include_str!("../../../lib/std/collections.nx"), false),
    ("std.tache", include_str!("../../../lib/std/tache.nx"), false),
    ("std.tzif", include_str!("../../../lib/std/tzif.nx"), true),
    ("std.fs", include_str!("../../../lib/std/fs.nx"), false),
    ("std.process", include_str!("../../../lib/std/process.nx"), false),
    ("std.net", include_str!("../../../lib/std/net.nx"), false),
    ("std.strview", include_str!("../../../lib/std/strview.nx"), false),
];

/// The system layer `std.sys`, one implementation per operating system.
const SYS_WINDOWS: &str = include_str!("../../../lib/std/sys/windows.nx");
const SYS_LINUX: &str = include_str!("../../../lib/std/sys/linux.nx");
const SYS_MACOS: &str = include_str!("../../../lib/std/sys/macos.nx");
const SYS_WASI: &str = include_str!("../../../lib/std/sys/wasi.nx");
const SYS_BARE: &str = include_str!("../../../lib/std/sys/bare.nx");

/// Source of a standard module for a given configuration.
pub fn std_source(path: &str, cfg: &Cfg) -> Option<&'static str> {
    if path == "std.sys" {
        return Some(match cfg.target.os {
            Os::Windows => SYS_WINDOWS,
            Os::Linux => SYS_LINUX,
            Os::MacOs => SYS_MACOS,
            Os::Wasi => SYS_WASI,
            Os::Web => include_str!("../../../lib/std/sys/web.nx"),
            Os::None => SYS_BARE,
        });
    }
    STD_MODULES.iter().find(|(p, _, _)| *p == path).map(|(_, t, _)| *t)
}

/// Names of the public standard modules (for diagnostics).
pub fn public_std_modules() -> Vec<&'static str> {
    STD_MODULES.iter().filter(|(_, _, internal)| !internal).map(|(p, _, _)| *p).collect()
}

/// Modules loaded in every program: they define the prelude and the runtime.
const ALWAYS_LOADED: &[&str] = &["std.core", "std.string", "std.vec", "std.mem", "std.rt"];

/// Where to find the root module and the modules it imports.
pub struct ProgramSource {
    /// Name shown in diagnostics for the root file.
    pub root_name: String,
    pub root_text: String,
    /// Directory containing the root file (imports are resolved under it).
    pub root_dir: Option<PathBuf>,
    /// Packages the program depends on (`nxpm`): `import name.module` is resolved under
    /// the package's source directory.
    pub deps: Vec<(String, PathBuf)>,
    /// Unsaved user files, keyed by their filesystem path. Standard-library sources are embedded and cannot be overridden.
    pub overlays: Vec<(PathBuf, String)>,
    /// Target and build mode, for `@cfg`.
    pub cfg: Cfg,
    /// The syntax of the root file (ADR 0010); the standard library stays in 0.2.
    pub edition: nxc_syntax::Edition,
    /// The syntax of the other files of the root's project that do not declare theirs
    /// (`// edition: X`): its `nexul.toml`, or `--edition`. `None`: that of the root file.
    pub project_edition: Option<nxc_syntax::Edition>,
    /// The syntax of each dependency package (its own `nexul.toml`), for its files that do
    /// not declare theirs; 0.2 when absent.
    pub dep_editions: Vec<(String, nxc_syntax::Edition)>,
}

impl ProgramSource {
    pub fn new(root_name: impl Into<String>, root_text: impl Into<String>, root_dir: Option<PathBuf>) -> ProgramSource {
        ProgramSource {
            root_name: root_name.into(),
            root_text: root_text.into(),
            root_dir,
            deps: Vec::new(),
            overlays: Vec::new(),
            cfg: Cfg::default(),
            edition: nxc_syntax::Edition::V0_2,
            project_edition: None,
            dep_editions: Vec::new(),
        }
    }

    /// The syntax of an imported file (not of the standard library): the one its first
    /// lines declare, else that of its package.
    fn edition_of(&self, segs: &[String], text: &str) -> nxc_syntax::Edition {
        if let Some(e) = nxc_syntax::header_edition(text) {
            return e;
        }
        match self.deps.iter().find(|(name, _)| Some(name) == segs.first()) {
            Some((name, _)) => self.dep_editions.iter().find(|(n, _)| n == name).map(|(_, e)| *e).unwrap_or_default(),
            None => self.project_edition.unwrap_or(self.edition),
        }
    }
}

fn normalized_path(path: &std::path::Path) -> PathBuf {
    let absolute = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(path) };
    if let Ok(path) = std::fs::canonicalize(&absolute) {
        return path;
    }
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        use std::path::Component;
        match component {
            Component::CurDir => {}
            Component::ParentDir => { normalized.pop(); }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn paths_match(a: &std::path::Path, b: &std::path::Path) -> bool {
    #[cfg(windows)]
    { normalized_path(a).to_string_lossy().eq_ignore_ascii_case(&normalized_path(b).to_string_lossy()) }
    #[cfg(not(windows))]
    { normalized_path(a) == normalized_path(b) }
}

fn read_module_file(dir: &PathBuf, path: &[String], overlays: &[(PathBuf, String)]) -> Option<(PathBuf, String)> {
    let mut p = dir.clone();
    for (i, seg) in path.iter().enumerate() {
        if i + 1 == path.len() {
            p.push(format!("{}.nx", seg));
        } else {
            p.push(seg);
        }
    }
    let overlay = overlays.iter().find(|(candidate, _)| paths_match(candidate, &p)).map(|(_, text)| text.clone());
    let text = match overlay {
        Some(text) => text,
        None => String::from_utf8(std::fs::read(&p).ok()?).ok()?,
    };
    let text = text.strip_prefix('\u{feff}').map(|s| s.to_string()).unwrap_or(text);
    Some((p, text))
}

fn read_user_module(src: &ProgramSource, path: &[String]) -> Option<(String, String)> {
    // A dependency package: `import pkg.module` reads `<pkg dir>/module.nx`
    // (`import pkg` alone reads `<pkg dir>/lib.nx`).
    if let Some((_, dir)) = src.deps.iter().find(|(name, _)| Some(name) == path.first()) {
        let rest: Vec<String> = if path.len() == 1 { vec!["lib".to_string()] } else { path[1..].to_vec() };
        let (p, t) = read_module_file(dir, &rest, &src.overlays)?;
        return Some((p.to_string_lossy().replace('\\', "/"), t));
    }
    let dir = src.root_dir.as_ref()?;
    let (_, text) = read_module_file(dir, path, &src.overlays)?;
    Some((format!("{}.nx", path.join("/")), text))
}

/// Evaluates the `@cfg` attributes of an item: `true` if the item is kept.
fn item_enabled(attrs: &[ast::Attr], cfg: &Cfg, diags: &DiagSink) -> bool {
    let mut keep = true;
    for a in attrs.iter().filter(|a| a.name.name == "cfg") {
        if a.args.is_empty() {
            diags.emit(Diagnostic::error("E0234", "`@cfg` needs a condition").primary(a.span, "for example `@cfg(\"windows\")`"));
        }
        for (lit, span) in &a.args {
            let Lit::Str(pred) = lit else {
                diags.emit(Diagnostic::error("E0234", "a `@cfg` condition is a string").primary(*span, "expected a string such as \"linux\""));
                continue;
            };
            match cfg.eval(pred) {
                Ok(v) => keep &= v,
                Err(name) => {
                    let mut d = Diagnostic::error("E0234", format!("unknown `@cfg` condition `{}`", name)).primary(*span, "unknown condition");
                    if let Some(s) = crate::util::best_match(&name, CFG_NAMES.iter().copied()) {
                        d = d.help(format!("did you mean `{}`?", s));
                    } else {
                        d = d.note(format!("known conditions: {}", CFG_NAMES.join(", ")));
                    }
                    diags.emit(d);
                }
            }
        }
    }
    keep
}

/// Removes the items (and `impl` members) whose `@cfg` is false.
fn apply_cfg(module: &mut ast::Module, cfg: &Cfg, diags: &DiagSink) {
    module.items.retain(|it| item_enabled(&it.attrs, cfg, diags));
    for it in module.items.iter_mut() {
        if let ItemKind::Impl(imp) = &mut it.kind {
            imp.items.retain(|m| item_enabled(&m.attrs, cfg, diags));
        }
    }
}

pub fn load(src: ProgramSource, sm: &mut SourceMap, diags: &DiagSink) -> Program {
    let mut prog = Program::new();
    prog.cfg = src.cfg.clone();
    let mut by_path: HashMap<String, ModId> = HashMap::new();
    let mut queue: VecDeque<ModId> = VecDeque::new();

    // The root module.
    let root_file = sm.add_with_edition(src.root_name.clone(), src.root_text.clone(), src.edition);
    let mut root_ast = nxc_syntax::parse_file(sm, root_file, diags);
    apply_cfg(&mut root_ast, &src.cfg, diags);
    let root_newtypes = crate::newtype::expand(&mut root_ast, sm, &src.root_name, diags);
    crate::derive::expand(&mut root_ast, sm, &src.root_name, diags);
    let stem = std::path::Path::new(&src.root_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "main".into());
    // The root module is named by its file (spec §6.1); a `module` declaration documents
    // that name and cannot change it (E0220, as for the other modules).
    if let Some((decl, dspan)) = &root_ast.decl {
        let declared = decl.iter().map(|i| i.name.clone()).collect::<Vec<_>>().join(".");
        if declared != stem {
            diags.emit(
                Diagnostic::error("E0220", format!("module declaration `{}` does not match the file path", declared))
                    .primary(*dspan, format!("this file is module `{}`", stem))
                    .help(format!("write `module {}` or remove the declaration", stem)),
            );
        }
    }
    let root_path = stem;
    let root = ModId(0);
    prog.modules.push(ModuleInfo {
        path: root_path.clone(),
        file: root_file,
        edition: sm.get(root_file).edition,
        newtypes: root_newtypes,
        is_std: false,
        items: HashMap::new(),
        imports: HashMap::new(),
        ast: Some(root_ast),
    });
    by_path.insert(root_path, root);
    prog.root = root;
    queue.push_back(root);

    let load_module = |prog: &mut Program, by_path: &mut HashMap<String, ModId>, queue: &mut VecDeque<ModId>, sm: &mut SourceMap, segs: &[String], span: Option<Span>| -> Option<ModId> {
        let path = segs.join(".");
        if let Some(&id) = by_path.get(&path) {
            return Some(id);
        }
        let (name, text, is_std) = if segs.first().map(|s| s.as_str()) == Some("std") {
            match std_source(&path, &src.cfg) {
                Some(text) => (format!("<{}>", path), text.to_string(), true),
                None => {
                    if let Some(span) = span {
                        let mut d = Diagnostic::error("E0221", format!("module `{}` not found", path)).primary(span, "no such module in the standard library");
                        d = d.note(format!("available: {}", public_std_modules().join(", ")));
                        diags.emit(d);
                    }
                    return None;
                }
            }
        } else {
            match read_user_module(&src, segs) {
                Some((n, t)) => (n, t, false),
                None => {
                    if let Some(span) = span {
                        diags.emit(
                            Diagnostic::error("E0221", format!("module `{}` not found", path))
                                .primary(span, "cannot find this module")
                                .help(format!("expected a file `{}.nx` next to the main file", segs.join("/"))),
                        );
                    }
                    return None;
                }
            }
        };
        let file = if is_std {
            // A module of the standard library says its edition in its header (0.2 without).
            let edition = nxc_syntax::header_edition(&text).unwrap_or_default();
            sm.add_with_edition(name.clone(), text, edition)
        } else {
            let edition = src.edition_of(segs, &text);
            sm.add_with_edition(name.clone(), text, edition)
        };
        let mut ast = nxc_syntax::parse_file(sm, file, diags);
        apply_cfg(&mut ast, &src.cfg, diags);
        let newtypes = crate::newtype::expand(&mut ast, sm, &name, diags);
        crate::derive::expand(&mut ast, sm, &name, diags);
        if let Some((decl, dspan)) = &ast.decl {
            let declared = decl.iter().map(|i| i.name.clone()).collect::<Vec<_>>().join(".");
            let dep_relative = src.deps.iter().any(|(n, _)| Some(n) == segs.first()) && declared == segs[1..].join(".");
            if declared != path && !dep_relative {
                diags.emit(
                    Diagnostic::error("E0220", format!("module declaration `{}` does not match the file path", declared))
                        .primary(*dspan, format!("this file is module `{}`", path))
                        .help(format!("write `module {}` or remove the declaration", path)),
                );
            }
        }
        let id = ModId(prog.modules.len() as u32);
        prog.modules.push(ModuleInfo { path: path.clone(), file, edition: sm.get(file).edition, newtypes, is_std, items: HashMap::new(), imports: HashMap::new(), ast: Some(ast) });
        by_path.insert(path, id);
        queue.push_back(id);
        Some(id)
    };

    let mut preloaded = ALWAYS_LOADED.to_vec();
    if src.cfg.target.os == Os::Web {
        preloaded.push("std.web");
    }
    for p in preloaded {
        let segs: Vec<String> = p.split('.').map(|s| s.to_string()).collect();
        load_module(&mut prog, &mut by_path, &mut queue, sm, &segs, None);
    }
    // Software floating point for processors without a double-precision unit.
    if !src.cfg.target.hard_float64 {
        load_module(&mut prog, &mut by_path, &mut queue, sm, &["std".to_string(), "builtins".to_string()], None);
    }

    // Breadth-first traversal of imports. Import resolution to items happens later,
    // once every module's items are collected; here we only record module targets.
    let mut pending_imports: Vec<(ModId, usize, Option<ModId>)> = Vec::new();
    while let Some(m) = queue.pop_front() {
        let imports = prog.modules[m.idx()].ast.as_ref().map(|a| a.imports.clone()).unwrap_or_default();
        for (i, imp) in imports.iter().enumerate() {
            let segs: Vec<String> = imp.path.iter().map(|s| s.name.clone()).collect();
            let target = load_module(&mut prog, &mut by_path, &mut queue, sm, &segs, Some(imp.span));
            pending_imports.push((m, i, target));
        }
    }

    // Record module imports (`import a.b` / `import a.b as c`). Item imports are bound
    // after collection (see `collect::bind_item_imports`).
    for (m, i, target) in pending_imports {
        let Some(target) = target else { continue };
        let imp = prog.modules[m.idx()].ast.as_ref().unwrap().imports[i].clone();
        if let ImportKind::Module { alias } = &imp.kind {
            let name = alias.as_ref().map(|a| a.name.clone()).unwrap_or_else(|| imp.path.last().unwrap().name.clone());
            let span = alias.as_ref().map(|a| a.span).unwrap_or(imp.path.last().unwrap().span);
            let module = &mut prog.modules[m.idx()];
            if module.imports.contains_key(&name) {
                diags.emit(Diagnostic::error("E0223", format!("the name `{}` is imported twice", name)).primary(span, "second import"));
                continue;
            }
            module.imports.insert(name, ImportEntry { item: crate::defs::ItemRef::Module(target), span, used: std::cell::Cell::new(false) });
        }
    }
    prog
}

/// Finds a loaded module by dotted path.
pub fn module_by_path(prog: &Program, path: &str) -> Option<ModId> {
    prog.modules.iter().position(|m| m.path == path).map(|i| ModId(i as u32))
}

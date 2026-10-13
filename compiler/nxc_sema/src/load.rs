//! Module loading: from the root file, follow `import` declarations, parse every
//! reachable module once, and register it in the program.

use crate::defs::{ImportEntry, ModId, ModuleInfo, Program};
use nxc_syntax::ast::ImportKind;
use nxc_syntax::{DiagSink, Diagnostic, SourceMap, Span};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

/// The standard library, embedded in the compiler (spec §09).
pub const STD_MODULES: &[(&str, &str)] = &[
    ("std.core", include_str!("../../../lib/std/core.nx")),
    ("std.string", include_str!("../../../lib/std/string.nx")),
    ("std.vec", include_str!("../../../lib/std/vec.nx")),
    ("std.mem", include_str!("../../../lib/std/mem.nx")),
    ("std.io", include_str!("../../../lib/std/io.nx")),
    ("std.system", include_str!("../../../lib/std/system.nx")),
    ("std.time", include_str!("../../../lib/std/time.nx")),
    ("std.math", include_str!("../../../lib/std/math.nx")),
];

/// Modules loaded in every program: they define the prelude.
const ALWAYS_LOADED: &[&str] = &["std.core", "std.string", "std.vec", "std.mem"];

/// Where to find the root module and the modules it imports.
pub struct ProgramSource {
    /// Name shown in diagnostics for the root file.
    pub root_name: String,
    pub root_text: String,
    /// Directory containing the root file (imports are resolved under it).
    pub root_dir: Option<PathBuf>,
}

fn read_user_module(root_dir: &Option<PathBuf>, path: &[String]) -> Option<(String, String)> {
    let dir = root_dir.as_ref()?;
    let mut p = dir.clone();
    for (i, seg) in path.iter().enumerate() {
        if i + 1 == path.len() {
            p.push(format!("{}.nx", seg));
        } else {
            p.push(seg);
        }
    }
    let bytes = std::fs::read(&p).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    let text = text.strip_prefix('\u{feff}').map(|s| s.to_string()).unwrap_or(text);
    Some((format!("{}.nx", path.join("/")), text))
}

pub fn load(src: ProgramSource, sm: &mut SourceMap, diags: &DiagSink) -> Program {
    let mut prog = Program::new();
    let mut by_path: HashMap<String, ModId> = HashMap::new();
    let mut queue: VecDeque<ModId> = VecDeque::new();

    // The root module.
    let root_file = sm.add(src.root_name.clone(), src.root_text);
    let root_ast = nxc_syntax::parse_file(sm, root_file, diags);
    let stem = std::path::Path::new(&src.root_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "main".into());
    let root_path = match &root_ast.decl {
        Some((p, _)) => p.iter().map(|i| i.name.clone()).collect::<Vec<_>>().join("."),
        None => stem,
    };
    let root = ModId(0);
    prog.modules.push(ModuleInfo {
        path: root_path.clone(),
        file: root_file,
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
            match STD_MODULES.iter().find(|(p, _)| *p == path) {
                Some((_, text)) => (format!("<{}>", path), text.to_string(), true),
                None => {
                    if let Some(span) = span {
                        let mut d = Diagnostic::error("E0221", format!("module `{}` not found", path)).primary(span, "no such module in the standard library");
                        let names: Vec<&str> = STD_MODULES.iter().map(|(p, _)| *p).filter(|p| *p != "std.core").collect();
                        d = d.note(format!("available: {}", names.join(", ")));
                        diags.emit(d);
                    }
                    return None;
                }
            }
        } else {
            match read_user_module(&src.root_dir, segs) {
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
        let file = sm.add(name, text);
        let ast = nxc_syntax::parse_file(sm, file, diags);
        if let Some((decl, dspan)) = &ast.decl {
            let declared = decl.iter().map(|i| i.name.clone()).collect::<Vec<_>>().join(".");
            if declared != path {
                diags.emit(
                    Diagnostic::error("E0220", format!("module declaration `{}` does not match the file path", declared))
                        .primary(*dspan, format!("this file is module `{}`", path))
                        .help(format!("write `module {}` or remove the declaration", path)),
                );
            }
        }
        let id = ModId(prog.modules.len() as u32);
        prog.modules.push(ModuleInfo { path: path.clone(), file, is_std, items: HashMap::new(), imports: HashMap::new(), ast: Some(ast) });
        by_path.insert(path, id);
        queue.push_back(id);
        Some(id)
    };

    for p in ALWAYS_LOADED {
        let segs: Vec<String> = p.split('.').map(|s| s.to_string()).collect();
        load_module(&mut prog, &mut by_path, &mut queue, sm, &segs, None);
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

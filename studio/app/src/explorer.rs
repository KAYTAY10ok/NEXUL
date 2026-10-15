//! The tree of the open folder for the explorer.

use gpui_kit::component::tree::TreeItem;
use nexul_studio::watch;
use std::collections::HashSet;
use std::path::Path;

/// More entries than this are not shown (a folder such as a whole disk).
const MAX_ENTRIES: usize = 20_000;

/// The id of the row where the name of a new file or folder is typed.
pub const NEW_ENTRY: &str = "\u{1}nouveau";

/// The tree of `root`: folders first, then files, by name without case. Returns the
/// items and the relative paths of the folders (an empty folder has no children, so the
/// tree alone cannot tell it from a file). `new_in`: the folder (`""` for the root) that
/// shows the [`NEW_ENTRY`] row first.
pub fn build(root: &Path, expanded: &HashSet<String>, new_in: Option<&str>) -> (Vec<TreeItem>, HashSet<String>) {
    let mut dirs = HashSet::new();
    let mut count = 0;
    let items = walk(root, "", 0, expanded, new_in, &mut dirs, &mut count);
    (items, dirs)
}

fn walk(dir: &Path, prefix: &str, depth: usize, expanded: &HashSet<String>, new_in: Option<&str>, dirs: &mut HashSet<String>, count: &mut usize) -> Vec<TreeItem> {
    let mut items = Vec::new();
    if new_in == Some(prefix) {
        items.push(TreeItem::new(NEW_ENTRY, ""));
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return items };
    let mut entries: Vec<(bool, String)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if watch::IGNORED.contains(&name.as_str()) || watch::is_hidden_name(&name) {
                return None;
            }
            Some((e.file_type().ok()?.is_dir(), name))
        })
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())));
    for (is_dir, name) in entries {
        *count += 1;
        if *count > MAX_ENTRIES {
            break;
        }
        let rel = if prefix.is_empty() { name.clone() } else { format!("{}/{}", prefix, name) };
        if is_dir {
            dirs.insert(rel.clone());
            let children = if depth < 16 { walk(&dir.join(&name), &rel, depth + 1, expanded, new_in, dirs, count) } else { Vec::new() };
            let open = expanded.contains(&rel);
            items.push(TreeItem::new(rel, name).children(children).expanded(open));
        } else {
            items.push(TreeItem::new(rel, name));
        }
    }
    items
}

/// The language of the editor for a file name.
pub fn language_of(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "nx" => "nexul",
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" => "typescript",
        "tsx" => "tsx",
        "json" => "json",
        "html" | "htm" => "html",
        "css" => "css",
        "py" => "python",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => "cpp",
        "cs" => "csharp",
        "sql" => "sql",
        "md" | "markdown" => "markdown",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        _ => "text",
    }
}

/// The name shown in the status bar.
pub fn language_name(lang: &str) -> &'static str {
    match lang {
        "nexul" => "NEXUL",
        "rust" => "Rust",
        "javascript" => "JavaScript",
        "typescript" | "tsx" => "TypeScript",
        "json" => "JSON",
        "html" => "HTML",
        "css" => "CSS",
        "python" => "Python",
        "c" => "C",
        "cpp" => "C++",
        "csharp" => "C#",
        "sql" => "SQL",
        "markdown" => "Markdown",
        "toml" => "TOML",
        "yaml" => "YAML",
        _ => "Texte",
    }
}

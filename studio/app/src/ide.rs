//! NEXUL language services in the editor, answered by the compiler: `nxc ide` (hover,
//! definition, completion, references, rename, format) and `nxc check --json`
//! (diagnostics). The commands run outside of the interface thread.
//!
//! Lines and columns of `nxc` count from 1, in characters, the end being excluded; those
//! of the editor count from 0, in characters.

use gpui_kit::component::input::{CompletionProvider, DefinitionProvider, HoverProvider, Position, Rope, RopeExt as _};
use gpui_kit::{App, AppContext as _, Result, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, Documentation, Hover, HoverContents, LocationLink, MarkupContent, MarkupKind, Range,
};
use serde_json::Value;
use std::cell::RefCell;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::rc::Rc;
use std::str::FromStr as _;

/// The file shown by an editor; it changes when the file is renamed or moved.
pub type FileRef = Rc<RefCell<PathBuf>>;

/// Runs `nxc args…` in `cwd` with `input` on its standard input; its standard output.
fn run(nxc: &Path, args: &[String], cwd: &Path, input: Option<String>) -> Option<String> {
    let mut cmd = nexul_studio::runner::command(nxc, args, cwd);
    cmd.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = cmd.spawn().ok()?;
    if let (Some(mut stdin), Some(text)) = (child.stdin.take(), input) {
        // Written by another thread: a large text would fill the pipe while nxc writes.
        std::thread::spawn(move || {
            let _ = stdin.write_all(text.as_bytes());
        });
    }
    let out = child.wait_with_output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

/// `nxc ide <request> <file> <line> <col> [extra] --stdin`, the text of the editor on
/// its standard input; the JSON answer (`None` for `null` or a failure).
pub fn ide(nxc: &Path, request: &str, file: &Path, at: Option<(u32, u32)>, extra: Option<&str>, text: String) -> Option<Value> {
    let mut args = vec!["ide".to_string(), request.to_string(), file.to_string_lossy().to_string()];
    if let Some((line, col)) = at {
        args.push(line.to_string());
        args.push(col.to_string());
    }
    if let Some(x) = extra {
        args.push(x.to_string());
    }
    args.push("--stdin".into());
    let cwd = file.parent().unwrap_or(Path::new("."));
    let out = run(nxc, &args, cwd, Some(text))?;
    serde_json::from_str::<Value>(out.trim()).ok().filter(|v| !v.is_null())
}

/// `nxc ide format`: the formatted text, or `None` when nxc cannot format it.
pub fn format(nxc: &Path, file: &Path, text: String) -> Option<String> {
    let args = ["ide".to_string(), "format".into(), file.to_string_lossy().to_string(), "--stdin".into()];
    let out = run(nxc, &args, file.parent().unwrap_or(Path::new(".")), Some(text))?;
    (!out.trim().is_empty() && !out.trim_start().starts_with("{\"error\"")).then_some(out)
}

/// The position (1-based, for nxc) of a byte offset of the text.
fn nxc_position(text: &Rope, offset: usize) -> (u32, u32) {
    let p = text.offset_to_position(offset);
    (p.line + 1, p.character + 1)
}

fn num(v: &Value, key: &str) -> u32 {
    v.get(key).and_then(Value::as_u64).unwrap_or(1).max(1) as u32
}

/// The range (0-based) of `{line, col, end_line, end_col}` (1-based).
pub fn range_of(v: &Value) -> Range {
    let start = Position::new(num(v, "line") - 1, num(v, "col") - 1);
    let end = Position::new(v.get("end_line").map(|_| num(v, "end_line") - 1).unwrap_or(start.line), v.get("end_col").map(|_| num(v, "end_col") - 1).unwrap_or(start.character));
    Range::new(start, end)
}

/// A location of nxc (`file` relative to the folder of `base`, empty for `base` itself).
pub fn location_path(v: &Value, base: &Path) -> Option<PathBuf> {
    if v.get("std").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    match v.get("file").and_then(Value::as_str) {
        None | Some("") => Some(base.to_path_buf()),
        Some(f) => Some(base.parent().unwrap_or(Path::new(".")).join(f)),
    }
}

// ------------------------------------------------------------------ file URIs

/// `file:///C:/dossier/fichier%20nom.nx`.
pub fn file_uri(p: &Path) -> lsp_types::Uri {
    let s = p.to_string_lossy().replace('\\', "/");
    let mut out = String::from(if s.starts_with('/') { "file://" } else { "file:///" });
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~:".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    lsp_types::Uri::from_str(&out).unwrap_or_else(|_| lsp_types::Uri::from_str("file:///").unwrap())
}

/// The path of a `file:` URI made by [`file_uri`].
pub fn uri_path(uri: &lsp_types::Uri) -> Option<PathBuf> {
    let s = uri.as_str().strip_prefix("file://")?;
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let path = String::from_utf8(out).ok()?;
    // `/C:/x` → `C:/x` on Windows.
    let path = if cfg!(windows) && path.len() > 2 && path.as_bytes()[0] == b'/' && path.as_bytes()[2] == b':' { path[1..].to_string() } else { path };
    Some(PathBuf::from(if cfg!(windows) { path.replace('/', "\\") } else { path }))
}

// ------------------------------------------------------------------ providers

/// The services of nxc for one editor.
pub struct Nexul {
    pub nxc: PathBuf,
    pub file: FileRef,
}

impl Nexul {
    fn request(&self, request: &'static str, text: &Rope, offset: usize, cx: &mut App) -> Task<Option<Value>> {
        let (nxc, file) = (self.nxc.clone(), self.file.borrow().clone());
        let at = nxc_position(text, offset);
        let text = text.to_string();
        cx.background_spawn(async move { ide(&nxc, request, &file, Some(at), None, text) })
    }
}

impl HoverProvider for Nexul {
    fn hover(&self, text: &Rope, offset: usize, _: &mut Window, cx: &mut App) -> Task<Result<Option<Hover>>> {
        let task = self.request("hover", text, offset, cx);
        cx.background_spawn(async move {
            let Some(v) = task.await else { return Ok(None) };
            let code = v.get("code").and_then(Value::as_str).unwrap_or("");
            let doc = v.get("doc").and_then(Value::as_str).unwrap_or("");
            if code.is_empty() && doc.is_empty() {
                return Ok(None);
            }
            let mut value = String::new();
            if !code.is_empty() {
                value.push_str(&format!("```nexul\n{}\n```", code));
            }
            if !doc.is_empty() {
                if !value.is_empty() {
                    value.push_str("\n\n");
                }
                value.push_str(doc);
            }
            Ok(Some(Hover { contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value }), range: None }))
        })
    }
}

impl DefinitionProvider for Nexul {
    fn definitions(&self, text: &Rope, offset: usize, _: &mut Window, cx: &mut App) -> Task<Result<Vec<LocationLink>>> {
        let task = self.request("definition", text, offset, cx);
        let base = self.file.borrow().clone();
        cx.background_spawn(async move {
            let Some(v) = task.await else { return Ok(Vec::new()) };
            let Some(path) = location_path(&v, &base) else { return Ok(Vec::new()) };
            let range = range_of(&v);
            Ok(vec![LocationLink { origin_selection_range: None, target_uri: file_uri(&path), target_range: range, target_selection_range: range }])
        })
    }
}

fn completion_kind(kind: &str) -> CompletionItemKind {
    match kind {
        "function" => CompletionItemKind::FUNCTION,
        "method" => CompletionItemKind::METHOD,
        "field" => CompletionItemKind::FIELD,
        "variable" | "parameter" => CompletionItemKind::VARIABLE,
        "struct" => CompletionItemKind::STRUCT,
        "enum" => CompletionItemKind::ENUM,
        "variant" => CompletionItemKind::ENUM_MEMBER,
        "trait" => CompletionItemKind::INTERFACE,
        "module" => CompletionItemKind::MODULE,
        "keyword" => CompletionItemKind::KEYWORD,
        "constant" | "const" => CompletionItemKind::CONSTANT,
        "type" => CompletionItemKind::CLASS,
        _ => CompletionItemKind::TEXT,
    }
}

/// Is the character before `offset` in a comment or a string (where there is nothing to
/// complete)? The text is read up to the end of that line: a string ends there, and its
/// interpolations `{…}` are code. The comments and strings are the same in both editions.
pub fn in_comment_or_string(text: &str, offset: usize) -> bool {
    use nexul_studio_syntax::Kind;
    let Some(last) = offset.checked_sub(1) else { return false };
    let end = text[offset..].find('\n').map_or(text.len(), |i| offset + i);
    nexul_studio_syntax::tokens("nexul", &text[..end])
        .iter()
        .rev()
        .find(|t| t.range.start <= last && last < t.range.end)
        .is_some_and(|t| matches!(t.kind, Kind::Comment | Kind::DocComment | Kind::String | Kind::Escape))
}

impl CompletionProvider for Nexul {
    fn completions(&self, text: &Rope, offset: usize, _: CompletionContext, _: &mut Window, cx: &mut App) -> Task<Result<CompletionResponse>> {
        if in_comment_or_string(&text.to_string(), offset) {
            return Task::ready(Ok(CompletionResponse::Array(Vec::new())));
        }
        let task = self.request("completion", text, offset, cx);
        cx.background_spawn(async move {
            let items = task.await.and_then(|v| v.as_array().cloned()).unwrap_or_default();
            let items = items
                .iter()
                .filter_map(|it| {
                    let label = it.get("label")?.as_str()?.to_string();
                    let detail = it.get("detail").and_then(Value::as_str).filter(|d| !d.is_empty()).map(str::to_string);
                    let doc = it.get("doc").and_then(Value::as_str).filter(|d| !d.is_empty());
                    Some(CompletionItem {
                        label,
                        kind: Some(completion_kind(it.get("kind").and_then(Value::as_str).unwrap_or(""))),
                        detail,
                        documentation: doc.map(|d| Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value: d.to_string() })),
                        ..Default::default()
                    })
                })
                .collect();
            Ok(CompletionResponse::Array(items))
        })
    }

    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _: &mut App) -> bool {
        new_text == "." || new_text == ":" || new_text.chars().all(|c| c.is_alphanumeric() || c == '_') && !new_text.is_empty()
    }
}

// ------------------------------------------------------------------ diagnostics

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A diagnostic of `nxc check`.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    /// The file concerned.
    pub path: PathBuf,
    pub range: Range,
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

/// `nxc check <file> --json`: the diagnostics of the file and of the files it imports.
/// `Err` when nxc could not check it (message for the user).
pub fn check(nxc: &Path, file: &Path) -> std::result::Result<Vec<Problem>, String> {
    let args = ["check".to_string(), file.to_string_lossy().to_string(), "--json".into()];
    let dir = file.parent().unwrap_or(Path::new("."));
    let out = run(nxc, &args, dir, None).ok_or_else(|| "Impossible de lancer nxc.".to_string())?;
    let v: Value = serde_json::from_str(out.trim()).map_err(|_| format!("Réponse inattendue de nxc check : {}", out.trim().chars().take(200).collect::<String>()))?;
    let list = v.as_array().cloned().unwrap_or_default();
    Ok(list.iter().map(|d| problem(d, dir)).collect())
}

fn problem(d: &Value, dir: &Path) -> Problem {
    let s = |k: &str| d.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let mut message = s("message");
    // The explanation of the main label, then the notes.
    if let Some(label) = d.get("labels").and_then(Value::as_array).and_then(|l| l.iter().find(|l| l.get("primary").and_then(Value::as_bool) == Some(true))) {
        let m = label.get("message").and_then(Value::as_str).unwrap_or("");
        if !m.is_empty() && m != message {
            message = format!("{} : {}", message, m);
        }
    }
    for note in d.get("notes").and_then(Value::as_array).into_iter().flatten() {
        if let Some(n) = note.as_str().or_else(|| note.get("message").and_then(Value::as_str)) {
            message.push_str(&format!("\n{}", n));
        }
    }
    let file = s("file");
    Problem {
        path: if file.is_empty() { dir.to_path_buf() } else { dir.join(file) },
        range: range_of(d),
        severity: match s("level").as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            _ => Severity::Info,
        },
        code: s("code"),
        message,
    }
}

/// A NEXUL editor: edition 0.2 (`nexul`) or edition 1 of the syntax (`nexul1`, ADR 0010).
pub fn is_nexul(lang: &str) -> bool {
    lang == "nexul" || lang == "nexul1"
}

/// The edition the first lines of a text declare (`// edition: 1` or `// edition: 0.2`).
pub fn declared_edition(text: &str) -> Option<bool> {
    text.lines().take(3).find_map(|l| match l.trim().strip_prefix("// edition:").map(str::trim) {
        Some("1") | Some("1.0") => Some(true),
        Some("0.2") => Some(false),
        _ => None,
    })
}

/// Is `file` written in edition 1? As `nxc` decides: its first lines, else the
/// `edition` of the `nexul.toml` of its project.
pub fn edition_1(file: &Path, text: &str) -> bool {
    if let Some(e) = declared_edition(text) {
        return e;
    }
    let mut dir = file.parent();
    while let Some(d) = dir {
        if let Ok(manifest) = std::fs::read_to_string(d.join("nexul.toml")) {
            return manifest.lines().any(|l| {
                let l = l.split("#").next().unwrap_or("").trim();
                l.strip_prefix("edition").map(|r| r.trim_start()).and_then(|r| r.strip_prefix('=')).is_some_and(|v| matches!(v.trim(), "\"1\"" | "\"1.0\""))
            });
        }
        dir = d.parent();
    }
    false
}

/// Paths compared as Windows does (case-insensitive there).
pub fn same_file(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        let s = p.to_string_lossy().replace('/', "\\");
        if cfg!(windows) {
            s.to_lowercase()
        } else {
            s
        }
    };
    norm(a) == norm(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use core::prelude::v1::test;

    #[test]
    fn uris() {
        let p = if cfg!(windows) { PathBuf::from(r"C:\Mes projets\été\a.nx") } else { PathBuf::from("/home/x/été/a.nx") };
        let u = file_uri(&p);
        assert!(u.as_str().starts_with("file:///"), "{}", u.as_str());
        assert!(!u.as_str().contains(' '));
        assert_eq!(uri_path(&u), Some(p));
    }

    #[test]
    fn diagnostics_of_nxc() {
        let d: Value = serde_json::from_str(
            r#"{"level":"error","code":"E0302","message":"mismatched types","file":"a.nx","line":8,"col":21,"end_line":8,"end_col":22,"labels":[{"primary":true,"message":"expected `String`, found `{integer}`","file":"a.nx","line":8,"col":21,"end_line":8,"end_col":22}],"notes":[]}"#,
        )
        .unwrap();
        let p = problem(&d, Path::new("dossier"));
        assert_eq!(p.path, Path::new("dossier").join("a.nx"));
        assert_eq!(p.range, Range::new(Position::new(7, 20), Position::new(7, 21)));
        assert_eq!(p.severity, Severity::Error);
        assert_eq!(p.message, "mismatched types : expected `String`, found `{integer}`");
    }

    #[test]
    fn no_completion_in_comments_and_strings() {
        // `|` marks the cursor.
        let at = |s: &str| {
            let offset = s.find('|').unwrap();
            in_comment_or_string(&s.replace('|', ""), offset)
        };
        for s in ["fn b() {} // modif b|", "/// doc d|\nfn f() {}", "/* a\n b| */", r#"let s = "ab|""#, r#"let s = "a\n|"#, "let s = \"ab|\nx"] {
            assert!(at(s), "{s:?}");
        }
        for s in ["|", "pri|", "x.l|", "/* a */ x|", r#"let s = "a" + b|"#, r#"let s = "{v.l|}""#, "let x = 1 /|", "// c\nx|"] {
            assert!(!at(s), "{s:?}");
        }
    }
}

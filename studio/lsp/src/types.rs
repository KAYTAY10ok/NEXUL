//! The data of the protocol used by the client, read from and written to JSON, and the
//! helpers an editor needs around them: positions (UTF-8, UTF-16 or UTF-32 code units) to
//! byte offsets and back, file paths to `file://` URIs and back, applying text edits.

use crate::json::Json;
use std::path::{Path, PathBuf};

/// Unit of the `character` of a [`Position`]. LSP's default is UTF-16; the client offers
/// UTF-8 too, and the server chooses (see `Client::position_encoding`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum PositionEncoding {
    Utf8,
    #[default]
    Utf16,
    Utf32,
}

impl PositionEncoding {
    /// Name in the protocol: `"utf-8"`, `"utf-16"`, `"utf-32"`.
    pub fn name(self) -> &'static str {
        match self {
            PositionEncoding::Utf8 => "utf-8",
            PositionEncoding::Utf16 => "utf-16",
            PositionEncoding::Utf32 => "utf-32",
        }
    }

    pub fn from_name(name: &str) -> Option<PositionEncoding> {
        match name {
            "utf-8" => Some(PositionEncoding::Utf8),
            "utf-16" => Some(PositionEncoding::Utf16),
            "utf-32" => Some(PositionEncoding::Utf32),
            _ => None,
        }
    }

    fn units(self, c: char) -> usize {
        match self {
            PositionEncoding::Utf8 => c.len_utf8(),
            PositionEncoding::Utf16 => c.len_utf16(),
            PositionEncoding::Utf32 => 1,
        }
    }
}

/// Zero-based line and character (in code units of the [`PositionEncoding`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub fn new(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    pub fn to_json(self) -> Json {
        Json::object([("line", Json::from(self.line)), ("character", Json::from(self.character))])
    }

    pub fn from_json(v: &Json) -> Option<Position> {
        Some(Position { line: v.get("line")?.as_u32()?, character: v.get("character")?.as_u32()? })
    }
}

/// A range between two positions, end excluded.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Range {
        Range { start, end }
    }

    pub fn to_json(self) -> Json {
        Json::object([("start", self.start.to_json()), ("end", self.end.to_json())])
    }

    pub fn from_json(v: &Json) -> Option<Range> {
        Some(Range { start: Position::from_json(v.get("start")?)?, end: Position::from_json(v.get("end")?)? })
    }
}

/// A range in a document designated by its URI.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

impl Location {
    /// The local path of the document, for a `file://` URI.
    pub fn path(&self) -> Option<PathBuf> {
        uri_to_path(&self.uri)
    }

    pub fn from_json(v: &Json) -> Option<Location> {
        Some(Location { uri: v.get("uri")?.as_str()?.to_string(), range: Range::from_json(v.get("range")?)? })
    }

    /// The result of `textDocument/definition` (and similar): a location, a list of
    /// locations or of location links, or `null`.
    pub fn list_from_json(v: &Json) -> Vec<Location> {
        let one = |v: &Json| {
            Location::from_json(v).or_else(|| {
                // LocationLink: the target, at its selection range (the name).
                let range = v.get("targetSelectionRange").or_else(|| v.get("targetRange"))?;
                Some(Location { uri: v.get("targetUri")?.as_str()?.to_string(), range: Range::from_json(range)? })
            })
        };
        match v {
            Json::Array(items) => items.iter().filter_map(one).collect(),
            Json::Null => Vec::new(),
            v => one(v).into_iter().collect(),
        }
    }
}

/// Severity of a diagnostic.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

impl Severity {
    pub fn from_json(v: &Json) -> Option<Severity> {
        match v.as_i64()? {
            1 => Some(Severity::Error),
            2 => Some(Severity::Warning),
            3 => Some(Severity::Information),
            4 => Some(Severity::Hint),
            _ => None,
        }
    }
}

/// A diagnostic published by the server (`textDocument/publishDiagnostics`).
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Option<Severity>,
    /// The code, a number or a string in the protocol, as text.
    pub code: Option<String>,
    /// Who produced it: `"rustc"`, `"clang"`, `"Pyright"`...
    pub source: Option<String>,
    pub message: String,
}

impl Diagnostic {
    pub fn from_json(v: &Json) -> Option<Diagnostic> {
        let code = v.get("code").and_then(|c| match c {
            Json::String(s) => Some(s.clone()),
            Json::Int(i) => Some(i.to_string()),
            _ => None,
        });
        Some(Diagnostic {
            range: Range::from_json(v.get("range")?)?,
            severity: v.get("severity").and_then(Severity::from_json),
            code,
            source: v.get("source").and_then(Json::as_str).map(str::to_string),
            message: v.get("message")?.as_str()?.to_string(),
        })
    }
}

/// Replace `range` with `new_text`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

impl TextEdit {
    pub fn from_json(v: &Json) -> Option<TextEdit> {
        Some(TextEdit { range: Range::from_json(v.get("range")?)?, new_text: v.get("newText")?.as_str()?.to_string() })
    }

    pub fn list_from_json(v: &Json) -> Vec<TextEdit> {
        v.as_array().map(|a| a.iter().filter_map(TextEdit::from_json).collect()).unwrap_or_default()
    }
}

/// Text of a `MarkupContent`, a `MarkedString` or a list of them, as Markdown when
/// `markdown` is true, plain text otherwise.
fn markup(v: &Json) -> Option<(String, bool)> {
    match v {
        Json::String(s) => Some((s.clone(), true)),
        Json::Array(items) => {
            let parts: Vec<(String, bool)> = items.iter().filter_map(markup).filter(|(s, _)| !s.is_empty()).collect();
            let markdown = parts.iter().any(|(_, m)| *m);
            Some((parts.into_iter().map(|(s, _)| s).collect::<Vec<_>>().join("\n\n"), markdown))
        }
        Json::Object(_) => {
            let value = v.get("value")?.as_str()?.to_string();
            if let Some(kind) = v.get("kind").and_then(Json::as_str) {
                Some((value, kind == "markdown"))
            } else {
                let language = v.get("language").and_then(Json::as_str).unwrap_or("");
                Some((format!("```{language}\n{value}\n```"), true))
            }
        }
        _ => None,
    }
}

/// Result of `textDocument/hover`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hover {
    /// The text to show.
    pub contents: String,
    /// `contents` is Markdown (otherwise plain text).
    pub markdown: bool,
    /// The range the hover applies to, if the server gives it.
    pub range: Option<Range>,
}

impl Hover {
    /// `None` for `null` or empty contents.
    pub fn from_json(v: &Json) -> Option<Hover> {
        let (contents, markdown) = markup(v.get("contents")?)?;
        if contents.trim().is_empty() {
            return None;
        }
        Some(Hover { contents, markdown, range: v.get("range").and_then(Range::from_json) })
    }
}

/// Names of the `CompletionItemKind` values 1 to 25.
const COMPLETION_KINDS: [&str; 25] = [
    "Text",
    "Method",
    "Function",
    "Constructor",
    "Field",
    "Variable",
    "Class",
    "Interface",
    "Module",
    "Property",
    "Unit",
    "Value",
    "Enum",
    "Keyword",
    "Snippet",
    "Color",
    "File",
    "Reference",
    "Folder",
    "EnumMember",
    "Constant",
    "Struct",
    "Event",
    "Operator",
    "TypeParameter",
];

/// A completion proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    /// `CompletionItemKind` (1 to 25), see [`CompletionItem::kind_name`].
    pub kind: Option<u32>,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
    /// The `insertText` is a snippet (`$1`, `${2:x}`): the client asks for plain text, but
    /// some servers send snippets anyway.
    pub snippet: bool,
    /// Precise edit to apply (for an `InsertReplaceEdit`, its insert range).
    pub edit: Option<TextEdit>,
    pub filter_text: Option<String>,
    pub sort_text: Option<String>,
}

impl CompletionItem {
    pub fn from_json(v: &Json) -> Option<CompletionItem> {
        let text = |k: &str| v.get(k).and_then(Json::as_str).map(str::to_string);
        let edit = v.get("textEdit").and_then(|e| {
            TextEdit::from_json(e).or_else(|| {
                Some(TextEdit {
                    range: Range::from_json(e.get("insert")?)?,
                    new_text: e.get("newText")?.as_str()?.to_string(),
                })
            })
        });
        Some(CompletionItem {
            label: v.get("label")?.as_str()?.to_string(),
            kind: v.get("kind").and_then(Json::as_u32),
            detail: text("detail"),
            documentation: v.get("documentation").and_then(markup).map(|(s, _)| s),
            insert_text: text("insertText"),
            snippet: v.get("insertTextFormat").and_then(Json::as_i64) == Some(2),
            edit,
            filter_text: text("filterText"),
            sort_text: text("sortText"),
        })
    }

    /// `"Function"`, `"Keyword"`... (`"Text"` when unknown).
    pub fn kind_name(&self) -> &'static str {
        self.kind.and_then(|k| COMPLETION_KINDS.get((k as usize).wrapping_sub(1))).copied().unwrap_or("Text")
    }

    /// The text to insert: the edit's, else `insertText`, else the label.
    pub fn text(&self) -> &str {
        self.edit.as_ref().map(|e| e.new_text.as_str()).or(self.insert_text.as_deref()).unwrap_or(&self.label)
    }
}

/// Result of `textDocument/completion`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Completions {
    /// The list is partial: typing more should ask again.
    pub incomplete: bool,
    pub items: Vec<CompletionItem>,
}

impl Completions {
    /// From a list of items, a `CompletionList` or `null`.
    pub fn from_json(v: &Json) -> Completions {
        let (incomplete, items) = match v {
            Json::Array(items) => (false, Some(items)),
            Json::Object(_) => (
                v.get("isIncomplete").and_then(Json::as_bool).unwrap_or(false),
                v.get("items").and_then(Json::as_array),
            ),
            _ => (false, None),
        };
        Completions {
            incomplete,
            items: items.map(|a| a.iter().filter_map(CompletionItem::from_json).collect()).unwrap_or_default(),
        }
    }
}

/// The edits of one document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEdit {
    pub uri: String,
    pub edits: Vec<TextEdit>,
}

impl FileEdit {
    pub fn path(&self) -> Option<PathBuf> {
        uri_to_path(&self.uri)
    }
}

/// Result of `textDocument/rename`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct WorkspaceEdit {
    /// Text edits, per document.
    pub files: Vec<FileEdit>,
    /// File creations, renamings and deletions (`CreateFile`, `RenameFile`, `DeleteFile`),
    /// as sent by the server, in order. The client does not announce support for them, so
    /// servers should not send any.
    pub resource_operations: Vec<Json>,
}

impl WorkspaceEdit {
    pub fn from_json(v: &Json) -> WorkspaceEdit {
        let mut out = WorkspaceEdit::default();
        if let Some(changes) = v.get("documentChanges").and_then(Json::as_array) {
            for c in changes {
                match c.path(&["textDocument", "uri"]).and_then(Json::as_str) {
                    Some(uri) => out.add(uri, TextEdit::list_from_json(c.get("edits").unwrap_or(&Json::Null))),
                    None => out.resource_operations.push(c.clone()),
                }
            }
        } else if let Some(changes) = v.get("changes").and_then(Json::as_object) {
            for (uri, edits) in changes {
                out.add(uri, TextEdit::list_from_json(edits));
            }
        }
        out
    }

    fn add(&mut self, uri: &str, edits: Vec<TextEdit>) {
        match self.files.iter_mut().find(|f| f.uri == uri) {
            Some(f) => f.edits.extend(edits),
            None => self.files.push(FileEdit { uri: uri.to_string(), edits }),
        }
    }
}

/// Options of `textDocument/formatting`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FormattingOptions {
    pub tab_size: u32,
    pub insert_spaces: bool,
}

impl Default for FormattingOptions {
    fn default() -> FormattingOptions {
        FormattingOptions { tab_size: 4, insert_spaces: true }
    }
}

impl FormattingOptions {
    pub fn to_json(self) -> Json {
        Json::object([("tabSize", Json::from(self.tab_size)), ("insertSpaces", Json::from(self.insert_spaces))])
    }
}

/// Byte offset of the start of line `line` (lines end with `\n`, `\r\n` or `\r`), or
/// `None` past the last line.
fn line_start(text: &str, line: u32) -> Option<usize> {
    if line == 0 {
        return Some(0);
    }
    let b = text.as_bytes();
    let mut seen = 0;
    let mut i = 0;
    while i < b.len() {
        let len = match b[i] {
            b'\r' if b.get(i + 1) == Some(&b'\n') => 2,
            b'\r' | b'\n' => 1,
            _ => {
                i += 1;
                continue;
            }
        };
        i += len;
        seen += 1;
        if seen == line {
            return Some(i);
        }
    }
    None
}

/// The position of byte `offset` of `text` (clamped to the text, rounded down to a
/// character boundary).
pub fn offset_to_position(text: &str, offset: usize, encoding: PositionEncoding) -> Position {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let b = text.as_bytes();
    let (mut line, mut start, mut i) = (0u32, 0usize, 0usize);
    while i < offset {
        match b[i] {
            c @ (b'\r' | b'\n') => {
                let len = if c == b'\r' && b.get(i + 1) == Some(&b'\n') { 2 } else { 1 };
                if i + len > offset {
                    break; // between `\r` and `\n`: still on this line
                }
                line += 1;
                i += len;
                start = i;
            }
            _ => i += 1,
        }
    }
    let character: usize = text[start..offset].chars().map(|c| encoding.units(c)).sum();
    Position { line, character: character as u32 }
}

/// The byte offset of `pos` in `text`. As the protocol says, a character past the end of
/// its line means the end of the line, and a line past the end, the end of the text; a
/// position inside a character (half of a UTF-16 surrogate pair) is rounded down.
pub fn position_to_offset(text: &str, pos: Position, encoding: PositionEncoding) -> usize {
    let Some(start) = line_start(text, pos.line) else {
        return text.len();
    };
    let mut units = 0usize;
    let mut offset = start;
    for c in text[start..].chars() {
        if c == '\n' || c == '\r' {
            break;
        }
        let u = encoding.units(c);
        if units + u > pos.character as usize {
            break;
        }
        units += u;
        offset += c.len_utf8();
    }
    offset
}

/// Applies `edits` (positions in `text` before any edit, not overlapping, as the protocol
/// requires) and returns the new text, or `None` if two edits overlap. Insertions at the
/// same position are applied in the order of the list.
pub fn apply_edits(text: &str, edits: &[TextEdit], encoding: PositionEncoding) -> Option<String> {
    let mut spans: Vec<(usize, usize, usize, &str)> = edits
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let a = position_to_offset(text, e.range.start, encoding);
            let b = position_to_offset(text, e.range.end, encoding).max(a);
            (a, b, i, e.new_text.as_str())
        })
        .collect();
    spans.sort_by_key(|&(a, b, i, _)| (a, b, i));
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (a, b, _, new) in spans {
        if a < last {
            return None;
        }
        out.push_str(&text[last..a]);
        out.push_str(new);
        last = b;
    }
    out.push_str(&text[last..]);
    Some(out)
}

/// The `file://` URI of a path (made absolute first). On Windows,
/// `C:\Users\a b\x.rs` gives `file:///C:/Users/a%20b/x.rs` and `\\server\share\x`
/// gives `file://server/share/x`.
pub fn path_to_uri(path: &Path) -> String {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let s = abs.to_string_lossy().replace('\\', "/");
    // Verbatim prefixes: `//?/C:/x` and `//?/UNC/server/share`.
    let s = if let Some(rest) = s.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else if let Some(rest) = s.strip_prefix("//?/") {
        rest.to_string()
    } else {
        s
    };
    let (authority, path) = match s.strip_prefix("//") {
        Some(unc) => match unc.split_once('/') {
            Some((host, rest)) => (host.to_string(), format!("/{rest}")),
            None => (unc.to_string(), "/".to_string()),
        },
        None if s.starts_with('/') => (String::new(), s),
        None => (String::new(), format!("/{s}")),
    };
    let mut out = format!("file://{}", encode(&authority, false));
    out.push_str(&encode(&path, true));
    out
}

/// Percent-encodes everything except unreserved characters, `/` and, in a path, the colon
/// of a drive (`/C:`).
fn encode(s: &str, is_path: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let drive_colon = is_path
        && s.len() >= 3
        && s.as_bytes()[0] == b'/'
        && s.as_bytes()[1].is_ascii_alphabetic()
        && s.as_bytes()[2] == b':';
    for (i, b) in s.bytes().enumerate() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') || (drive_colon && i == 2) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The local path of a `file://` URI (`None` for another scheme or invalid UTF-8). On
/// Windows the drive letter is upper-cased (`file:///c%3A/x` gives `C:\x`).
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.get(..7).filter(|p| p.eq_ignore_ascii_case("file://")).map(|_| &uri[7..])?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let path = decode(path)?;
    let authority = decode(authority)?;
    if cfg!(windows) {
        let b = path.as_bytes();
        if authority.is_empty() || authority.eq_ignore_ascii_case("localhost") {
            if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
                let mut p = path[1..].replace('/', "\\");
                p[..1].make_ascii_uppercase();
                if p.len() == 2 {
                    p.push('\\');
                }
                return Some(PathBuf::from(p));
            }
            return Some(PathBuf::from(path.replace('/', "\\")));
        }
        return Some(PathBuf::from(format!("\\\\{}{}", authority, path.replace('/', "\\"))));
    }
    if authority.is_empty() || authority.eq_ignore_ascii_case("localhost") {
        Some(PathBuf::from(path))
    } else {
        Some(PathBuf::from(format!("//{authority}{path}")))
    }
}

fn decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            out.push(hex(b[i + 1]) * 16 + hex(b[i + 2]));
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use PositionEncoding::*;

    #[test]
    fn positions_round_trip() {
        let text = "a€😀b\r\nxy\rz\n\né";
        for enc in [Utf8, Utf16, Utf32] {
            for (offset, _) in text.char_indices().chain([(text.len(), ' ')]) {
                let p = offset_to_position(text, offset, enc);
                let back = position_to_offset(text, p, enc);
                // Inside a CRLF, the position is the one of the `\r`.
                let inside_crlf =
                    offset > 0 && text.as_bytes()[offset - 1] == b'\r' && text[offset..].starts_with('\n');
                let expect = if inside_crlf { offset - 1 } else { offset };
                assert_eq!(back, expect, "{enc:?} offset {offset} -> {p:?}");
            }
        }
        // `😀` is 4 bytes, 2 UTF-16 units, 1 UTF-32 unit.
        assert_eq!(offset_to_position(text, 8, Utf16), Position::new(0, 4));
        assert_eq!(offset_to_position(text, 8, Utf8), Position::new(0, 8));
        assert_eq!(offset_to_position(text, 8, Utf32), Position::new(0, 3));
        assert_eq!(offset_to_position(text, 11, Utf16), Position::new(1, 0)); // after \r\n
        assert_eq!(offset_to_position(text, 14, Utf16), Position::new(2, 0)); // after \r
        assert_eq!(offset_to_position(text, 17, Utf16), Position::new(4, 0));
        // Clamping, as the protocol says.
        assert_eq!(position_to_offset(text, Position::new(1, 99), Utf16), 13);
        assert_eq!(position_to_offset(text, Position::new(99, 0), Utf16), text.len());
        // Half of a surrogate pair rounds down.
        assert_eq!(position_to_offset(text, Position::new(0, 3), Utf16), 4);
        assert_eq!(offset_to_position(text, 6, Utf16), Position::new(0, 2)); // inside 😀
    }

    #[test]
    fn edits() {
        let text = "fn  main( ) {\n  x;   \n}\n";
        let e = |l1, c1, l2, c2, t: &str| TextEdit {
            range: Range::new(Position::new(l1, c1), Position::new(l2, c2)),
            new_text: t.into(),
        };
        let edits = [e(1, 4, 1, 7, ""), e(0, 2, 0, 4, " "), e(0, 9, 0, 10, ""), e(1, 0, 1, 2, "    ")];
        assert_eq!(apply_edits(text, &edits, Utf16).unwrap(), "fn main() {\n    x;\n}\n");
        // Insertions at one position keep their order.
        assert_eq!(apply_edits("ab", &[e(0, 1, 0, 1, "1"), e(0, 1, 0, 1, "2")], Utf16).unwrap(), "a12b");
        assert_eq!(apply_edits("abc", &[e(0, 0, 0, 2, ""), e(0, 1, 0, 3, "")], Utf16), None);
        assert_eq!(apply_edits("é😀x", &[e(0, 3, 0, 4, "y")], Utf16).unwrap(), "é😀y");
    }

    #[test]
    fn uris() {
        if cfg!(windows) {
            assert_eq!(path_to_uri(Path::new(r"C:\Users\a b\x#1.rs")), "file:///C:/Users/a%20b/x%231.rs");
            assert_eq!(path_to_uri(Path::new(r"\\server\share\é.nx")), "file://server/share/%C3%A9.nx");
            assert_eq!(path_to_uri(Path::new(r"\\?\C:\x")), "file:///C:/x");
            assert_eq!(uri_to_path("file:///c%3A/Users/a%20b/x%231.rs"), Some(PathBuf::from(r"C:\Users\a b\x#1.rs")));
            assert_eq!(uri_to_path("file:///C:/x"), Some(PathBuf::from(r"C:\x")));
            assert_eq!(uri_to_path("file:///d:"), Some(PathBuf::from(r"D:\")));
            assert_eq!(uri_to_path("file://server/share/%C3%A9.nx"), Some(PathBuf::from(r"\\server\share\é.nx")));
            let p = Path::new(r"C:\Program Files\NEXUL\été 😀\main.nx");
            assert_eq!(uri_to_path(&path_to_uri(p)).as_deref(), Some(p));
        } else {
            assert_eq!(path_to_uri(Path::new("/home/a b/x.rs")), "file:///home/a%20b/x.rs");
            assert_eq!(uri_to_path("file:///home/a%20b/x.rs"), Some(PathBuf::from("/home/a b/x.rs")));
        }
        assert_eq!(uri_to_path("https://example.com/x"), None);
        assert_eq!(uri_to_path("file:///%FF"), None);
        assert!(uri_to_path("file:///a%2").is_some());
    }

    #[test]
    fn results_from_json() {
        let j = |s: &str| Json::parse(s).unwrap();
        let r = r#"{"start":{"line":1,"character":2},"end":{"line":1,"character":5}}"#;
        let loc = format!(r#"{{"uri":"file:///x","range":{r}}}"#);
        assert_eq!(Location::list_from_json(&j(&loc)).len(), 1);
        assert_eq!(Location::list_from_json(&j(&format!("[{loc},{loc}]"))).len(), 2);
        assert_eq!(Location::list_from_json(&j("null")), vec![]);
        let link = format!(r#"[{{"targetUri":"file:///y","targetRange":{r},"targetSelectionRange":{r}}}]"#);
        assert_eq!(Location::list_from_json(&j(&link))[0].uri, "file:///y");

        let h = Hover::from_json(&j(r#"{"contents":{"kind":"markdown","value":"**x**"}}"#)).unwrap();
        assert_eq!((h.contents.as_str(), h.markdown), ("**x**", true));
        let h = Hover::from_json(&j(r#"{"contents":[{"language":"rust","value":"fn f()"},"doc"]}"#)).unwrap();
        assert_eq!(h.contents, "```rust\nfn f()\n```\n\ndoc");
        let h = Hover::from_json(&j(
            r#"{"contents":{"kind":"plaintext","value":"t"},"range":RANGE}"#.replace("RANGE", r).as_str(),
        ))
        .unwrap();
        assert!(!h.markdown && h.range.is_some());
        assert_eq!(Hover::from_json(&j(r#"{"contents":""}"#)), None);

        let c = Completions::from_json(&j(
            r#"{"isIncomplete":true,"items":[{"label":"len","kind":3,"detail":"fn() -> usize","documentation":{"kind":"markdown","value":"Length"},"insertTextFormat":2,"insertText":"len()$0"},{"label":"x","textEdit":{"newText":"xx","insert":RANGE,"replace":RANGE}},{"nolabel":1}]}"#
                .replace("RANGE", r)
                .as_str(),
        ));
        assert!(c.incomplete);
        assert_eq!(c.items.len(), 2);
        assert_eq!((c.items[0].kind_name(), c.items[0].snippet, c.items[0].text()), ("Function", true, "len()$0"));
        assert_eq!(c.items[0].documentation.as_deref(), Some("Length"));
        assert_eq!((c.items[1].kind_name(), c.items[1].text()), ("Text", "xx"));
        assert_eq!(Completions::from_json(&j(r#"[{"label":"a"}]"#)).items.len(), 1);
        assert_eq!(Completions::from_json(&Json::Null), Completions::default());

        let d =
            Diagnostic::from_json(&j(&format!(r#"{{"range":{r},"severity":2,"code":42,"source":"s","message":"m"}}"#)))
                .unwrap();
        assert_eq!(
            (d.severity, d.code.as_deref(), d.source.as_deref(), d.message.as_str()),
            (Some(Severity::Warning), Some("42"), Some("s"), "m")
        );

        let w = WorkspaceEdit::from_json(&j(&format!(
            r#"{{"changes":{{"file:///a":[{{"range":{r},"newText":"n"}}],"file:///b":[]}}}}"#
        )));
        assert_eq!((w.files.len(), w.files[0].edits.len()), (2, 1));
        let w = WorkspaceEdit::from_json(&j(&format!(
            r#"{{"documentChanges":[{{"textDocument":{{"uri":"file:///a","version":3}},"edits":[{{"range":{r},"newText":"n"}}]}},{{"kind":"create","uri":"file:///c"}},{{"textDocument":{{"uri":"file:///a","version":3}},"edits":[{{"range":{r},"newText":"m"}}]}}]}}"#
        )));
        assert_eq!((w.files.len(), w.files[0].edits.len(), w.resource_operations.len()), (1, 2, 1));
    }
}

//! Searching the files of the workspace (« Rechercher dans les fichiers ») and replacing.
//!
//! Plain text, with or without case, whole words or not. Folders ignored by the watch
//! (`.git`, `target`…), binary files and very large files are skipped; the texts of the
//! open editors (unsaved) are searched instead of the files on disk. Columns are counted
//! in characters, from 0.

use crate::watch;
use crate::workspace;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Files larger than this are not searched.
pub const MAX_FILE: u64 = 4 * 1024 * 1024;
/// The search stops after this many matches.
pub const MAX_HITS: usize = 10_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    /// Upper and lower case differ.
    pub case: bool,
    /// Only whole words (not inside a longer name).
    pub word: bool,
}

/// One match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The file, relative to the root, with `/`.
    pub rel: String,
    /// Line, from 0.
    pub line: usize,
    /// Columns of the match in the line, in characters (end excluded).
    pub start: usize,
    pub end: usize,
    /// The line around the match (at most about 200 characters), and the match in it.
    pub preview: String,
    pub preview_start: usize,
    pub preview_end: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Found {
    pub hits: Vec<Hit>,
    /// Files that have at least one match.
    pub files: usize,
    /// Stopped at [`MAX_HITS`] (or asked to stop): there may be more.
    pub truncated: bool,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn same(a: char, b: char, case: bool) -> bool {
    a == b || (!case && a.to_lowercase().eq(b.to_lowercase()))
}

/// The matches in one line: columns in characters, in order, without overlap.
pub fn find_in_line(line: &str, q: &Query) -> Vec<(usize, usize)> {
    let needle: Vec<char> = q.text.chars().collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let hay: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        let hit = needle.iter().enumerate().all(|(k, &n)| same(hay[i + k], n, q.case))
            && (!q.word || ((i == 0 || !is_word_char(hay[i - 1])) && hay.get(i + needle.len()).map_or(true, |&c| !is_word_char(c))));
        if hit {
            out.push((i, i + needle.len()));
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

/// The lines of a text, without their end (`\n` or `\r\n`).
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l))
}

fn preview(line: &str, start: usize, end: usize) -> (String, usize, usize) {
    let chars: Vec<char> = line.chars().collect();
    // Leading spaces are not shown; at most 60 characters before the match.
    let indent = chars.iter().take_while(|c| c.is_whitespace()).count().min(start);
    let from = indent.max(start.saturating_sub(60));
    let to = chars.len().min(end.max(from + 200));
    let mut s: String = chars[from..to].iter().collect();
    let mut shift = 0;
    if from > indent {
        s.insert(0, '…');
        shift = 1;
    }
    if to < chars.len() {
        s.push('…');
    }
    (s, start - from + shift, end.min(to) - from + shift)
}

/// The matches in a text (`rel` is copied into each).
pub fn find_in_text(rel: &str, text: &str, q: &Query, max: usize) -> Vec<Hit> {
    let mut out = Vec::new();
    for (n, line) in lines(text).enumerate() {
        for (start, end) in find_in_line(line, q) {
            if out.len() >= max {
                return out;
            }
            let (preview, preview_start, preview_end) = preview(line, start, end);
            out.push(Hit { rel: rel.to_string(), line: n, start, end, preview, preview_start, preview_end });
        }
    }
    out
}

/// Replaces every match of `q` in `text`; gives the new text and the number of replacements.
pub fn replace_in_text(text: &str, q: &Query, with: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut count = 0;
    let mut first = true;
    for piece in text.split('\n') {
        if !first {
            out.push('\n');
        }
        first = false;
        let (line, cr) = match piece.strip_suffix('\r') {
            Some(l) => (l, "\r"),
            None => (piece, ""),
        };
        let chars: Vec<char> = line.chars().collect();
        let mut at = 0;
        for (start, end) in find_in_line(line, q) {
            out.extend(&chars[at..start]);
            out.push_str(with);
            at = end;
            count += 1;
        }
        out.extend(&chars[at..]);
        out.push_str(cr);
    }
    (out, count)
}

/// The text of a file of the workspace, if it is a text file small enough to search.
pub fn read_text(path: &Path) -> Option<(String, &'static str)> {
    let md = std::fs::metadata(path).ok()?;
    if !md.is_file() || md.len() > MAX_FILE {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    // A NUL byte early on: binary (UTF-16 text is recognized by its byte order mark).
    let utf16 = bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]);
    if !utf16 && bytes.iter().take(8000).any(|&b| b == 0) {
        return None;
    }
    workspace::decode_text(&bytes)
}

/// Every file under `root` that may be searched, relative with `/`, in the order of the
/// explorer (folders by name, then files by name). Links to folders are not followed.
pub fn files(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<(bool, String)> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let t = e.file_type().ok()?;
                if t.is_dir() && watch::IGNORED.contains(&name.as_str()) || watch::is_hidden_name(&name) {
                    return None;
                }
                (t.is_dir() || t.is_file()).then_some((t.is_dir(), name))
            })
            .collect();
        entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));
        for (is_dir, name) in entries {
            let r = if rel.is_empty() { name.clone() } else { format!("{}/{}", rel, name) };
            if is_dir {
                walk(&dir.join(&name), &r, out);
            } else {
                out.push(r);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, "", &mut out);
    out
}

/// Searches the workspace. `open`: the texts of the open editors by relative path, used
/// instead of the files. `stop`, set from another thread, ends the search early.
pub fn search(root: &Path, q: &Query, open: &HashMap<String, String>, stop: &AtomicBool) -> Found {
    let mut found = Found::default();
    if q.text.is_empty() {
        return found;
    }
    for rel in files(root) {
        if stop.load(Ordering::Relaxed) {
            found.truncated = true;
            break;
        }
        let owned;
        let text = match open.get(&rel) {
            Some(t) => t.as_str(),
            None => match read_text(&root.join(&rel)) {
                Some((t, _)) => {
                    owned = t;
                    owned.as_str()
                }
                None => continue,
            },
        };
        let hits = find_in_text(&rel, text, q, MAX_HITS - found.hits.len());
        if !hits.is_empty() {
            found.files += 1;
            found.hits.extend(hits);
        }
        if found.hits.len() >= MAX_HITS {
            found.truncated = true;
            break;
        }
    }
    found
}

/// Replaces every match in the files of `rels` on disk (not open in an editor), keeping
/// their encoding; gives the number of replacements and the files that could not be written.
pub fn replace_in_files(root: &Path, rels: &[String], q: &Query, with: &str) -> (usize, Vec<(String, String)>) {
    let mut count = 0;
    let mut failed = Vec::new();
    for rel in rels {
        let Some((text, encoding)) = read_text(&root.join(rel)) else { continue };
        let (new, n) = replace_in_text(&text, q, with);
        if n == 0 {
            continue;
        }
        let (bytes, _) = workspace::encode_text(&new, encoding);
        match workspace::save(root, rel, &bytes) {
            Ok(()) => count += n,
            Err(e) => failed.push((rel.clone(), e.message)),
        }
    }
    (count, failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(text: &str, case: bool, word: bool) -> Query {
        Query { text: text.into(), case, word }
    }

    #[test]
    fn matches_in_a_line() {
        assert_eq!(find_in_line("Total = total + TOTAL", &q("total", false, false)), vec![(0, 5), (8, 13), (16, 21)]);
        assert_eq!(find_in_line("Total = total + TOTAL", &q("total", true, false)), vec![(8, 13)]);
        assert_eq!(find_in_line("x total totaux sous_total", &q("total", false, true)), vec![(2, 7)]);
        // Characters, not bytes; letters with accents fold too.
        assert_eq!(find_in_line("été Été", &q("été", false, false)), vec![(0, 3), (4, 7)]);
        assert_eq!(find_in_line("aaaa", &q("aa", true, false)), vec![(0, 2), (2, 4)]);
        assert!(find_in_line("abc", &q("", false, false)).is_empty());
    }

    #[test]
    fn previews_and_replacement() {
        let hits = find_in_text("a.nx", "fn main() {\r\n    let total = 1\r\n}", &q("total", false, false), 10);
        assert_eq!(hits.len(), 1);
        let h = &hits[0];
        assert_eq!((h.line, h.start, h.end), (1, 8, 13));
        assert_eq!(&h.preview, "let total = 1");
        assert_eq!(h.preview.chars().skip(h.preview_start).take(h.preview_end - h.preview_start).collect::<String>(), "total");
        let long = format!("{}cible{}", "x".repeat(300), "y".repeat(300));
        let h = &find_in_text("b", &long, &q("cible", true, false), 10)[0];
        assert!(h.preview.starts_with('…') && h.preview.ends_with('…'));
        assert_eq!(h.preview.chars().skip(h.preview_start).take(5).collect::<String>(), "cible");
        // Line ends are kept.
        assert_eq!(replace_in_text("a x\r\nx b\nX", &q("x", false, true), "yy"), ("a yy\r\nyy b\nyy".to_string(), 3));
    }

    #[test]
    fn searches_the_workspace() {
        let root = std::env::temp_dir().join(format!("nexul-studio-search-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join("main.nx"), "// cible\nfn main() {}\n").unwrap();
        std::fs::write(root.join("src/b.nx"), "cible cible\n").unwrap();
        std::fs::write(root.join("target/out.nx"), "cible\n").unwrap();
        std::fs::write(root.join("image.bin"), b"cible\0\0\0").unwrap();
        let none = AtomicBool::new(false);
        let found = search(&root, &q("cible", true, false), &HashMap::new(), &none);
        let places: Vec<(&str, usize, usize)> = found.hits.iter().map(|h| (h.rel.as_str(), h.line, h.start)).collect();
        assert_eq!(places, vec![("src/b.nx", 0, 0), ("src/b.nx", 0, 6), ("main.nx", 0, 3)]);
        assert_eq!(found.files, 2);
        // The unsaved text of an open editor counts, not the file.
        let open = HashMap::from([("main.nx".to_string(), "rien\n".to_string())]);
        assert_eq!(search(&root, &q("cible", true, false), &open, &none).files, 1);
        // Replacing on disk.
        let (n, failed) = replace_in_files(&root, &["main.nx".into(), "src/b.nx".into()], &q("cible", true, false), "but");
        assert_eq!((n, failed.len()), (3, 0));
        assert_eq!(std::fs::read_to_string(root.join("src/b.nx")).unwrap(), "but but\n");
        let _ = std::fs::remove_dir_all(&root);
    }
}

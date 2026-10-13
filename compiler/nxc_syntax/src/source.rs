//! Source files and positions.
//!
//! Every syntax node carries a [`Span`]: a file and a half-open byte range. Line and
//! column numbers are computed lazily, only when a diagnostic is rendered.

use std::fmt;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(pub u32);

/// A half-open byte range `[lo, hi)` inside one source file.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Span {
    pub file: FileId,
    pub lo: u32,
    pub hi: u32,
}

impl Span {
    /// A span that points nowhere; used for compiler-synthesized nodes.
    pub const DUMMY: Span = Span { file: FileId(u32::MAX), lo: 0, hi: 0 };

    pub fn new(file: FileId, lo: u32, hi: u32) -> Span {
        Span { file, lo, hi }
    }

    pub fn is_dummy(&self) -> bool {
        self.file.0 == u32::MAX
    }

    /// The smallest span covering both `self` and `other` (same file expected).
    pub fn to(self, other: Span) -> Span {
        if self.is_dummy() {
            return other;
        }
        if other.is_dummy() || other.file != self.file {
            return self;
        }
        Span { file: self.file, lo: self.lo.min(other.lo), hi: self.hi.max(other.hi) }
    }

    /// An empty span located at the end of `self`.
    pub fn end(self) -> Span {
        Span { file: self.file, lo: self.hi, hi: self.hi }
    }

    pub fn start(self) -> Span {
        Span { file: self.file, lo: self.lo, hi: self.lo }
    }

    pub fn len(&self) -> u32 {
        self.hi - self.lo
    }

    pub fn is_empty(&self) -> bool {
        self.hi == self.lo
    }
}

pub struct SourceFile {
    /// Name shown in diagnostics (usually a path relative to the source root).
    pub name: String,
    pub text: String,
    /// Byte offset of the first character of each line.
    line_starts: Vec<u32>,
}

impl SourceFile {
    fn new(name: String, text: String) -> SourceFile {
        let mut line_starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i as u32 + 1);
            }
        }
        SourceFile { name, text, line_starts }
    }

    /// 0-based line index containing byte offset `pos`.
    pub fn line_index(&self, pos: u32) -> usize {
        match self.line_starts.binary_search(&pos) {
            Ok(i) => i,
            Err(i) => i - 1,
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// The text of 0-based line `idx`, without its line terminator.
    pub fn line_text(&self, idx: usize) -> &str {
        let start = self.line_starts[idx] as usize;
        let end = if idx + 1 < self.line_starts.len() {
            self.line_starts[idx + 1] as usize
        } else {
            self.text.len()
        };
        let line = &self.text[start..end];
        line.trim_end_matches(['\n', '\r'])
    }

    pub fn line_start(&self, idx: usize) -> u32 {
        self.line_starts[idx]
    }

    /// 1-based (line, column); the column counts Unicode scalar values.
    pub fn line_col(&self, pos: u32) -> (usize, usize) {
        let idx = self.line_index(pos);
        let start = self.line_starts[idx] as usize;
        let pos = (pos as usize).min(self.text.len());
        let col = self.text.get(start..pos).map(|s| s.chars().count()).unwrap_or(0);
        (idx + 1, col + 1)
    }
}

#[derive(Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> SourceMap {
        SourceMap { files: Vec::new() }
    }

    pub fn add(&mut self, name: impl Into<String>, text: impl Into<String>) -> FileId {
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceFile::new(name.into(), text.into()));
        id
    }

    pub fn get(&self, id: FileId) -> &SourceFile {
        &self.files[id.0 as usize]
    }

    pub fn try_get(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0 as usize)
    }

    pub fn snippet(&self, span: Span) -> &str {
        match self.try_get(span.file) {
            Some(f) => f.text.get(span.lo as usize..span.hi as usize).unwrap_or(""),
            None => "",
        }
    }

    /// `file:line:col` for the start of `span`.
    pub fn location(&self, span: Span) -> Location<'_> {
        match self.try_get(span.file) {
            Some(f) => {
                let (line, col) = f.line_col(span.lo);
                Location { file: &f.name, line, col }
            }
            None => Location { file: "<compiler>", line: 0, col: 0 },
        }
    }
}

pub struct Location<'a> {
    pub file: &'a str,
    pub line: usize,
    pub col: usize,
}

impl fmt::Display for Location<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_counts_chars() {
        let mut sm = SourceMap::new();
        let f = sm.add("a.nx", "let é = 1\nfoo\n");
        let file = sm.get(f);
        assert_eq!(file.line_col(0), (1, 1));
        // byte 6 is after "let é" (é is 2 bytes) -> column 6
        assert_eq!(file.line_col(6), (1, 6));
        assert_eq!(file.line_col(11), (2, 1));
        assert_eq!(file.line_text(1), "foo");
    }
}

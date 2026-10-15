//! Lexical highlighting for NEXUL Studio.
//!
//! This crate has no dependency, in particular none on GPUI: it turns source text into
//! classified byte ranges, which the editor maps to the colors of its theme.
//!
//! - [`tokens`] tokenizes a whole text;
//! - [`Highlighter`] keeps the tokens of every line and, after an edit, re-tokenizes only
//!   the lines whose tokens can have changed;
//! - [`tokenize_line`] is the primitive both are built on, for an editor that keeps its
//!   own cache of lines.
//!
//! The tokenizer never fails and never panics: text being typed (an unclosed string,
//! comment or interpolation) is highlighted as well as possible, and what the compiler
//! would reject is reported as [`Kind::Invalid`].

use std::ops::Range;
use std::path::Path;

pub mod nexul;

/// Class of a token, mapped to a color by the theme of the editor.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// Declaration and other keywords: `fn`, `let`, `struct`, `pub`, `and`, `as`, `self`,
    /// and the words reserved for future versions (`async`, `yield`...).
    Keyword,
    /// Control-flow keywords: `if`, `else`, `match`, `while`, `for`, `return`, `break`,
    /// `continue`.
    Control,
    /// Primitive types, `Self`, names written in `CamelCase`, names being declared by
    /// `struct`, `enum`, `trait` or `type`.
    Type,
    /// Name of a function being declared (`fn name`) or called (`name(`, `name[T](`).
    Function,
    /// Other identifiers: variables, parameters, fields, modules, labels.
    Variable,
    /// `true`, `false` and names written in `UPPER_CASE`.
    Constant,
    /// String and character literals, quotes included, except escapes and interpolations.
    String,
    /// Valid escape sequence in a literal: `\n`, `\{`, `\u{1F600}`.
    Escape,
    /// The braces `{` `}` of an interpolation in a string, and its format spec (`:>8.2`).
    /// The interpolated expression itself gets the ordinary classes.
    Interpolation,
    /// Integer and floating-point literals.
    Number,
    /// `// ...` and `/* ... */` (nested).
    Comment,
    /// Documentation comment: `/// ...`.
    DocComment,
    /// Attribute: `@test`, `@intrinsic` (the `@` and the name).
    Attribute,
    /// Operators: `+`, `+%`, `<<=`, `==`, `..=`, `->`, `=>`, `?`...
    Operator,
    /// Delimiters and separators: `( ) [ ] { } , ; . :`, and `@` without a name.
    Punctuation,
    /// Text the compiler rejects: unknown character or escape, non-ASCII character outside
    /// strings and comments, invisible bidirectional control character ("Trojan Source",
    /// even inside strings and comments), malformed character literal, empty interpolation,
    /// invalid format spec, letters glued to a number (`10u8`).
    Invalid,
}

impl Kind {
    /// Every class, in declaration order.
    pub const ALL: [Kind; 16] = [
        Kind::Keyword,
        Kind::Control,
        Kind::Type,
        Kind::Function,
        Kind::Variable,
        Kind::Constant,
        Kind::String,
        Kind::Escape,
        Kind::Interpolation,
        Kind::Number,
        Kind::Comment,
        Kind::DocComment,
        Kind::Attribute,
        Kind::Operator,
        Kind::Punctuation,
        Kind::Invalid,
    ];

    /// Stable lowercase name, for theme files: `"keyword"`, `"doc_comment"`...
    pub fn name(self) -> &'static str {
        match self {
            Kind::Keyword => "keyword",
            Kind::Control => "control",
            Kind::Type => "type",
            Kind::Function => "function",
            Kind::Variable => "variable",
            Kind::Constant => "constant",
            Kind::String => "string",
            Kind::Escape => "escape",
            Kind::Interpolation => "interpolation",
            Kind::Number => "number",
            Kind::Comment => "comment",
            Kind::DocComment => "doc_comment",
            Kind::Attribute => "attribute",
            Kind::Operator => "operator",
            Kind::Punctuation => "punctuation",
            Kind::Invalid => "invalid",
        }
    }

    /// Inverse of [`Kind::name`].
    pub fn from_name(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.name() == name)
    }
}

/// A classified range of the text, in bytes. Tokens never overlap, come in increasing
/// order, never contain a line break, and start and end on character boundaries; the text
/// between them is only spaces, tabs and line breaks.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Token {
    pub range: Range<usize>,
    pub kind: Kind,
}

/// A language known to this crate.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Language {
    /// Identifier passed to [`tokens`] and [`Highlighter::new`].
    pub id: &'static str,
    /// Name shown to the user.
    pub name: &'static str,
    /// File extensions, lowercase, without the dot.
    pub extensions: &'static [&'static str],
}

/// The languages this crate highlights.
pub const LANGUAGES: &[Language] = &[Language { id: "nexul", name: "NEXUL", extensions: &["nx"] }];

/// The language of a file, from its extension (case-insensitive).
pub fn language_for_path(path: &Path) -> Option<&'static Language> {
    let ext = path.extension()?.to_str()?;
    LANGUAGES.iter().find(|l| l.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
}

/// Is `lang` (a [`Language::id`], case-insensitive) highlighted by this crate?
pub fn is_supported(lang: &str) -> bool {
    Lang::from_id(lang).is_some()
}

/// Tokenizes a whole text. An unknown language gives no token (plain text).
pub fn tokens(lang: &str, text: &str) -> Vec<Token> {
    let Some(lang) = Lang::from_id(lang) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(text.len() / 4);
    let mut state = LineState::default();
    let mut base = 0;
    for (i, line) in text.split('\n').enumerate() {
        state = lang.line(line, state, base, i == 0, &mut out);
        base += line.len() + 1;
    }
    out
}

/// State of the tokenizer at a line boundary. It is opaque; the default value is the state
/// at the start of a file. A line tokenized from two equal states gives the same tokens,
/// which is what lets an editor stop re-tokenizing after an edit.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LineState(u32);

/// Tokenizes one line given the state at its start; returns its tokens (ranges relative to
/// the start of the line) and the state at its end. `line` must not contain `'\n'` (what
/// follows one is ignored); a final `'\r'` is ignored. A byte order mark at the start of a
/// file is not handled here: [`tokens`] and [`Highlighter`] skip it on the first line.
pub fn tokenize_line(lang: &str, line: &str, state: LineState) -> (Vec<Token>, LineState) {
    let mut out = Vec::new();
    let line = line.split('\n').next().unwrap_or("");
    match Lang::from_id(lang) {
        Some(lang) => {
            let end = lang.line(line, state, 0, false, &mut out);
            (out, end)
        }
        None => (out, state),
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Lang {
    Nexul,
}

impl Lang {
    fn from_id(id: &str) -> Option<Lang> {
        if id.eq_ignore_ascii_case("nexul") || id.eq_ignore_ascii_case("nx") {
            Some(Lang::Nexul)
        } else {
            None
        }
    }

    fn id(self) -> &'static str {
        match self {
            Lang::Nexul => "nexul",
        }
    }

    /// Appends the tokens of `line` (without its `'\n'`), shifted by `base`.
    fn line(self, line: &str, state: LineState, base: usize, first_line: bool, out: &mut Vec<Token>) -> LineState {
        match self {
            Lang::Nexul => LineState(nexul::tokenize(line, state.0, base, first_line, out)),
        }
    }
}

/// Incremental highlighting of a document, line by line.
///
/// The highlighter keeps a copy of the text of each line, its tokens and the tokenizer
/// state at its start and end. After an edit, lines are re-tokenized from the first edited
/// one until the state at the start of an unedited line is unchanged: typing in a line
/// re-tokenizes that line only, while opening a block comment re-tokenizes what follows.
#[derive(Clone, Debug)]
pub struct Highlighter {
    lang: Option<Lang>,
    lines: Vec<Line>,
}

#[derive(Clone, Debug)]
struct Line {
    text: String,
    start: LineState,
    end: LineState,
    /// Ranges relative to the start of the line.
    tokens: Vec<Token>,
}

impl Highlighter {
    /// Highlights `text` in language `lang` (no token for an unknown language).
    pub fn new(lang: &str, text: &str) -> Highlighter {
        let mut h = Highlighter { lang: Lang::from_id(lang), lines: Vec::new() };
        h.set_text(text);
        h
    }

    /// Replaces the whole text.
    pub fn set_text(&mut self, text: &str) {
        self.lines.clear();
        let new: Vec<&str> = text.split('\n').collect();
        self.edit_lines(0, 0, &new);
    }

    /// The language, if it is supported.
    pub fn language(&self) -> Option<&'static str> {
        self.lang.map(Lang::id)
    }

    /// Number of lines (a text without `'\n'` has one line).
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Text of a line, without its `'\n'` (empty past the end).
    pub fn line_text(&self, line: usize) -> &str {
        self.lines.get(line).map_or("", |l| l.text.as_str())
    }

    /// Tokens of a line, ranges relative to the start of the line (empty past the end).
    pub fn line_tokens(&self, line: usize) -> &[Token] {
        self.lines.get(line).map_or(&[], |l| l.tokens.as_slice())
    }

    /// The whole text, lines joined by `'\n'`.
    pub fn text(&self) -> String {
        let mut s = String::with_capacity(self.lines.iter().map(|l| l.text.len() + 1).sum());
        for (i, l) in self.lines.iter().enumerate() {
            if i > 0 {
                s.push('\n');
            }
            s.push_str(&l.text);
        }
        s
    }

    /// Every token of the document, with ranges in bytes from the start of the text:
    /// the same result as [`tokens`] on [`Highlighter::text`].
    pub fn tokens(&self) -> Vec<Token> {
        let mut out = Vec::new();
        let mut base = 0;
        for l in &self.lines {
            out.extend(
                l.tokens.iter().map(|t| Token { range: t.range.start + base..t.range.end + base, kind: t.kind }),
            );
            base += l.text.len() + 1;
        }
        out
    }

    /// Replaces `removed` lines starting at line `first` with `new_lines` (each without
    /// `'\n'`). Returns the lines whose tokens were recomputed, numbered after the edit.
    /// Out-of-range values are clamped.
    pub fn edit_lines(&mut self, first: usize, removed: usize, new_lines: &[&str]) -> Range<usize> {
        let first = first.min(self.lines.len());
        let removed = removed.min(self.lines.len() - first);
        let fresh = new_lines.iter().map(|t| Line {
            text: t.to_string(),
            start: LineState::default(),
            end: LineState::default(),
            tokens: Vec::new(),
        });
        self.lines.splice(first..first + removed, fresh);
        let edited_end = first + new_lines.len();
        let mut state = if first == 0 { LineState::default() } else { self.lines[first - 1].end };
        let mut i = first;
        while i < self.lines.len() {
            if i >= edited_end && self.lines[i].start == state {
                break;
            }
            let line = &mut self.lines[i];
            line.start = state;
            line.tokens.clear();
            line.end = match self.lang {
                Some(lang) => lang.line(&line.text, state, 0, i == 0, &mut line.tokens),
                None => state,
            };
            state = line.end;
            i += 1;
        }
        first..i
    }

    /// Replaces the bytes `range` of the text (as returned by [`Highlighter::text`]) with
    /// `new_text`, as an editor does. Returns the lines whose tokens were recomputed, or
    /// `None`, leaving the highlighter unchanged, if `range` is not a valid range of the
    /// text on character boundaries.
    pub fn replace(&mut self, range: Range<usize>, new_text: &str) -> Option<Range<usize>> {
        if range.start > range.end {
            return None;
        }
        let (l0, c0) = self.position(range.start)?;
        let (l1, c1) = self.position(range.end)?;
        let mut joined = String::with_capacity(c0 + new_text.len() + self.lines[l1].text.len() - c1);
        joined.push_str(&self.lines[l0].text[..c0]);
        joined.push_str(new_text);
        joined.push_str(&self.lines[l1].text[c1..]);
        let new: Vec<&str> = joined.split('\n').collect();
        Some(self.edit_lines(l0, l1 - l0 + 1, &new))
    }

    /// Line and column (in bytes) of a byte offset of the text.
    fn position(&self, offset: usize) -> Option<(usize, usize)> {
        let mut base = 0;
        for (i, l) in self.lines.iter().enumerate() {
            if offset <= base + l.text.len() {
                let col = offset - base;
                return l.text.is_char_boundary(col).then_some((i, col));
            }
            base += l.text.len() + 1;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_names_round_trip() {
        for k in Kind::ALL {
            assert_eq!(Kind::from_name(k.name()), Some(k));
        }
        assert_eq!(Kind::from_name("nothing"), None);
    }

    #[test]
    fn languages() {
        assert_eq!(language_for_path(Path::new("a/b/main.nx")).map(|l| l.id), Some("nexul"));
        assert_eq!(language_for_path(Path::new("MAIN.NX")).map(|l| l.id), Some("nexul"));
        assert_eq!(language_for_path(Path::new("main.rs")), None);
        assert_eq!(language_for_path(Path::new("nx")), None);
        assert!(is_supported("nexul") && is_supported("NEXUL") && is_supported("nx"));
        assert!(!is_supported("rust"));
        assert!(tokens("rust", "fn main() {}").is_empty());
    }

    #[test]
    fn line_api_carries_block_comments() {
        let (t, s) = tokenize_line("nexul", "let a = 1 /* open", LineState::default());
        assert_eq!(t.last().map(|t| t.kind), Some(Kind::Comment));
        assert_ne!(s, LineState::default());
        let (t, s2) = tokenize_line("nexul", "still */ x", s);
        assert_eq!(t[0], Token { range: 0..8, kind: Kind::Comment });
        assert_eq!(t[1], Token { range: 9..10, kind: Kind::Variable });
        assert_eq!(s2, LineState::default());
        // CRLF: the final carriage return is not part of any token.
        let (t, _) = tokenize_line("nexul", "x // c\r", LineState::default());
        assert_eq!(t[1].range, 2..6);
    }

    #[test]
    fn highlighter_edits() {
        let mut h = Highlighter::new("nexul", "let a = 1\nlet b = 2\nlet c = 3");
        assert_eq!(h.line_count(), 3);
        assert_eq!(h.tokens(), tokens("nexul", &h.text()));
        // Typing in a line re-tokenizes that line only.
        assert_eq!(h.replace(14..15, "bb"), Some(1..2));
        assert_eq!(h.text(), "let a = 1\nlet bb = 2\nlet c = 3");
        // Opening a block comment re-tokenizes up to the end...
        assert_eq!(h.replace(0..0, "/* "), Some(0..3));
        assert!(h.line_tokens(2).iter().all(|t| t.kind == Kind::Comment));
        // ...and closing it too.
        assert_eq!(h.replace(13..13, " */"), Some(1..3));
        assert_eq!(h.line_tokens(2)[0].kind, Kind::Keyword);
        assert_eq!(h.tokens(), tokens("nexul", &h.text()));
        // Splitting and joining lines.
        let r = h.replace(10..10, "x\n").unwrap();
        assert_eq!(r, 0..2);
        assert_eq!(h.line_count(), 4);
        assert_eq!(h.tokens(), tokens("nexul", &h.text()));
        assert!(h.replace(Range { start: 5, end: 3 }, "").is_none());
        assert!(h.replace(0..10_000, "").is_none());
        // Unknown language: lines are kept, no token.
        let mut p = Highlighter::new("text", "a\nb");
        assert_eq!(p.language(), None);
        assert_eq!(p.replace(1..2, ""), Some(0..1));
        assert_eq!(p.text(), "ab");
        assert!(p.tokens().is_empty());
    }

    #[test]
    fn byte_order_mark_on_the_first_line_only() {
        let t = tokens("nexul", "\u{FEFF}let x\n\u{FEFF}");
        assert_eq!(t[0], Token { range: 3..6, kind: Kind::Keyword });
        assert_eq!(t.last().unwrap().kind, Kind::Invalid);
    }
}

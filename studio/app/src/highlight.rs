//! Colors of NEXUL code in the editor, and the layer on which the editor draws the
//! diagnostics (the editor shows them only for a language that has a highlighter).
//!
//! A light lexical scan, temporary: the tokenizer of NEXUL of `studio/syntax` (Claude
//! Outils) will replace `tokens` when it is ready.

use gpui_kit::component::input::{EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter, InputHighlighterFactory, Rope};
use gpui_kit::{Context, HighlightStyle, SharedString, Window};
use std::ops::Range;
use std::rc::Rc;

const KEYWORDS: &[&str] = &[
    "and", "as", "break", "const", "continue", "dyn", "else", "enum", "extern", "fn", "for", "if", "impl", "import", "in", "indirect", "inout", "let", "match", "module", "move", "not",
    "or", "owned", "pub", "return", "static", "struct", "trait", "type", "unsafe", "var", "where", "while",
];
const TYPES: &[&str] = &["i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize", "f32", "f64", "bool", "char", "Never", "Ptr"];

/// The colored spans of a piece of NEXUL code: byte ranges and names of the theme
/// (`keyword`, `string`…), in order, without overlap.
pub fn tokens(src: &str) -> Vec<(Range<usize>, &'static str)> {
    let b = src.as_bytes();
    let n = b.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let c = b[i];
        let start = i;
        if c == b'/' && i + 1 < n && b[i + 1] == b'/' {
            let doc = i + 2 < n && b[i + 2] == b'/';
            while i < n && b[i] != b'\n' {
                i += 1;
            }
            out.push((start..i, if doc { "comment.doc" } else { "comment" }));
        } else if c == b'/' && i + 1 < n && b[i + 1] == b'*' {
            let mut depth = 0;
            while i < n {
                if b[i] == b'/' && i + 1 < n && b[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && i + 1 < n && b[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            out.push((start..i, "comment"));
        } else if c == b'"' || (c == b'r' && i + 1 < n && b[i + 1] == b'"') {
            let raw = c == b'r';
            i += if raw { 2 } else { 1 };
            while i < n && b[i] != b'"' {
                i += if b[i] == b'\\' && !raw { 2 } else { 1 };
            }
            i = (i + 1).min(n);
            out.push((start..i, "string"));
        } else if c == b'\'' {
            // A character: 'a', '\n', '\u{e9}', 'é'.
            let mut j = i + 1;
            if j < n && b[j] == b'\\' {
                j += 2;
            }
            while j < n && j - i < 12 && b[j] != b'\'' && b[j] != b'\n' {
                j += 1;
            }
            if j < n && b[j] == b'\'' {
                i = j + 1;
                out.push((start..i, "string"));
            } else {
                i += 1;
            }
        } else if c.is_ascii_digit() {
            while i < n && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || (b[i] == b'.' && i + 1 < n && b[i + 1].is_ascii_digit())) {
                i += 1;
            }
            out.push((start..i, "number"));
        } else if c == b'@' && i + 1 < n && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'_') {
            i += 1;
            while i < n && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push((start..i, "attribute"));
        } else if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
            while i < n && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] >= 0x80) {
                i += 1;
            }
            let word = &src[start..i];
            let next = src[i..].trim_start_matches([' ', '\t']).as_bytes().first().copied();
            let kind = if KEYWORDS.contains(&word) {
                Some("keyword")
            } else if word == "true" || word == "false" {
                Some("boolean")
            } else if word == "self" || word == "Self" {
                Some("variable.special")
            } else if TYPES.contains(&word) {
                Some("type")
            } else if next == Some(b'(') {
                Some("function")
            } else if word.len() > 1 && word.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_') {
                Some("constant")
            } else if word.as_bytes()[0].is_ascii_uppercase() {
                Some("type")
            } else {
                None
            };
            if let Some(kind) = kind {
                out.push((start..i, kind));
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The highlighter of an editor of NEXUL code.
#[derive(Default)]
pub struct NexulHighlighter {
    text: Rope,
}

impl InputHighlighter for NexulHighlighter {
    fn language(&self) -> SharedString {
        "nexul".into()
    }

    fn update(&mut self, _: Option<InputEdit>, text: &Rope, _: bool, _: &mut Window, _: &mut Context<EditorState>) {
        self.text = text.clone();
    }

    fn styles(&self, range: &Range<usize>, resolver: &dyn HighlightStyleResolver) -> Vec<(Range<usize>, HighlightStyle)> {
        let end = range.end.min(self.text.len());
        let start = range.start.min(end);
        let src = self.text.slice(start..end).to_string();
        let mut runs = Vec::new();
        let mut at = start;
        for (r, name) in tokens(&src) {
            let (a, b) = (start + r.start, start + r.end);
            if a > at {
                runs.push((at..a, HighlightStyle::default()));
            }
            runs.push((a..b, resolver.style(name).unwrap_or_default()));
            at = b;
        }
        if at < range.end {
            runs.push((at..range.end, HighlightStyle::default()));
        }
        runs
    }

    fn fold_ranges(&self, _: &Rope) -> Vec<FoldRange> {
        Vec::new()
    }
}

/// For the editors of NEXUL files.
pub fn factory() -> InputHighlighterFactory {
    Rc::new(|language: &str| (language == "nexul").then(|| Box::new(NexulHighlighter::default()) as Box<dyn InputHighlighter>))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use core::prelude::v1::test;

    fn kinds(src: &str) -> Vec<(&str, &'static str)> {
        tokens(src).into_iter().map(|(r, k)| (&src[r], k)).collect()
    }

    #[test]
    fn nexul_tokens() {
        assert_eq!(
            kinds("fn carré(x: i32) -> Point { return x * 2 } // fin"),
            vec![("fn", "keyword"), ("carré", "function"), ("i32", "type"), ("Point", "type"), ("return", "keyword"), ("2", "number"), ("// fin", "comment")]
        );
        assert_eq!(kinds(r#"let s = "a \"b\" {x}" + r"c\d""#), vec![("let", "keyword"), (r#""a \"b\" {x}""#, "string"), (r#"r"c\d""#, "string")]);
        assert_eq!(kinds("@test fn t() { assert(true) }"), vec![("@test", "attribute"), ("fn", "keyword"), ("t", "function"), ("assert", "function"), ("true", "boolean")]);
        assert_eq!(kinds("/* a /* b */ c */ MAX 'é' 0x1F 1.5"), vec![("/* a /* b */ c */", "comment"), ("MAX", "constant"), ("'é'", "string"), ("0x1F", "number"), ("1.5", "number")]);
        // Unfinished pieces stop at the end.
        assert_eq!(kinds("\"abc"), vec![("\"abc", "string")]);
        assert_eq!(kinds("/* abc"), vec![("/* abc", "comment")]);
    }
}

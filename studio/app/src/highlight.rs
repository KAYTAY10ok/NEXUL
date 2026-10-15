//! Colors of NEXUL code in the editor, and the layer on which the editor draws the
//! diagnostics (the editor shows them only for a language that has a highlighter).
//!
//! The tokens come from `studio/syntax` (the lexer of NEXUL, kept in step with the one of
//! the compiler); its incremental highlighter re-tokenizes only the lines an edit touches.

use gpui_kit::component::input::{EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter, InputHighlighterFactory, Rope};
use gpui_kit::{Context, HighlightStyle, SharedString, Window};
use nexul_studio_syntax::{Highlighter, Kind};
use std::ops::Range;
use std::rc::Rc;

/// The name of the style of the theme for a token (`None`: the color of plain text).
fn style_name(kind: Kind, text: &str) -> Option<&'static str> {
    Some(match kind {
        Kind::Keyword if text == "self" => "variable.special",
        Kind::Keyword | Kind::Control => "keyword",
        Kind::Type => "type",
        Kind::Function => "function",
        Kind::Constant if text == "true" || text == "false" => "boolean",
        Kind::Constant => "constant",
        Kind::String => "string",
        Kind::Escape => "string.escape",
        Kind::Interpolation => "punctuation.special",
        Kind::Number => "number",
        Kind::Comment => "comment",
        Kind::DocComment => "comment.doc",
        Kind::Attribute => "attribute",
        Kind::Operator => "operator",
        Kind::Punctuation if matches!(text, "(" | ")" | "[" | "]" | "{" | "}") => "punctuation.bracket",
        Kind::Punctuation => "punctuation.delimiter",
        Kind::Variable | Kind::Invalid => return None,
    })
}

/// The colored spans of a piece of NEXUL code: byte ranges and names of the theme
/// (`keyword`, `string`…), in order, without overlap.
pub fn tokens(src: &str) -> Vec<(Range<usize>, &'static str)> {
    nexul_studio_syntax::tokens("nexul", src).into_iter().filter_map(|t| style_name(t.kind, &src[t.range.clone()]).map(|n| (t.range, n))).collect()
}

/// The highlighter of an editor of NEXUL code.
pub struct NexulHighlighter {
    lines: Highlighter,
    /// Byte offset of the start of each line, and the length of the text.
    starts: Vec<usize>,
    len: usize,
}

impl Default for NexulHighlighter {
    fn default() -> Self {
        NexulHighlighter { lines: Highlighter::new("nexul", ""), starts: vec![0], len: 0 }
    }
}

impl NexulHighlighter {
    fn index_lines(&mut self) {
        self.starts.clear();
        let mut at = 0;
        for i in 0..self.lines.line_count() {
            self.starts.push(at);
            at += self.lines.line_text(i).len() + 1;
        }
        self.len = at.saturating_sub(1);
    }

    fn set_text(&mut self, text: &Rope) {
        self.lines.set_text(&text.to_string());
        self.index_lines();
    }
}

impl InputHighlighter for NexulHighlighter {
    fn language(&self) -> SharedString {
        "nexul".into()
    }

    fn update(&mut self, edit: Option<InputEdit>, text: &Rope, _: bool, _: &mut Window, _: &mut Context<EditorState>) {
        self.apply(edit.map(|e| (e.start_byte, e.old_end_byte, e.new_end_byte)), text);
    }

    fn styles(&self, range: &Range<usize>, resolver: &dyn HighlightStyleResolver) -> Vec<(Range<usize>, HighlightStyle)> {
        let mut runs = Vec::new();
        let mut at = range.start;
        for (r, name) in self.spans(range) {
            if r.start > at {
                runs.push((at..r.start, HighlightStyle::default()));
            }
            at = r.end;
            runs.push((r, resolver.style(name).unwrap_or_default()));
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

impl NexulHighlighter {
    /// Follows an edit of the text: `(start, old_end, new_end)` replaced `start..old_end`
    /// of the previous text with `start..new_end` of `text`; `None`, a new text.
    fn apply(&mut self, edit: Option<(usize, usize, usize)>, text: &Rope) {
        let Some((start, old_end, new_end)) = edit else { return self.set_text(text) };
        // Anything that does not add up means the copy is out of step: start over.
        let fits = start <= old_end && old_end <= self.len && start <= new_end && new_end <= text.len() && self.len - (old_end - start) + (new_end - start) == text.len();
        if fits && self.lines.replace(start..old_end, &text.slice(start..new_end).to_string()).is_some() {
            self.index_lines();
        } else {
            self.set_text(text);
        }
    }

    /// The colored spans that meet `range`, cut to it, in order.
    fn spans(&self, range: &Range<usize>) -> Vec<(Range<usize>, &'static str)> {
        let end = range.end.min(self.len);
        let mut runs = Vec::new();
        let at = range.start;
        // The first line that ends at or after the start of the range.
        let first = self.starts.partition_point(|&s| s <= range.start).saturating_sub(1);
        for line in first..self.lines.line_count() {
            let base = self.starts[line];
            if base >= end {
                break;
            }
            let text = self.lines.line_text(line);
            for t in self.lines.line_tokens(line) {
                let (a, b) = (base + t.range.start, base + t.range.end);
                if b <= at || a >= end {
                    continue;
                }
                let Some(name) = style_name(t.kind, &text[t.range.clone()]) else { continue };
                runs.push((a.max(at)..b.min(end), name));
            }
        }
        runs
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
        tokens(src).into_iter().filter(|(_, k)| !k.starts_with("punctuation") && *k != "operator").map(|(r, k)| (&src[r], k)).collect()
    }

    #[test]
    fn nexul_tokens() {
        assert_eq!(
            kinds("fn carre(x: i32) -> Point { return x * 2 } // fin"),
            vec![("fn", "keyword"), ("carre", "function"), ("i32", "type"), ("Point", "type"), ("return", "keyword"), ("2", "number"), ("// fin", "comment")]
        );
        assert_eq!(kinds(r#"let s = "a \"b\"" + "\n""#), vec![("let", "keyword"), ("\"a ", "string"), ("\\\"", "string.escape"), ("b", "string"), ("\\\"", "string.escape"), ("\"", "string"), ("\"", "string"), ("\\n", "string.escape"), ("\"", "string")]);
        assert_eq!(kinds("@test fn t() { assert(true) }"), vec![("@test", "attribute"), ("fn", "keyword"), ("t", "function"), ("assert", "function"), ("true", "boolean")]);
        assert_eq!(kinds("/* a /* b */ c */ MAX 'é' 0x1F 1.5 /// doc"), vec![("/* a /* b */ c */", "comment"), ("MAX", "constant"), ("'é'", "string"), ("0x1F", "number"), ("1.5", "number"), ("/// doc", "comment.doc")]);
        // Unfinished pieces stop at the end.
        assert_eq!(kinds("/* abc"), vec![("/* abc", "comment")]);
    }

    #[test]
    fn edits_follow_the_text() {
        let mut text = String::from("fn main() {\r\n    let x = 1\r\n}\n");
        let mut h = NexulHighlighter::default();
        h.apply(None, &Rope::from(text.as_str()));
        assert_eq!(h.spans(&(0..text.len())), tokens(&text));
        // Typing, a comment that opens and closes over several lines, several lines at
        // once, deletions.
        for (old, new) in [("let", "var"), ("fn", "/* fn"), ("{", "{ */"), ("1", "\"é\" // fin\n    x = 2"), ("/* ", ""), ("x = 2", "")] {
            let start = text.find(old).unwrap();
            let old_end = start + old.len();
            text.replace_range(start..old_end, new);
            h.apply(Some((start, old_end, start + new.len())), &Rope::from(text.as_str()));
            assert_eq!(h.spans(&(0..text.len())), tokens(&text), "after {old:?} -> {new:?}");
        }
        // Part of the text: the spans are cut to it.
        let part = 5..16;
        let cut: Vec<_> = tokens(&text).into_iter().filter(|(r, _)| r.end > part.start && r.start < part.end).map(|(r, n)| (r.start.max(part.start)..r.end.min(part.end), n)).collect();
        assert_eq!(h.spans(&part), cut);
        // An edit that does not match the copy: highlighted again from the whole text.
        h.apply(Some((0, 999, 3)), &Rope::from("let"));
        assert_eq!(h.spans(&(0..3)), vec![(0..3, "keyword")]);
    }
}

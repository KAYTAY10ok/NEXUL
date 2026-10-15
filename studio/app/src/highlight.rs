//! GPUI adapter for the compiler-conformant lexer in `studio/syntax`.

use gpui_kit::component::input::{
    EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
    InputHighlighterFactory, Rope, RopeExt,
};
use gpui_kit::{Context, HighlightStyle, SharedString, Window};
use nexul_studio_syntax::{Highlighter as SyntaxHighlighter, Kind};
use std::ops::Range;
use std::rc::Rc;

fn style_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Keyword | Kind::Control => "keyword",
        Kind::Type => "type",
        Kind::Function => "function",
        Kind::Variable => "variable",
        Kind::Constant => "constant",
        Kind::String => "string",
        Kind::Escape => "string.escape",
        Kind::Interpolation => "string.interpolation",
        Kind::Number => "number",
        Kind::Comment => "comment",
        Kind::DocComment => "comment.doc",
        Kind::Attribute => "attribute",
        Kind::Operator => "operator",
        Kind::Punctuation => "punctuation",
        Kind::Invalid => "invalid",
    }
}

/// Colored spans of NEXUL source, using UTF-8 byte ranges and theme style names.
#[cfg(test)]
pub fn tokens(src: &str) -> Vec<(Range<usize>, &'static str)> {
    nexul_studio_syntax::tokens("nexul", src)
        .into_iter()
        .map(|token| (token.range, style_name(token.kind)))
        .collect()
}

/// GPUI highlighter backed by the incremental, compiler-conformant NEXUL tokenizer.
pub struct NexulHighlighter {
    text: Rope,
    syntax: Option<SyntaxHighlighter>,
}

impl Default for NexulHighlighter {
    fn default() -> Self {
        Self {
            text: Rope::new(),
            syntax: None,
        }
    }
}

impl InputHighlighter for NexulHighlighter {
    fn language(&self) -> SharedString {
        "nexul".into()
    }

    fn update(
        &mut self,
        edit: Option<InputEdit>,
        text: &Rope,
        _: bool,
        _: &mut Window,
        _: &mut Context<EditorState>,
    ) {
        match (&mut self.syntax, edit) {
            (None, _) => self.syntax = Some(SyntaxHighlighter::new("nexul", &text.to_string())),
            (Some(syntax), Some(edit)) => {
                let range_valid = edit.start_byte <= edit.new_end_byte
                    && edit.new_end_byte <= text.len()
                    && text.is_char_boundary(edit.start_byte)
                    && text.is_char_boundary(edit.new_end_byte);
                let replacement = if range_valid {
                    text.slice(edit.start_byte..edit.new_end_byte).to_string()
                } else {
                    String::new()
                };
                if !range_valid
                    || syntax
                        .replace(edit.start_byte..edit.old_end_byte, &replacement)
                        .is_none()
                {
                    syntax.set_text(&text.to_string());
                }
            }
            (Some(syntax), None) => syntax.set_text(&text.to_string()),
        }
        self.text = text.clone();
    }

    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let end = range.end.min(self.text.len());
        let start = range.start.min(end);
        if start == end {
            return Vec::new();
        }
        let Some(syntax) = &self.syntax else {
            return vec![(start..end, HighlightStyle::default())];
        };

        let first_line = self.text.offset_to_point(start).row;
        let last_line = self.text.offset_to_point(end - 1).row;
        let mut runs = Vec::new();
        let mut at = start;
        for line in first_line..=last_line {
            let line_start = self.text.line_start_offset(line);
            for token in syntax.line_tokens(line) {
                let token_start = (line_start + token.range.start).max(start);
                let token_end = (line_start + token.range.end).min(end);
                if token_start >= token_end {
                    continue;
                }
                if token_start > at {
                    runs.push((at..token_start, HighlightStyle::default()));
                }
                runs.push((
                    token_start..token_end,
                    resolver.style(style_name(token.kind)).unwrap_or_default(),
                ));
                at = token_end;
            }
        }
        if at < end {
            runs.push((at..end, HighlightStyle::default()));
        }
        runs
    }

    fn fold_ranges(&self, _: &Rope) -> Vec<FoldRange> {
        Vec::new()
    }
}

/// Factory for NEXUL source files.
pub fn factory() -> InputHighlighterFactory {
    Rc::new(|language: &str| {
        (language == "nexul")
            .then(|| Box::new(NexulHighlighter::default()) as Box<dyn InputHighlighter>)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use core::prelude::v1::test;

    fn styled_text<'a>(src: &'a str) -> Vec<(&'a str, &'static str)> {
        tokens(src)
            .into_iter()
            .map(|(range, style)| (&src[range], style))
            .collect()
    }

    #[test]
    fn uses_the_official_nexul_lexer_classes() {
        let spans = styled_text("/// docs\npub fn main() { let answer: u64 = 42; return answer }");
        assert!(spans.contains(&("/// docs", "comment.doc")));
        assert!(spans.contains(&("pub", "keyword")));
        assert!(spans.contains(&("main", "function")));
        assert!(spans.contains(&("u64", "type")));
        assert!(spans.contains(&("42", "number")));
        assert!(spans.contains(&("return", "keyword")));
    }

    #[test]
    fn classifies_interpolations_escapes_and_invalid_source() {
        let source = r#"let label = "value {answer}\n";"#;
        let spans = styled_text(source);
        assert!(spans.contains(&("{", "string.interpolation")));
        assert!(spans.contains(&("answer", "variable")));
        assert!(spans.contains(&("}", "string.interpolation")));
        assert!(spans.contains(&("\\n", "string.escape")));
        assert!(styled_text("let café = 1")
            .iter()
            .any(|(_, style)| *style == "invalid"));
    }

    #[test]
    fn token_ranges_are_utf8_byte_ranges() {
        let source = "let label = \"é\"; let answer = 42";
        let spans = tokens(source);
        for (range, _) in &spans {
            assert!(source.is_char_boundary(range.start));
            assert!(source.is_char_boundary(range.end));
        }
        let answer = spans
            .iter()
            .find(|(range, _)| &source[range.clone()] == "answer")
            .unwrap();
        assert_eq!(&source[answer.0.clone()], "answer");
        assert!(answer.0.start > source.find('é').unwrap());
    }
}

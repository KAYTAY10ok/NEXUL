//! NEXUL syntax layer: source management, diagnostics, lexer, parser and AST.

pub mod ast;
pub mod diag;
pub mod lexer;
pub mod parser;
pub mod source;
pub mod token;

pub use diag::{DiagSink, Diagnostic, Level};
pub use source::{Edition, FileId, SourceMap, Span};

/// The edition a file declares on one of its first three lines (`// edition: 1`,
/// `// edition: 0.2`), after an optional UTF-8 byte order mark (ADR 0010).
pub fn header_edition(text: &str) -> Option<Edition> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    text.lines().take(3).find_map(|l| l.trim().strip_prefix("// edition:").and_then(|e| Edition::parse(e.trim())))
}

/// Lexes and parses one file.
pub fn parse_file(sm: &SourceMap, file: FileId, diags: &DiagSink) -> ast::Module {
    let text = &sm.get(file).text;
    let tokens = lexer::lex_in(text, file, diags, sm.get(file).edition);
    parser::parse_module_in(tokens, diags, sm.get(file).edition)
}

//! NEXUL syntax layer: source management, diagnostics, lexer, parser and AST.

pub mod ast;
pub mod diag;
pub mod lexer;
pub mod parser;
pub mod source;
pub mod token;

pub use diag::{DiagSink, Diagnostic, Level};
pub use source::{FileId, SourceMap, Span};

/// Lexes and parses one file.
pub fn parse_file(sm: &SourceMap, file: FileId, diags: &DiagSink) -> ast::Module {
    let text = &sm.get(file).text;
    let tokens = lexer::lex(text, file, diags);
    parser::parse_module(tokens, diags)
}

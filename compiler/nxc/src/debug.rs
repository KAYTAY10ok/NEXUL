//! `nxc debug tokens --json <file.nx>`: the tokens of a file, as the lexer of `nxc` reads
//! them, and the diagnostics of the lexer. A hidden command: the reference against which
//! the lexer written in NEXUL is compared (self-hosting, step SH2).
//!
//! Format, one JSON value per line:
//! - first `{"edition":"1"}` (or `"0.2"`), the edition read;
//! - then one line per token: `{"k":"Ident","v":"x","lo":0,"hi":1,"ls":true}`. `k` is the
//!   name of the kind; `v` is the value of an identifier (a string), of an integer (its
//!   decimal digits, a string), of a float (its IEEE 754 bits, `"0x…"`) or of a character
//!   (its code point, a number). A string has `"parts"`: `{"t":"text"}` or
//!   `{"i":[tokens],"lo":…,"hi":…,"spec":null|{…}}`. `ls` says whether the token starts
//!   a line;
//! - then one line per diagnostic, in the order `nxc` reports them:
//!   `{"diag":"E0006","msg":"…","lo":…,"hi":…}`.

use crate::json_str;
use nxc_syntax::token::{Align, FormatKind, FormatSpec, StrPart, Token, TokenKind};
use nxc_syntax::{DiagSink, Edition, SourceMap};
use std::fmt::Write;

fn kind_name(k: &TokenKind) -> String {
    let d = format!("{:?}", k);
    match d.find('(') {
        Some(p) => d[..p].to_string(),
        None => d,
    }
}

fn spec_json(s: &FormatSpec) -> String {
    let opt = |v: Option<u32>| v.map(|v| v.to_string()).unwrap_or_else(|| "null".into());
    let align = match s.align {
        None => "null",
        Some(Align::Left) => "\"<\"",
        Some(Align::Center) => "\"^\"",
        Some(Align::Right) => "\">\"",
    };
    let kind = match s.kind {
        None => "null",
        Some(FormatKind::LowerHex) => "\"x\"",
        Some(FormatKind::UpperHex) => "\"X\"",
        Some(FormatKind::Octal) => "\"o\"",
        Some(FormatKind::Binary) => "\"b\"",
        Some(FormatKind::Exponent) => "\"e\"",
    };
    format!(
        "{{\"fill\":{},\"align\":{},\"plus\":{},\"zero\":{},\"width\":{},\"prec\":{},\"kind\":{},\"lo\":{},\"hi\":{}}}",
        s.fill as u32,
        align,
        s.plus,
        s.zero,
        opt(s.width),
        opt(s.precision),
        kind,
        s.span.lo,
        s.span.hi
    )
}

fn token_json(t: &Token, out: &mut String) {
    let _ = write!(out, "{{\"k\":\"{}\"", kind_name(&t.kind));
    match &t.kind {
        TokenKind::Ident(n) => {
            let _ = write!(out, ",\"v\":{}", json_str(n));
        }
        TokenKind::Int(v) => {
            let _ = write!(out, ",\"v\":\"{}\"", v);
        }
        TokenKind::Float(v) => {
            let _ = write!(out, ",\"v\":\"0x{:016x}\"", v.to_bits());
        }
        TokenKind::Char(c) => {
            let _ = write!(out, ",\"v\":{}", *c as u32);
        }
        TokenKind::Str(s) => {
            out.push_str(",\"parts\":[");
            for (i, p) in s.parts.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                match p {
                    StrPart::Text(text) => {
                        let _ = write!(out, "{{\"t\":{}}}", json_str(text));
                    }
                    StrPart::Interp { tokens, spec, span } => {
                        out.push_str("{\"i\":[");
                        for (j, tok) in tokens.iter().enumerate() {
                            if j > 0 {
                                out.push(',');
                            }
                            token_json(tok, out);
                        }
                        let spec = spec.as_ref().map(spec_json).unwrap_or_else(|| "null".into());
                        let _ = write!(out, "],\"lo\":{},\"hi\":{},\"spec\":{}}}", span.lo, span.hi, spec);
                    }
                }
            }
            out.push(']');
        }
        _ => {}
    }
    let _ = write!(out, ",\"lo\":{},\"hi\":{},\"ls\":{}}}", t.span.lo, t.span.hi, t.line_start);
}

/// The tokens of `text` in `edition`, and the diagnostics of the lexer.
pub fn tokens_json(text: &str, edition: Edition) -> String {
    let mut sm = SourceMap::new();
    let file = sm.add("input.nx", text);
    let diags = DiagSink::new();
    let tokens = nxc_syntax::lexer::lex_in(text, file, &diags, edition);
    let mut out = format!("{{\"edition\":\"{}\"}}\n", if edition == Edition::V1 { "1" } else { "0.2" });
    for t in &tokens {
        token_json(t, &mut out);
        out.push('\n');
    }
    for d in diags.take_sorted() {
        let (lo, hi) = d.primary_span().map(|s| (s.lo, s.hi)).unwrap_or((0, 0));
        let _ = writeln!(out, "{{\"diag\":\"{}\",\"msg\":{},\"lo\":{},\"hi\":{}}}", d.code, json_str(&d.message), lo, hi);
    }
    out
}

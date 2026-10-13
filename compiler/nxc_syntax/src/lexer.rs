//! The lexer: source text -> tokens, including significant newlines (spec §1.7)
//! and the tokenization of interpolated strings (spec §1.5).

use crate::diag::{DiagSink, Diagnostic};
use crate::source::{FileId, Span};
use crate::token::{keyword, Align, FormatKind, FormatSpec, StrLit, StrPart, Token, TokenKind, RESERVED};

/// Bidirectional control characters forbidden everywhere ("Trojan Source").
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

pub fn lex(text: &str, file: FileId, diags: &DiagSink) -> Vec<Token> {
    // Security pre-pass: report every forbidden bidirectional control character once.
    for (i, c) in text.char_indices() {
        if is_bidi_control(c) {
            diags.emit(
                Diagnostic::error("E0003", format!("forbidden bidirectional control character U+{:04X}", c as u32))
                    .primary(Span::new(file, i as u32, (i + c.len_utf8()) as u32), "invisible character here")
                    .note("these characters can make code display differently from how it is compiled (CVE-2021-42574)")
                    .help("if the character is needed inside a string, write it as an escape such as `\\u{202E}`"),
            );
        }
    }
    let mut lx = Lexer::new(text, file, 0, text.len(), diags, false);
    lx.run();
    lx.tokens
}

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    file: FileId,
    pos: usize,
    end: usize,
    diags: &'a DiagSink,
    tokens: Vec<Token>,
    /// Open delimiters: b'(', b'[' or b'{'.
    delims: Vec<u8>,
    saw_newline: bool,
    /// Inside an interpolation: newlines are never significant.
    nested: bool,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str, file: FileId, start: usize, end: usize, diags: &'a DiagSink, nested: bool) -> Lexer<'a> {
        Lexer {
            src,
            bytes: src.as_bytes(),
            file,
            pos: start,
            end,
            diags,
            tokens: Vec::new(),
            delims: Vec::new(),
            saw_newline: false,
            nested,
        }
    }

    fn span(&self, lo: usize, hi: usize) -> Span {
        Span::new(self.file, lo as u32, hi as u32)
    }

    fn peek(&self) -> u8 {
        if self.pos < self.end {
            self.bytes[self.pos]
        } else {
            0
        }
    }

    fn peek_at(&self, n: usize) -> u8 {
        if self.pos + n < self.end {
            self.bytes[self.pos + n]
        } else {
            0
        }
    }

    fn peek_char(&self) -> Option<char> {
        if self.pos < self.end {
            self.src[self.pos..self.end].chars().next()
        } else {
            None
        }
    }

    fn error(&self, code: &'static str, msg: impl Into<String>, lo: usize, hi: usize, label: impl Into<String>) {
        self.diags.emit(Diagnostic::error(code, msg).primary(self.span(lo, hi), label));
    }

    fn newline_significant(&self) -> bool {
        if self.nested {
            return false;
        }
        match self.delims.last() {
            None | Some(b'{') => {}
            _ => return false,
        }
        match self.tokens.last() {
            Some(t) => t.kind.can_end_statement(),
            None => false,
        }
    }

    fn push(&mut self, kind: TokenKind, lo: usize, hi: usize) {
        let line_start = self.saw_newline;
        if self.saw_newline {
            self.saw_newline = false;
            // Rule 3: a line starting with `.ident` or `.0` continues the previous line.
            let continues = kind == TokenKind::Dot && {
                let c = self.bytes.get(hi).copied().unwrap_or(0);
                c.is_ascii_alphanumeric() || c == b'_'
            };
            if !continues && self.newline_significant() {
                let at = self.tokens.last().map(|t| t.span.hi as usize).unwrap_or(lo);
                self.tokens.push(Token { kind: TokenKind::Newline, span: self.span(at, at), line_start: false });
            }
        }
        self.tokens.push(Token { kind, span: self.span(lo, hi), line_start });
    }

    fn run(&mut self) {
        loop {
            self.skip_trivia();
            if self.pos >= self.end {
                break;
            }
            self.lex_token();
        }
        // A final terminator simplifies the parser (the last statement needs no newline).
        if self.saw_newline || !self.nested {
            if self.newline_significant() {
                let at = self.tokens.last().map(|t| t.span.hi as usize).unwrap_or(self.end);
                self.tokens.push(Token { kind: TokenKind::Newline, span: self.span(at, at), line_start: false });
            }
        }
        self.tokens.push(Token { kind: TokenKind::Eof, span: self.span(self.end, self.end), line_start: self.saw_newline });
    }

    fn skip_trivia(&mut self) {
        while self.pos < self.end {
            match self.peek() {
                b' ' | b'\t' => self.pos += 1,
                b'\n' => {
                    self.pos += 1;
                    self.saw_newline = true;
                }
                b'\r' => {
                    if self.peek_at(1) == b'\n' {
                        self.pos += 2;
                        self.saw_newline = true;
                    } else {
                        self.error("E0002", "carriage return not followed by a line feed", self.pos, self.pos + 1, "lone `\\r`");
                        self.pos += 1;
                    }
                }
                b'/' if self.peek_at(1) == b'/' => {
                    while self.pos < self.end && self.peek() != b'\n' {
                        self.pos += 1;
                    }
                }
                b'/' if self.peek_at(1) == b'*' => self.block_comment(),
                _ => break,
            }
        }
    }

    fn block_comment(&mut self) {
        let start = self.pos;
        self.pos += 2;
        let mut depth = 1;
        while self.pos < self.end {
            if self.peek() == b'/' && self.peek_at(1) == b'*' {
                depth += 1;
                self.pos += 2;
            } else if self.peek() == b'*' && self.peek_at(1) == b'/' {
                depth -= 1;
                self.pos += 2;
                if depth == 0 {
                    return;
                }
            } else {
                if self.peek() == b'\n' {
                    self.saw_newline = true;
                }
                self.pos += 1;
            }
        }
        self.error("E0005", "unterminated block comment", start, start + 2, "comment starts here");
    }

    fn lex_token(&mut self) {
        let lo = self.pos;
        let c = self.peek();
        if c.is_ascii_alphabetic() || c == b'_' {
            // Raw string r"..."
            if c == b'r' && self.peek_at(1) == b'"' {
                self.pos += 1;
                self.string(lo, true);
                return;
            }
            while self.pos < self.end && (self.peek().is_ascii_alphanumeric() || self.peek() == b'_') {
                self.pos += 1;
            }
            let word = &self.src[lo..self.pos];
            if word == "_" {
                // `_` is lexed as an identifier; the parser treats it as the wildcard.
                self.push(TokenKind::Ident("_".into()), lo, self.pos);
                return;
            }
            if let Some(kw) = keyword(word) {
                self.push(kw, lo, self.pos);
            } else {
                if RESERVED.contains(&word) {
                    self.diags.emit(
                        Diagnostic::error("E0010", format!("`{}` is a reserved keyword", word))
                            .primary(self.span(lo, self.pos), "reserved for a future version of NEXUL")
                            .help(format!("choose another name, for example `{}_`", word)),
                    );
                }
                self.push(TokenKind::Ident(word.to_string()), lo, self.pos);
            }
            return;
        }
        if c.is_ascii_digit() {
            self.number(lo);
            return;
        }
        match c {
            b'"' => {
                self.string(lo, false);
                return;
            }
            b'\'' => {
                self.char_lit(lo);
                return;
            }
            _ => {}
        }
        if c >= 0x80 {
            let ch = self.peek_char().unwrap();
            self.pos += ch.len_utf8();
            if !is_bidi_control(ch) {
                let mut d = Diagnostic::error("E0017", format!("unexpected character `{}`", ch))
                    .primary(self.span(lo, self.pos), "not valid here");
                if ch.is_alphabetic() {
                    d = d.note("identifiers are ASCII in NEXUL 0.1 (this prevents look-alike character attacks)");
                }
                self.diags.emit(d);
            }
            return;
        }
        use TokenKind::*;
        let two = (c, self.peek_at(1));
        let three = (c, self.peek_at(1), self.peek_at(2));
        let (kind, len) = match three {
            (b'<', b'<', b'=') => (ShlEq, 3),
            (b'>', b'>', b'=') => (ShrEq, 3),
            (b'+', b'%', b'=') => (PlusWrapEq, 3),
            (b'-', b'%', b'=') => (MinusWrapEq, 3),
            (b'*', b'%', b'=') => (StarWrapEq, 3),
            (b'.', b'.', b'=') => (DotDotEq, 3),
            _ => match two {
                (b'<', b'<') => (Shl, 2),
                (b'>', b'>') => (Shr, 2),
                (b'+', b'%') => (PlusWrap, 2),
                (b'-', b'%') => (MinusWrap, 2),
                (b'*', b'%') => (StarWrap, 2),
                (b'+', b'=') => (PlusEq, 2),
                (b'-', b'=') => (MinusEq, 2),
                (b'*', b'=') => (StarEq, 2),
                (b'/', b'=') => (SlashEq, 2),
                (b'%', b'=') => (PercentEq, 2),
                (b'&', b'=') => (AmpEq, 2),
                (b'|', b'=') => (PipeEq, 2),
                (b'^', b'=') => (CaretEq, 2),
                (b'=', b'=') => (EqEq, 2),
                (b'!', b'=') => (NotEq, 2),
                (b'<', b'=') => (Le, 2),
                (b'>', b'=') => (Ge, 2),
                (b'-', b'>') => (Arrow, 2),
                (b'=', b'>') => (FatArrow, 2),
                (b'.', b'.') => (DotDot, 2),
                _ => match c {
                    b'+' => (Plus, 1),
                    b'-' => (Minus, 1),
                    b'*' => (Star, 1),
                    b'/' => (Slash, 1),
                    b'%' => (Percent, 1),
                    b'&' => (Amp, 1),
                    b'|' => (Pipe, 1),
                    b'^' => (Caret, 1),
                    b'~' => (Tilde, 1),
                    b'=' => (Eq, 1),
                    b'<' => (Lt, 1),
                    b'>' => (Gt, 1),
                    b'(' => (LParen, 1),
                    b')' => (RParen, 1),
                    b'[' => (LBracket, 1),
                    b']' => (RBracket, 1),
                    b'{' => (LBrace, 1),
                    b'}' => (RBrace, 1),
                    b',' => (Comma, 1),
                    b'.' => (Dot, 1),
                    b':' => (Colon, 1),
                    b';' => (Semi, 1),
                    b'?' => (Question, 1),
                    b'@' => (At, 1),
                    _ => {
                        self.pos += 1;
                        let msg = if c == b'!' {
                            self.diags.emit(
                                Diagnostic::error("E0017", "unexpected character `!`")
                                    .primary(self.span(lo, lo + 1), "not an operator in NEXUL")
                                    .help("logical negation is written `not`, and inequality is written `!=`"),
                            );
                            return;
                        } else if c < 0x20 || c == 0x7f {
                            format!("unexpected control character U+{:04X}", c)
                        } else {
                            format!("unexpected character `{}`", c as char)
                        };
                        self.error(if c < 0x20 || c == 0x7f { "E0004" } else { "E0017" }, msg, lo, lo + 1, "not valid here");
                        return;
                    }
                },
            },
        };
        self.pos += len;
        // Push first: a pending newline before a closing delimiter is judged with the
        // delimiter still open.
        let closing = matches!(kind, RParen | RBracket | RBrace);
        let opening = match kind {
            LParen => Some(b'('),
            LBracket => Some(b'['),
            LBrace => Some(b'{'),
            _ => None,
        };
        let want = match kind {
            RParen => b'(',
            RBracket => b'[',
            _ => b'{',
        };
        self.push(kind, lo, self.pos);
        if let Some(d) = opening {
            self.delims.push(d);
        } else if closing {
            if let Some(i) = self.delims.iter().rposition(|&d| d == want) {
                self.delims.truncate(i);
            }
            // Mismatched delimiters are reported by the parser, which knows the context.
        }
    }

    fn number(&mut self, lo: usize) {
        let after_dot = matches!(self.tokens.last(), Some(Token { kind: TokenKind::Dot, .. }));
        let radix = if self.peek() == b'0' {
            match self.peek_at(1) {
                b'x' | b'X' => 16,
                b'o' | b'O' => 8,
                b'b' | b'B' => 2,
                _ => 10,
            }
        } else {
            10
        };
        if radix != 10 {
            self.pos += 2;
            let digits_start = self.pos;
            if self.peek() == b'_' {
                self.error("E0006", "`_` cannot follow a radix prefix", self.pos, self.pos + 1, "remove this `_`");
            }
            let mut value: u128 = 0;
            let mut overflow = false;
            let mut any = false;
            while self.pos < self.end {
                let b = self.peek();
                if b == b'_' {
                    self.pos += 1;
                    continue;
                }
                let d = match (b as char).to_digit(16) {
                    Some(d) => d,
                    None => break,
                };
                if d >= radix {
                    let p = self.pos;
                    self.pos += 1;
                    self.error("E0016", format!("invalid digit `{}` in base {} literal", b as char, radix), p, p + 1, "invalid digit");
                    continue;
                }
                any = true;
                value = value * radix as u128 + d as u128;
                if value > u64::MAX as u128 {
                    overflow = true;
                }
                self.pos += 1;
            }
            if !any {
                self.error("E0016", "missing digits after the radix prefix", lo, self.pos, "expected digits");
            } else if self.bytes[self.pos - 1] == b'_' && self.pos > digits_start {
                self.error("E0006", "a numeric literal cannot end with `_`", self.pos - 1, self.pos, "remove this `_`");
            }
            self.check_suffix();
            if overflow {
                self.error("E0301", "integer literal is too large", lo, self.pos, "does not fit in 64 bits");
                value = 0;
            }
            self.push(TokenKind::Int(value), lo, self.pos);
            return;
        }
        while self.pos < self.end && (self.peek().is_ascii_digit() || self.peek() == b'_') {
            self.pos += 1;
        }
        let mut is_float = false;
        if !after_dot && self.peek() == b'.' && self.peek_at(1).is_ascii_digit() {
            is_float = true;
            self.pos += 1;
            while self.pos < self.end && (self.peek().is_ascii_digit() || self.peek() == b'_') {
                self.pos += 1;
            }
        }
        if !after_dot && (self.peek() == b'e' || self.peek() == b'E') {
            let save = self.pos;
            let mut p = self.pos + 1;
            if p < self.end && (self.bytes[p] == b'+' || self.bytes[p] == b'-') {
                p += 1;
            }
            if p < self.end && self.bytes[p].is_ascii_digit() {
                is_float = true;
                self.pos = p;
                while self.pos < self.end && (self.peek().is_ascii_digit() || self.peek() == b'_') {
                    self.pos += 1;
                }
            } else {
                self.pos = save;
            }
        }
        let text = &self.src[lo..self.pos];
        if text.ends_with('_') {
            self.error("E0006", "a numeric literal cannot end with `_`", self.pos - 1, self.pos, "remove this `_`");
        }
        if text.contains("_.") || text.contains("._") || text.contains("_e") || text.contains("_E") {
            self.error("E0006", "misplaced `_` in numeric literal", lo, self.pos, "`_` must be between digits");
        }
        let clean: String = text.chars().filter(|&c| c != '_').collect();
        self.check_suffix();
        if is_float {
            let v: f64 = clean.parse().unwrap_or(0.0);
            if v.is_infinite() {
                self.error("E0011", "float literal is out of range", lo, self.pos, "too large for `f64`");
            }
            self.push(TokenKind::Float(v), lo, self.pos);
        } else {
            let v = match clean.parse::<u128>() {
                Ok(v) if v <= u64::MAX as u128 => v,
                _ => {
                    self.error("E0301", "integer literal is too large", lo, self.pos, "does not fit in 64 bits");
                    0
                }
            };
            self.push(TokenKind::Int(v), lo, self.pos);
        }
    }

    /// Letters directly after a number (`10px`) are an error rather than two tokens.
    fn check_suffix(&mut self) {
        let start = self.pos;
        if self.pos < self.end && (self.peek().is_ascii_alphabetic() || self.peek() == b'_') {
            while self.pos < self.end && (self.peek().is_ascii_alphanumeric() || self.peek() == b'_') {
                self.pos += 1;
            }
            let suffix = &self.src[start..self.pos];
            let mut d = Diagnostic::error("E0016", format!("invalid suffix `{}` on numeric literal", suffix))
                .primary(self.span(start, self.pos), "unexpected suffix");
            if matches!(suffix, "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "isize" | "usize" | "f32" | "f64") {
                d = d.help(format!("NEXUL has no literal suffixes; write a type annotation or `as {}`", suffix));
            }
            self.diags.emit(d);
        }
    }

    /// Parses one escape sequence after a backslash; returns the character.
    fn escape(&mut self) -> Option<char> {
        let lo = self.pos - 1; // position of the backslash
        if self.pos >= self.end {
            return None;
        }
        let c = self.peek();
        self.pos += 1;
        Some(match c {
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'0' => '\0',
            b'\\' => '\\',
            b'\'' => '\'',
            b'"' => '"',
            b'{' => '{',
            b'}' => '}',
            b'u' => {
                if self.peek() != b'{' {
                    self.error("E0007", "invalid unicode escape", lo, self.pos, "expected `\\u{...}`");
                    return None;
                }
                self.pos += 1;
                let start = self.pos;
                while self.pos < self.end && self.peek().is_ascii_hexdigit() {
                    self.pos += 1;
                }
                let digits = &self.src[start..self.pos];
                if self.peek() != b'}' || digits.is_empty() || digits.len() > 6 {
                    if self.peek() == b'}' {
                        self.pos += 1;
                    }
                    self.error("E0007", "invalid unicode escape", lo, self.pos, "expected 1 to 6 hexadecimal digits in braces");
                    return None;
                }
                self.pos += 1;
                let v = u32::from_str_radix(digits, 16).unwrap();
                match char::from_u32(v) {
                    Some(ch) => ch,
                    None => {
                        self.error("E0007", format!("invalid unicode scalar value U+{:X}", v), lo, self.pos, "surrogates and values above 10FFFF are not characters");
                        return None;
                    }
                }
            }
            0 => {
                return None;
            }
            other => {
                let ch_len = if other >= 0x80 { self.src[self.pos - 1..].chars().next().map(|c| c.len_utf8()).unwrap_or(1) } else { 1 };
                self.pos += ch_len - 1;
                self.error("E0007", "unknown escape sequence", lo, self.pos, "unknown escape");
                return None;
            }
        })
    }

    fn char_lit(&mut self, lo: usize) {
        self.pos += 1; // opening quote
        let mut value: Option<char> = None;
        let mut count = 0;
        loop {
            if self.pos >= self.end || self.peek() == b'\n' || self.peek() == b'\r' {
                self.error("E0015", "unterminated character literal", lo, self.pos, "missing closing `'`");
                self.push(TokenKind::Char(value.unwrap_or('\0')), lo, self.pos);
                return;
            }
            let c = self.peek();
            if c == b'\'' {
                self.pos += 1;
                break;
            }
            count += 1;
            if c == b'\\' {
                self.pos += 1;
                if let Some(ch) = self.escape() {
                    value.get_or_insert(ch);
                }
            } else {
                let ch = self.peek_char().unwrap();
                self.pos += ch.len_utf8();
                value.get_or_insert(ch);
            }
        }
        if count == 0 {
            self.error("E0015", "empty character literal", lo, self.pos, "a character literal needs exactly one character");
        } else if count > 1 {
            self.diags.emit(
                Diagnostic::error("E0015", "character literal contains more than one character")
                    .primary(self.span(lo, self.pos), "more than one character")
                    .help("use double quotes for a string: `\"...\"`"),
            );
        }
        self.push(TokenKind::Char(value.unwrap_or('\0')), lo, self.pos);
    }

    fn string(&mut self, lo: usize, raw: bool) {
        self.pos += 1; // opening quote
        let mut parts: Vec<StrPart> = Vec::new();
        let mut text = String::new();
        loop {
            if self.pos >= self.end {
                self.error("E0008", "unterminated string literal", lo, self.pos, "missing closing `\"`");
                break;
            }
            let c = self.peek();
            match c {
                b'"' => {
                    self.pos += 1;
                    break;
                }
                b'\n' | b'\r' => {
                    self.diags.emit(
                        Diagnostic::error("E0008", "unterminated string literal")
                            .primary(self.span(lo, self.pos), "string not closed on this line")
                            .help("strings cannot span lines in NEXUL 0.1; use `\\n` for a line break"),
                    );
                    break;
                }
                b'\\' if !raw => {
                    self.pos += 1;
                    if let Some(ch) = self.escape() {
                        text.push(ch);
                    }
                }
                b'{' if !raw => {
                    if !text.is_empty() {
                        parts.push(StrPart::Text(std::mem::take(&mut text)));
                    }
                    if let Some(part) = self.interpolation() {
                        parts.push(part);
                    }
                }
                b'}' if !raw => {
                    // A lone `}` is literal text; `\}` is also accepted for symmetry.
                    self.pos += 1;
                    text.push('}');
                }
                _ => {
                    let ch = self.peek_char().unwrap();
                    if (ch as u32) < 0x20 && ch != '\t' || ch == '\u{7f}' {
                        self.error("E0004", format!("control character U+{:04X} must be escaped in a string", ch as u32), self.pos, self.pos + 1, "write it as an escape sequence");
                    }
                    self.pos += ch.len_utf8();
                    if !is_bidi_control(ch) {
                        text.push(ch);
                    }
                }
            }
        }
        if !text.is_empty() || parts.is_empty() {
            parts.push(StrPart::Text(text));
        }
        self.push(TokenKind::Str(Box::new(StrLit { parts })), lo, self.pos);
    }

    /// At `{` inside a string: tokenizes the embedded expression and its format spec.
    fn interpolation(&mut self) -> Option<StrPart> {
        let open = self.pos;
        self.pos += 1;
        let start = self.pos;
        let mut depth = 0i32;
        let mut colon: Option<usize> = None;
        loop {
            if self.pos >= self.end || matches!(self.peek(), b'\n' | b'\r') {
                self.diags.emit(
                    Diagnostic::error("E0013", "unterminated interpolation in string")
                        .primary(self.span(open, self.pos), "`{` is never closed")
                        .help("write `\\{` for a literal brace"),
                );
                return None;
            }
            match self.peek() {
                b'"' => {
                    // A nested string literal, possibly with its own interpolations.
                    let raw = self.pos >= start + 1
                        && self.bytes[self.pos - 1] == b'r'
                        && (self.pos < start + 2 || !(self.bytes[self.pos - 2].is_ascii_alphanumeric() || self.bytes[self.pos - 2] == b'_'));
                    match skip_string(self.bytes, self.pos, self.end, raw) {
                        Some(next) => {
                            self.pos = next;
                            continue;
                        }
                        None => {
                            self.diags.emit(
                                Diagnostic::error("E0013", "unterminated interpolation in string")
                                    .primary(self.span(open, self.pos), "`{` is never closed")
                                    .help("write `\\{` for a literal brace"),
                            );
                            return None;
                        }
                    }
                }
                b'\'' => {
                    // Skip a character literal so that `'}'` or `':'` is not misread.
                    self.pos += 1;
                    while self.pos < self.end && self.peek() != b'\'' && self.peek() != b'\n' {
                        if self.peek() == b'\\' {
                            self.pos += 1;
                        }
                        self.pos += 1;
                    }
                    self.pos += 1;
                    continue;
                }
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' => depth -= 1,
                b'}' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                b':' if depth == 0 && colon.is_none() => colon = Some(self.pos),
                _ => {}
            }
            let ch = self.peek_char().unwrap();
            self.pos += ch.len_utf8();
        }
        let close = self.pos;
        self.pos += 1; // `}`
        let expr_end = colon.unwrap_or(close);
        if self.src[start..expr_end].trim().is_empty() {
            self.diags.emit(
                Diagnostic::error("E0014", "empty interpolation in string")
                    .primary(self.span(open, close + 1), "expected an expression between the braces")
                    .help("write `\\{` for a literal brace"),
            );
            return None;
        }
        let mut sub = Lexer::new(self.src, self.file, start, expr_end, self.diags, true);
        sub.run();
        let spec = colon.map(|c| self.format_spec(c + 1, close));
        Some(StrPart::Interp { tokens: sub.tokens, spec, span: self.span(start, expr_end) })
    }

    fn format_spec(&self, lo: usize, hi: usize) -> FormatSpec {
        let s = &self.src[lo..hi];
        let span = self.span(lo, hi);
        let mut spec = FormatSpec { fill: ' ', align: None, plus: false, zero: false, width: None, precision: None, kind: None, span };
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        let align_of = |c: char| match c {
            '<' => Some(Align::Left),
            '^' => Some(Align::Center),
            '>' => Some(Align::Right),
            _ => None,
        };
        if chars.len() >= 2 && align_of(chars[1]).is_some() {
            spec.fill = chars[0];
            spec.align = align_of(chars[1]);
            i = 2;
        } else if !chars.is_empty() && align_of(chars[0]).is_some() {
            spec.align = align_of(chars[0]);
            i = 1;
        }
        if i < chars.len() && chars[i] == '+' {
            spec.plus = true;
            i += 1;
        }
        if i < chars.len() && chars[i] == '0' {
            spec.zero = true;
            i += 1;
        }
        let num = |i: &mut usize| -> Option<u32> {
            let st = *i;
            while *i < chars.len() && chars[*i].is_ascii_digit() {
                *i += 1;
            }
            if *i > st {
                chars[st..*i].iter().collect::<String>().parse().ok().or(Some(u32::MAX))
            } else {
                None
            }
        };
        spec.width = num(&mut i);
        if i < chars.len() && chars[i] == '.' {
            i += 1;
            spec.precision = num(&mut i);
            if spec.precision.is_none() {
                self.error("E0009", "invalid format specification", lo, hi, "expected digits after `.`");
                return spec;
            }
        }
        if i < chars.len() {
            spec.kind = match chars[i] {
                'x' => Some(FormatKind::LowerHex),
                'X' => Some(FormatKind::UpperHex),
                'o' => Some(FormatKind::Octal),
                'b' => Some(FormatKind::Binary),
                'e' => Some(FormatKind::Exponent),
                _ => None,
            };
            if spec.kind.is_some() {
                i += 1;
            }
        }
        if i != chars.len() {
            self.diags.emit(
                Diagnostic::error("E0009", "invalid format specification")
                    .primary(span, "cannot parse this format")
                    .note("format: [[fill]align][+][0][width][.precision][x|X|o|b|e], for example `{x:>8.2}`"),
            );
        }
        if spec.width.map_or(false, |w| w > 1024) || spec.precision.map_or(false, |p| p > 1024) {
            self.error("E0009", "format width or precision is too large", lo, hi, "the maximum is 1024");
        }
        spec
    }
}

/// Skips a string literal whose opening quote is at `start`; returns the position after
/// the closing quote. Nested interpolations may themselves contain strings.
fn skip_string(b: &[u8], start: usize, end: usize, raw: bool) -> Option<usize> {
    let mut i = start + 1;
    while i < end {
        match b[i] {
            b'"' => return Some(i + 1),
            b'\\' if !raw => i += 2,
            b'\n' | b'\r' => return None,
            b'{' if !raw => {
                let mut depth = 0i32;
                i += 1;
                loop {
                    if i >= end || b[i] == b'\n' {
                        return None;
                    }
                    match b[i] {
                        b'"' => {
                            let r = b[i - 1] == b'r';
                            i = skip_string(b, i, end, r)?;
                            continue;
                        }
                        b'(' | b'[' | b'{' => depth += 1,
                        b')' | b']' => depth -= 1,
                        b'}' => {
                            if depth == 0 {
                                break;
                            }
                            depth -= 1;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceMap;
    use TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        let mut sm = SourceMap::new();
        let f = sm.add("t.nx", src);
        let d = DiagSink::new();
        let toks = lex(src, f, &d);
        assert!(d.is_empty(), "unexpected diagnostics: {:?}", d.take_sorted());
        toks.into_iter().map(|t| t.kind).collect()
    }

    fn errors(src: &str) -> Vec<&'static str> {
        let mut sm = SourceMap::new();
        let f = sm.add("t.nx", src);
        let d = DiagSink::new();
        lex(src, f, &d);
        d.take_sorted().into_iter().map(|d| d.code).collect()
    }

    fn id(s: &str) -> TokenKind {
        Ident(s.into())
    }

    #[test]
    fn newline_rules() {
        assert_eq!(kinds("let a = b\nlet c = d"), vec![Let, id("a"), Eq, id("b"), Newline, Let, id("c"), Eq, id("d"), Newline, Eof]);
        // Trailing operator continues the line.
        assert_eq!(kinds("a +\n b"), vec![id("a"), Plus, id("b"), Newline, Eof]);
        // Newlines inside parentheses are ignored.
        assert_eq!(kinds("f(a,\n b)"), vec![id("f"), LParen, id("a"), Comma, id("b"), RParen, Newline, Eof]);
        // Leading `.method` continues the line.
        assert_eq!(kinds("x\n  .len()"), vec![id("x"), Dot, id("len"), LParen, RParen, Newline, Eof]);
        // But `..` does not.
        assert_eq!(kinds("x\n..y"), vec![id("x"), Newline, DotDot, id("y"), Newline, Eof]);
        // Braces inside parentheses make newlines significant again.
        assert_eq!(
            kinds("f({\na\nb\n})"),
            vec![id("f"), LParen, LBrace, id("a"), Newline, id("b"), Newline, RBrace, RParen, Newline, Eof]
        );
        // Blank lines and comments produce a single terminator.
        assert_eq!(kinds("a // c\n\n/* x\n */ b"), vec![id("a"), Newline, id("b"), Newline, Eof]);
    }

    #[test]
    fn numbers() {
        assert_eq!(kinds("1_000 0xFF 0o17 0b101 3.5 1e3 2.5e-2"), vec![Int(1000), Int(255), Int(15), Int(5), Float(3.5), Float(1000.0), Float(0.025), Newline, Eof]);
        assert_eq!(kinds("0..10"), vec![Int(0), DotDot, Int(10), Newline, Eof]);
        assert_eq!(kinds("t.0.1"), vec![id("t"), Dot, Int(0), Dot, Int(1), Newline, Eof]);
        assert_eq!(errors("18446744073709551616"), vec!["E0301"]);
        assert_eq!(errors("10u8"), vec!["E0016"]);
        assert_eq!(errors("1_"), vec!["E0006"]);
        assert_eq!(errors("0x"), vec!["E0016"]);
    }

    #[test]
    fn strings_and_interpolation() {
        let k = kinds(r#""a {x:>4} b \{c\}""#);
        match &k[0] {
            Str(s) => {
                assert_eq!(s.parts.len(), 3);
                assert_eq!(s.parts[0], StrPart::Text("a ".into()));
                match &s.parts[1] {
                    StrPart::Interp { tokens, spec, .. } => {
                        assert_eq!(tokens.iter().map(|t| t.kind.clone()).collect::<Vec<_>>(), vec![id("x"), Eof]);
                        let spec = spec.as_ref().unwrap();
                        assert_eq!(spec.align, Some(Align::Right));
                        assert_eq!(spec.width, Some(4));
                    }
                    _ => panic!(),
                }
                assert_eq!(s.parts[2], StrPart::Text(" b {c}".into()));
            }
            _ => panic!(),
        }
        assert_eq!(errors("\"abc"), vec!["E0008"]);
        assert_eq!(errors("\"{}\""), vec!["E0014"]);
        // Interpolations may contain string literals, themselves interpolated.
        assert_eq!(errors("\"{f(\"a{g(\"b\")}\")}\""), Vec::<&str>::new());
        assert_eq!(errors("\"{f(\"a)}"), vec!["E0013"]);
        assert_eq!(errors("\"\\q\""), vec!["E0007"]);
        assert_eq!(errors("'ab'"), vec!["E0015"]);
        assert_eq!(kinds("r\"C:\\x{y}\""), vec![Str(Box::new(StrLit { parts: vec![StrPart::Text("C:\\x{y}".into())] })), Newline, Eof]);
    }

    #[test]
    fn security_and_keywords() {
        assert_eq!(errors("let a = 1 // \u{202E} hidden"), vec!["E0003"]);
        assert_eq!(errors("let trait = 1"), vec!["E0010"]);
        assert_eq!(errors("let é = 1"), vec!["E0017"]);
        assert_eq!(errors("if !x {}"), vec!["E0017"]);
    }
}

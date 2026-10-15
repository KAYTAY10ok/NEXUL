//! The NEXUL tokenizer (spec chapter 01), line by line.
//!
//! It follows the lexer of the compiler (`compiler/nxc_syntax/src/lexer.rs`) token by
//! token on valid code, which the conformance test checks, and differs on purpose where
//! an editor needs it: it never stops, keeps the text being typed readable, and splits
//! literals into their parts (text, escapes, interpolations). The only construct spanning
//! lines in NEXUL is the nested block comment: the state between lines is its depth.

use crate::{Kind, Token};

/// Declaration and other keywords ([`Kind::Keyword`]).
pub const KEYWORDS: &[&str] = &[
    "and", "as", "const", "dyn", "enum", "extern", "fn", "impl", "import", "in", "indirect", "inout", "let", "module",
    "move", "not", "or", "owned", "pub", "self", "static", "struct", "trait", "type", "unsafe", "var", "where",
];

/// Control-flow keywords ([`Kind::Control`]).
pub const CONTROL: &[&str] = &["break", "continue", "else", "for", "if", "match", "return", "while"];

/// Keywords reserved for future versions ([`Kind::Keyword`]; the compiler rejects them as
/// names).
pub const RESERVED: &[&str] =
    &["async", "await", "class", "defer", "interface", "loop", "macro", "ref", "try", "yield"];

/// Keywords that are values ([`Kind::Constant`]).
pub const LITERAL_KEYWORDS: &[&str] = &["true", "false"];

/// Primitive types and `Self` ([`Kind::Type`]); other types are recognized by their
/// `CamelCase` name.
pub const PRIMITIVE_TYPES: &[&str] = &[
    "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize", "Never",
    "Self",
];

/// Nested strings in an interpolation deeper than this are treated as unterminated, which
/// bounds the recursion on hostile input (the compiler has no limit; real code nests 2 or 3).
const MAX_NESTING: u32 = 64;

/// Appends the tokens of `line` (without `'\n'`; a final `'\r'` is ignored), shifted by
/// `base`, given the depth of block comments open at its start; returns the depth at its
/// end.
pub(crate) fn tokenize(line: &str, depth: u32, base: usize, first_line: bool, out: &mut Vec<Token>) -> u32 {
    let end = if line.ends_with('\r') { line.len() - 1 } else { line.len() };
    let mut lx = Lexer { s: line, b: line.as_bytes(), pos: 0, end, base, out, prev: Prev::Other };
    if first_line && line[..end].starts_with('\u{FEFF}') {
        lx.pos = '\u{FEFF}'.len_utf8();
    }
    lx.run(depth)
}

/// What the previous token says about the next identifier.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Prev {
    Other,
    /// `.`: no float literal can follow (`t.0.1`).
    Dot,
    /// `fn`: the next name is a function.
    Fn,
    /// `struct`, `enum`, `trait`, `type`: the next name is a type.
    TypeDecl,
}

struct Lexer<'a> {
    s: &'a str,
    b: &'a [u8],
    pos: usize,
    /// End of the text being tokenized: the line, or the expression of an interpolation.
    end: usize,
    base: usize,
    out: &'a mut Vec<Token>,
    prev: Prev,
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Bidirectional control characters, forbidden everywhere ("Trojan Source", spec §1.1).
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Length of the UTF-8 character starting with byte `c`.
fn utf8_len(c: u8) -> usize {
    match c {
        0xF0.. => 4,
        0xE0.. => 3,
        0xC0.. => 2,
        _ => 1,
    }
}

fn keyword_kind(word: &str) -> Option<Kind> {
    Some(match word {
        "break" | "continue" | "else" | "for" | "if" | "match" | "return" | "while" => Kind::Control,
        "and" | "as" | "const" | "dyn" | "enum" | "extern" | "fn" | "impl" | "import" | "in" | "indirect" | "inout"
        | "let" | "module" | "move" | "not" | "or" | "owned" | "pub" | "self" | "static" | "struct" | "trait"
        | "type" | "unsafe" | "var" | "where" => Kind::Keyword,
        "async" | "await" | "class" | "defer" | "interface" | "loop" | "macro" | "ref" | "try" | "yield" => {
            Kind::Keyword
        }
        "true" | "false" => Kind::Constant,
        "Self" => Kind::Type,
        _ => return None,
    })
}

fn is_primitive_type(word: &str) -> bool {
    matches!(
        word,
        "bool"
            | "char"
            | "f32"
            | "f64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
            | "Never"
    )
}

impl Lexer<'_> {
    fn at(&self, n: usize) -> u8 {
        if self.pos + n < self.end {
            self.b[self.pos + n]
        } else {
            0
        }
    }

    fn push(&mut self, lo: usize, hi: usize, kind: Kind) {
        if lo < hi {
            self.out.push(Token { range: self.base + lo..self.base + hi, kind });
        }
    }

    fn run(&mut self, mut depth: u32) -> u32 {
        if depth > 0 {
            depth = self.block_comment(self.pos, depth);
        }
        while self.pos < self.end {
            match self.b[self.pos] {
                b' ' | b'\t' => self.pos += 1,
                b'/' if self.at(1) == b'/' => self.line_comment(),
                b'/' if self.at(1) == b'*' => {
                    let lo = self.pos;
                    self.pos += 2;
                    depth = self.block_comment(lo, 1);
                }
                b'"' => self.string(self.pos, false),
                b'r' if self.at(1) == b'"' => {
                    let lo = self.pos;
                    self.pos += 1;
                    self.string(lo, true);
                }
                b'\'' => self.char_literal(),
                b'0'..=b'9' => self.number(),
                b'@' => self.attribute(),
                c if is_ident_start(c) => self.word(),
                0x80.. => self.non_ascii(),
                _ => self.operator(),
            }
        }
        depth
    }

    /// Emits `lo..hi` as `kind`, except bidirectional control characters (and, when
    /// `strict`, other control characters than the tab) which are emitted as invalid.
    fn text(&mut self, lo: usize, hi: usize, kind: Kind, strict: bool) {
        let mut seg = lo;
        let mut i = lo;
        while i < hi {
            let c = self.b[i];
            // U+061C starts with 0xD8, the other bidi controls with 0xE2 (never a
            // continuation byte, so `i` is on a character boundary).
            let bad = if c == 0xD8 || c == 0xE2 {
                let ch = self.s[i..].chars().next().unwrap_or(' ');
                if is_bidi_control(ch) {
                    ch.len_utf8()
                } else {
                    0
                }
            } else if strict && ((c < 0x20 && c != b'\t') || c == 0x7F) {
                1
            } else {
                0
            };
            if bad > 0 {
                self.push(seg, i, kind);
                self.push(i, i + bad, Kind::Invalid);
                i += bad;
                seg = i;
            } else {
                i += 1;
            }
        }
        self.push(seg, hi, kind);
    }

    fn line_comment(&mut self) {
        let lo = self.pos;
        // `///` is a doc comment, `////...` an ordinary one (as in Rust).
        let doc = self.at(2) == b'/' && self.at(3) != b'/';
        self.pos = self.end;
        self.text(lo, self.end, if doc { Kind::DocComment } else { Kind::Comment }, false);
    }

    /// Continues a block comment of nesting `depth` from `self.pos` (the comment token starts
    /// at `lo`); returns the depth at the end of the comment or of the text.
    fn block_comment(&mut self, lo: usize, mut depth: u32) -> u32 {
        while self.pos < self.end {
            if self.b[self.pos] == b'/' && self.at(1) == b'*' {
                depth = depth.saturating_add(1);
                self.pos += 2;
            } else if self.b[self.pos] == b'*' && self.at(1) == b'/' {
                depth -= 1;
                self.pos += 2;
                if depth == 0 {
                    break;
                }
            } else {
                self.pos += 1;
            }
        }
        // `pos` may have stopped inside a multi-byte character only at `end`, which is a
        // boundary: the loop advances byte by byte but stops on ASCII delimiters.
        self.text(lo, self.pos, Kind::Comment, false);
        depth
    }

    fn word(&mut self) {
        let lo = self.pos;
        while self.pos < self.end && is_ident(self.b[self.pos]) {
            self.pos += 1;
        }
        let word = &self.s[lo..self.pos];
        let kind = match keyword_kind(word) {
            Some(k) => k,
            None => self.identifier_kind(word),
        };
        self.push(lo, self.pos, kind);
        self.prev = match word {
            "fn" => Prev::Fn,
            "struct" | "enum" | "trait" | "type" => Prev::TypeDecl,
            _ => Prev::Other,
        };
    }

    fn identifier_kind(&self, word: &str) -> Kind {
        match self.prev {
            Prev::Fn => return Kind::Function,
            Prev::TypeDecl => return Kind::Type,
            _ => {}
        }
        if is_primitive_type(word) {
            return Kind::Type;
        }
        let b = word.as_bytes();
        if b[0].is_ascii_uppercase() {
            let screaming =
                b.len() >= 2 && b.iter().all(|&c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_');
            return if screaming { Kind::Constant } else { Kind::Type };
        }
        if self.called() {
            Kind::Function
        } else {
            Kind::Variable
        }
    }

    /// Is the name just read followed by a call: `(`, or generic arguments then `(`?
    fn called(&self) -> bool {
        let skip_blanks = |mut i: usize| {
            while i < self.end && matches!(self.b[i], b' ' | b'\t') {
                i += 1;
            }
            i
        };
        let mut i = skip_blanks(self.pos);
        if i < self.end && self.b[i] == b'[' {
            let mut depth = 0u32;
            while i < self.end {
                match self.b[i] {
                    b'[' => depth += 1,
                    b']' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    b'"' | b'\'' | b'(' | b'{' | b'/' => return false,
                    _ => {}
                }
                i += 1;
            }
            if i >= self.end {
                return false;
            }
            i = skip_blanks(i + 1);
        }
        i < self.end && self.b[i] == b'('
    }

    fn attribute(&mut self) {
        let lo = self.pos;
        self.pos += 1;
        if self.pos < self.end && is_ident_start(self.b[self.pos]) {
            while self.pos < self.end && is_ident(self.b[self.pos]) {
                self.pos += 1;
            }
            self.push(lo, self.pos, Kind::Attribute);
        } else {
            self.push(lo, self.pos, Kind::Punctuation);
        }
        self.prev = Prev::Other;
    }

    fn number(&mut self) {
        let lo = self.pos;
        let after_dot = self.prev == Prev::Dot;
        self.prev = Prev::Other;
        let radix = if self.b[lo] == b'0' {
            match self.at(1) {
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
            let digits = self.pos;
            while self.pos < self.end && (self.b[self.pos] == b'_' || (self.b[self.pos] as char).is_digit(radix)) {
                self.pos += 1;
            }
            let number_end = self.pos;
            self.suffix();
            if self.b[digits..number_end].iter().all(|&c| c == b'_') {
                // `0x` without digits.
                self.push(lo, self.pos, Kind::Invalid);
            } else {
                self.push(lo, number_end, Kind::Number);
                self.push(number_end, self.pos, Kind::Invalid);
            }
            return;
        }
        self.digits();
        if !after_dot && self.at(0) == b'.' && self.at(1).is_ascii_digit() {
            self.pos += 1;
            self.digits();
        }
        if !after_dot && matches!(self.at(0), b'e' | b'E') {
            let mut p = 1;
            if matches!(self.at(1), b'+' | b'-') {
                p = 2;
            }
            if self.at(p).is_ascii_digit() {
                self.pos += p;
                self.digits();
            }
        }
        let number_end = self.pos;
        self.suffix();
        self.push(lo, number_end, Kind::Number);
        self.push(number_end, self.pos, Kind::Invalid);
    }

    fn digits(&mut self) {
        while self.pos < self.end && (self.b[self.pos].is_ascii_digit() || self.b[self.pos] == b'_') {
            self.pos += 1;
        }
    }

    /// Letters and digits glued to a number (`10u8`, `0b102`): an error for the compiler.
    fn suffix(&mut self) {
        while self.pos < self.end && is_ident(self.b[self.pos]) {
            self.pos += 1;
        }
    }

    fn non_ascii(&mut self) {
        let lo = self.pos;
        while self.pos < self.end && self.b[self.pos] >= 0x80 {
            self.pos += utf8_len(self.b[self.pos]);
        }
        self.pos = self.pos.min(self.end);
        self.push(lo, self.pos, Kind::Invalid);
        self.prev = Prev::Other;
    }

    fn operator(&mut self) {
        let lo = self.pos;
        let (c, c1, c2) = (self.b[lo], self.at(1), self.at(2));
        let (len, kind) = match (c, c1, c2) {
            (b'<', b'<', b'=')
            | (b'>', b'>', b'=')
            | (b'+', b'%', b'=')
            | (b'-', b'%', b'=')
            | (b'*', b'%', b'=')
            | (b'.', b'.', b'=') => (3, Kind::Operator),
            _ => match (c, c1) {
                (b'<', b'<')
                | (b'>', b'>')
                | (b'+', b'%')
                | (b'-', b'%')
                | (b'*', b'%')
                | (b'+', b'=')
                | (b'-', b'=')
                | (b'*', b'=')
                | (b'/', b'=')
                | (b'%', b'=')
                | (b'&', b'=')
                | (b'|', b'=')
                | (b'^', b'=')
                | (b'=', b'=')
                | (b'!', b'=')
                | (b'<', b'=')
                | (b'>', b'=')
                | (b'-', b'>')
                | (b'=', b'>')
                | (b'.', b'.') => (2, Kind::Operator),
                _ => match c {
                    b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^' | b'~' | b'=' | b'<' | b'>' | b'?' => {
                        (1, Kind::Operator)
                    }
                    b'(' | b')' | b'[' | b']' | b'{' | b'}' | b',' | b'.' | b':' | b';' => (1, Kind::Punctuation),
                    // `!` (negation is `not`), `$`, `#`, `` ` ``, `\`, control characters,
                    // a carriage return in the middle of a line.
                    _ => (1, Kind::Invalid),
                },
            },
        };
        self.pos += len;
        self.push(lo, self.pos, kind);
        self.prev = if c == b'.' && len == 1 { Prev::Dot } else { Prev::Other };
    }

    /// End of the escape sequence whose backslash is at `i`, and whether it is valid.
    fn escape_end(&self, i: usize) -> (usize, bool) {
        let mut p = i + 1;
        if p >= self.end {
            return (p, false);
        }
        match self.b[p] {
            b'n' | b'r' | b't' | b'0' | b'\\' | b'\'' | b'"' | b'{' | b'}' => (p + 1, true),
            b'u' => {
                p += 1;
                if p >= self.end || self.b[p] != b'{' {
                    return (p, false);
                }
                p += 1;
                let digits = p;
                while p < self.end && self.b[p].is_ascii_hexdigit() {
                    p += 1;
                }
                if p >= self.end || self.b[p] != b'}' {
                    return (p, false);
                }
                let hex = &self.s[digits..p];
                let ok = !hex.is_empty()
                    && hex.len() <= 6
                    && u32::from_str_radix(hex, 16).ok().and_then(char::from_u32).is_some();
                (p + 1, ok)
            }
            c => ((p + utf8_len(c)).min(self.end), false),
        }
    }

    fn escape(&mut self) {
        let lo = self.pos;
        let (hi, ok) = self.escape_end(lo);
        self.pos = hi;
        self.push(lo, hi, if ok { Kind::Escape } else { Kind::Invalid });
    }

    /// A string literal whose first character (`"`, or `r` of `r"`) is at `lo`;
    /// `self.pos` is on the opening quote.
    fn string(&mut self, lo: usize, raw: bool) {
        self.pos += 1;
        let mut seg = lo;
        loop {
            if self.pos >= self.end {
                // Unterminated: the string goes to the end of the line.
                self.text(seg, self.end, Kind::String, true);
                break;
            }
            match self.b[self.pos] {
                b'"' => {
                    self.pos += 1;
                    self.text(seg, self.pos, Kind::String, true);
                    break;
                }
                b'\\' if !raw => {
                    self.text(seg, self.pos, Kind::String, true);
                    self.escape();
                    seg = self.pos;
                }
                b'{' if !raw => match interpolation_end(self.b, self.pos + 1, self.end, 0) {
                    Some((close, colon)) => {
                        self.text(seg, self.pos, Kind::String, true);
                        self.interpolation(self.pos, close, colon);
                        self.pos = close + 1;
                        seg = self.pos;
                    }
                    None => {
                        // An interpolation still being typed: the rest of the line stays
                        // string text rather than turning into code.
                        self.pos = self.end;
                        self.text(seg, self.end, Kind::String, true);
                        break;
                    }
                },
                _ => self.pos += 1,
            }
        }
        self.prev = Prev::Other;
    }

    /// The interpolation `{expr[:spec]}` from `open` to `close` (the braces).
    fn interpolation(&mut self, open: usize, close: usize, colon: Option<usize>) {
        let expr_end = colon.unwrap_or(close);
        if self.s[open + 1..expr_end].trim().is_empty() {
            self.push(open, close + 1, Kind::Invalid);
            return;
        }
        self.push(open, open + 1, Kind::Interpolation);
        let (end, prev) = (self.end, self.prev);
        self.end = expr_end;
        self.pos = open + 1;
        self.prev = Prev::Other;
        // A block comment left open ends with the expression.
        self.run(0);
        self.end = end;
        self.prev = prev;
        if let Some(colon) = colon {
            let kind =
                if format_spec_is_valid(&self.s[colon + 1..close]) { Kind::Interpolation } else { Kind::Invalid };
            self.push(colon, close, kind);
        }
        self.push(close, close + 1, Kind::Interpolation);
    }

    fn char_literal(&mut self) {
        let lo = self.pos;
        let mut i = lo + 1;
        let mut count = 0;
        let closed = loop {
            if i >= self.end {
                break false;
            }
            match self.b[i] {
                b'\'' => {
                    i += 1;
                    break true;
                }
                b'\\' => {
                    count += 1;
                    i = self.escape_end(i).0;
                }
                c => {
                    count += 1;
                    i += utf8_len(c);
                }
            }
        };
        let i = i.min(self.end);
        if closed && count != 1 {
            // `''`, or `'ab'` where a string was meant.
            self.push(lo, i, Kind::Invalid);
        } else {
            let body_end = if closed { i - 1 } else { i };
            let mut seg = lo;
            self.pos = lo + 1;
            while self.pos < body_end {
                if self.b[self.pos] == b'\\' {
                    self.text(seg, self.pos, Kind::String, true);
                    self.escape();
                    seg = self.pos;
                } else {
                    self.pos += 1;
                }
            }
            self.text(seg, i, Kind::String, true);
        }
        self.pos = i;
        self.prev = Prev::Other;
    }
}

/// Finds the `}` closing an interpolation whose expression starts at `i`, and the first
/// `:` at depth zero (start of a format spec). Same scan as the compiler: nested string
/// literals and character literals are skipped.
fn interpolation_end(b: &[u8], mut i: usize, end: usize, nesting: u32) -> Option<(usize, Option<usize>)> {
    if nesting > MAX_NESTING {
        return None;
    }
    let start = i;
    let mut depth = 0i32;
    let mut colon = None;
    while i < end {
        match b[i] {
            b'"' => {
                let raw = i > start && b[i - 1] == b'r' && (i < start + 2 || !is_ident(b[i - 2]));
                i = skip_string(b, i, end, raw, nesting + 1)?;
                continue;
            }
            b'\'' => {
                i += 1;
                while i < end && b[i] != b'\'' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' => depth -= 1,
            b'}' => {
                if depth == 0 {
                    return Some((i, colon));
                }
                depth -= 1;
            }
            b':' if depth == 0 && colon.is_none() => colon = Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Skips a string literal whose quote is at `start`; returns the position after its
/// closing quote, or `None` if it is not closed on the line.
fn skip_string(b: &[u8], start: usize, end: usize, raw: bool, nesting: u32) -> Option<usize> {
    if nesting > MAX_NESTING {
        return None;
    }
    let mut i = start + 1;
    while i < end {
        match b[i] {
            b'"' => return Some(i + 1),
            b'\\' if !raw => i += 2,
            b'{' if !raw => {
                let mut depth = 0i32;
                i += 1;
                loop {
                    if i >= end {
                        return None;
                    }
                    match b[i] {
                        b'"' => {
                            let r = b[i - 1] == b'r';
                            i = skip_string(b, i, end, r, nesting + 1)?;
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

/// `[[fill]align][+][0][width][.precision][x|X|o|b|e]`, width and precision at most 1024
/// (spec §1.5); the empty spec is valid.
fn format_spec_is_valid(spec: &str) -> bool {
    let chars: Vec<char> = spec.chars().collect();
    let is_align = |c: char| matches!(c, '<' | '^' | '>');
    let mut i = 0;
    if chars.len() >= 2 && is_align(chars[1]) {
        i = 2;
    } else if !chars.is_empty() && is_align(chars[0]) {
        i = 1;
    }
    if chars.get(i) == Some(&'+') {
        i += 1;
    }
    if chars.get(i) == Some(&'0') {
        i += 1;
    }
    let number = |i: &mut usize| -> Option<u64> {
        let start = *i;
        let mut v: u64 = 0;
        while *i < chars.len() && chars[*i].is_ascii_digit() {
            v = (v * 10 + chars[*i].to_digit(10).unwrap_or(0) as u64).min(u32::MAX as u64);
            *i += 1;
        }
        (*i > start).then_some(v)
    };
    let width = number(&mut i);
    let mut precision = None;
    if chars.get(i) == Some(&'.') {
        i += 1;
        precision = number(&mut i);
        if precision.is_none() {
            return false;
        }
    }
    if matches!(chars.get(i), Some('x' | 'X' | 'o' | 'b' | 'e')) {
        i += 1;
    }
    i == chars.len() && width.is_none_or(|w| w <= 1024) && precision.is_none_or(|p| p <= 1024)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens;
    use Kind::*;

    /// `(text, kind)` of every token.
    fn toks(src: &str) -> Vec<(&str, Kind)> {
        tokens("nexul", src).into_iter().map(|t| (&src[t.range], t.kind)).collect()
    }

    fn kinds(src: &str) -> Vec<Kind> {
        tokens("nexul", src).into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn word_lists_agree_with_the_classifier() {
        for w in KEYWORDS.iter().chain(RESERVED) {
            assert_eq!(keyword_kind(w), Some(Keyword), "{w}");
        }
        for w in CONTROL {
            assert_eq!(keyword_kind(w), Some(Control), "{w}");
        }
        for w in LITERAL_KEYWORDS {
            assert_eq!(keyword_kind(w), Some(Constant), "{w}");
        }
        for w in PRIMITIVE_TYPES {
            assert_eq!(toks(w), vec![(*w, Type)], "{w}");
        }
    }

    #[test]
    fn declarations() {
        assert_eq!(
            toks("pub fn aire[T: Forme](f: T) -> f64 {"),
            vec![
                ("pub", Keyword),
                ("fn", Keyword),
                ("aire", Function),
                ("[", Punctuation),
                ("T", Type),
                (":", Punctuation),
                ("Forme", Type),
                ("]", Punctuation),
                ("(", Punctuation),
                ("f", Variable),
                (":", Punctuation),
                ("T", Type),
                (")", Punctuation),
                ("->", Operator),
                ("f64", Type),
                ("{", Punctuation),
            ]
        );
        assert_eq!(kinds("struct point_t {"), vec![Keyword, Type, Punctuation]);
        assert_eq!(kinds("type Octet = u8"), vec![Keyword, Type, Operator, Type]);
        assert_eq!(kinds("const MAX_N: usize = 10"), vec![Keyword, Constant, Punctuation, Type, Operator, Number]);
        assert_eq!(kinds("impl Self"), vec![Keyword, Type]);
        // A comment between `fn` and the name does not hide the declaration.
        assert_eq!(kinds("fn /* x */ f"), vec![Keyword, Comment, Function]);
    }

    #[test]
    fn calls_fields_and_control() {
        assert_eq!(
            toks("if x.len () > 0 and not fini { return taille[u64]() }"),
            vec![
                ("if", Control),
                ("x", Variable),
                (".", Punctuation),
                ("len", Function),
                ("(", Punctuation),
                (")", Punctuation),
                (">", Operator),
                ("0", Number),
                ("and", Keyword),
                ("not", Keyword),
                ("fini", Variable),
                ("{", Punctuation),
                ("return", Control),
                ("taille", Function),
                ("[", Punctuation),
                ("u64", Type),
                ("]", Punctuation),
                ("(", Punctuation),
                (")", Punctuation),
                ("}", Punctuation),
            ]
        );
        assert_eq!(kinds("p.x"), vec![Variable, Punctuation, Variable]);
        assert_eq!(
            kinds("Some(n) => total += n"),
            vec![Type, Punctuation, Variable, Punctuation, Operator, Variable, Operator, Variable]
        );
        assert_eq!(kinds("for i in 0..10 {"), vec![Control, Variable, Keyword, Number, Operator, Number, Punctuation]);
        assert_eq!(kinds("v[i]"), vec![Variable, Punctuation, Variable, Punctuation]);
        assert_eq!(kinds("let yield = true"), vec![Keyword, Keyword, Operator, Constant]);
    }

    #[test]
    fn operators_longest_match() {
        assert_eq!(
            toks("a <<= b >>= c +%= d -% e *% f != g ..= h => i -> j ? ~k"),
            vec![
                ("a", Variable),
                ("<<=", Operator),
                ("b", Variable),
                (">>=", Operator),
                ("c", Variable),
                ("+%=", Operator),
                ("d", Variable),
                ("-%", Operator),
                ("e", Variable),
                ("*%", Operator),
                ("f", Variable),
                ("!=", Operator),
                ("g", Variable),
                ("..=", Operator),
                ("h", Variable),
                ("=>", Operator),
                ("i", Variable),
                ("->", Operator),
                ("j", Variable),
                ("?", Operator),
                ("~", Operator),
                ("k", Variable),
            ]
        );
        assert_eq!(toks("!x"), vec![("!", Invalid), ("x", Variable)]);
        assert_eq!(toks("a $ b"), vec![("a", Variable), ("$", Invalid), ("b", Variable)]);
    }

    #[test]
    fn numbers() {
        assert_eq!(
            toks("42 1_000_000 0xFF_FF 0o755 0b1010_0001 3.14 1.0e-9 6e23"),
            vec![
                ("42", Number),
                ("1_000_000", Number),
                ("0xFF_FF", Number),
                ("0o755", Number),
                ("0b1010_0001", Number),
                ("3.14", Number),
                ("1.0e-9", Number),
                ("6e23", Number),
            ]
        );
        assert_eq!(toks("0..10"), vec![("0", Number), ("..", Operator), ("10", Number)]);
        assert_eq!(
            toks("t.0.1"),
            vec![("t", Variable), (".", Punctuation), ("0", Number), (".", Punctuation), ("1", Number)]
        );
        assert_eq!(toks("1.max"), vec![("1", Number), (".", Punctuation), ("max", Variable)]);
        assert_eq!(toks("10u8"), vec![("10", Number), ("u8", Invalid)]);
        assert_eq!(toks("0b102"), vec![("0b10", Number), ("2", Invalid)]);
        assert_eq!(toks("0x"), vec![("0x", Invalid)]);
        assert_eq!(toks("1e"), vec![("1", Number), ("e", Invalid)]);
    }

    #[test]
    fn strings_escapes_and_interpolation() {
        assert_eq!(
            toks(r#""a\n{x:>8.2} b\{c\}""#),
            vec![
                ("\"a", String),
                ("\\n", Escape),
                ("{", Interpolation),
                ("x", Variable),
                (":>8.2", Interpolation),
                ("}", Interpolation),
                (" b", String),
                ("\\{", Escape),
                ("c", String),
                ("\\}", Escape),
                ("\"", String),
            ]
        );
        // Interpolated expressions may contain strings, themselves interpolated.
        assert_eq!(
            toks(r#""{nom.replace("-", " {n}")}""#),
            vec![
                ("\"", String),
                ("{", Interpolation),
                ("nom", Variable),
                (".", Punctuation),
                ("replace", Function),
                ("(", Punctuation),
                ("\"-\"", String),
                (",", Punctuation),
                ("\" ", String),
                ("{", Interpolation),
                ("n", Variable),
                ("}", Interpolation),
                ("\"", String),
                (")", Punctuation),
                ("}", Interpolation),
                ("\"", String),
            ]
        );
        // A lone `}` is text; `'}'` and `':'` inside an interpolation are characters.
        assert_eq!(toks(r#""a}""#), vec![("\"a}\"", String)]);
        assert_eq!(
            toks(r#""{f('}', ':')}""#).into_iter().map(|t| t.1).collect::<Vec<_>>(),
            vec![
                String,
                Interpolation,
                Function,
                Punctuation,
                String,
                Punctuation,
                String,
                Punctuation,
                Interpolation,
                String
            ]
        );
        // Raw strings: no escape, no interpolation.
        assert_eq!(toks(r#"r"C:\x{y}""#), vec![(r#"r"C:\x{y}""#, String)]);
        assert_eq!(toks(r#"r "x""#), vec![("r", Variable), ("\"x\"", String)]);
        // Errors stay visible.
        assert_eq!(toks(r#""\q""#), vec![("\"", String), ("\\q", Invalid), ("\"", String)]);
        assert_eq!(toks(r#""\u{110000}""#)[1], ("\\u{110000}", Invalid));
        assert_eq!(toks(r#""\u{1F600}""#)[1], ("\\u{1F600}", Escape));
        assert_eq!(toks(r#""\u{1234567}""#)[1], ("\\u{1234567}", Invalid));
        assert_eq!(toks(r#""\u12""#)[1], ("\\u", Invalid));
        assert_eq!(toks(r#""\é""#)[1], ("\\é", Invalid));
        assert_eq!(toks(r#""{}""#), vec![("\"", String), ("{}", Invalid), ("\"", String)]);
        assert_eq!(toks(r#""{ :x}""#)[1], ("{ :x}", Invalid));
        assert_eq!(toks(r#""{x:}""#)[3], (":", Interpolation));
        assert_eq!(toks(r#""{x:q}""#)[3], (":q", Invalid));
        assert_eq!(toks(r#""{x:2000}""#)[3], (":2000", Invalid));
        assert_eq!(toks(r#""{x:-<20}""#)[3], (":-<20", Interpolation));
        assert_eq!(toks(r#""{x:+08.3e}""#)[3], (":+08.3e", Interpolation));
        assert_eq!(toks(r#""{x:.}""#)[3], (":.", Invalid));
    }

    #[test]
    fn characters() {
        assert_eq!(toks("'a' 'é' '😀'"), vec![("'a'", String), ("'é'", String), ("'😀'", String)]);
        assert_eq!(toks(r"'\n'"), vec![("'", String), ("\\n", Escape), ("'", String)]);
        assert_eq!(toks(r"'\u{1F600}'"), vec![("'", String), ("\\u{1F600}", Escape), ("'", String)]);
        assert_eq!(toks(r"'\''"), vec![("'", String), ("\\'", Escape), ("'", String)]);
        assert_eq!(toks("'ab' x"), vec![("'ab'", Invalid), ("x", Variable)]);
        assert_eq!(toks("''"), vec![("''", Invalid)]);
    }

    #[test]
    fn comments() {
        assert_eq!(toks("x // note"), vec![("x", Variable), ("// note", Comment)]);
        assert_eq!(toks("/// Doc."), vec![("/// Doc.", DocComment)]);
        assert_eq!(toks("//// line"), vec![("//// line", Comment)]);
        assert_eq!(toks("///"), vec![("///", DocComment)]);
        assert_eq!(
            toks("a /* b /* c */ d */ e"),
            vec![("a", Variable), ("/* b /* c */ d */", Comment), ("e", Variable)]
        );
        let t = toks("/* a\n /* b */\n c */ d");
        assert_eq!(t, vec![("/* a", Comment), (" /* b */", Comment), (" c */", Comment), ("d", Variable)]);
        // `/` then `/=` are operators, not comments.
        assert_eq!(kinds("a / b /= c"), vec![Variable, Operator, Variable, Operator, Variable]);
    }

    #[test]
    fn attributes() {
        assert_eq!(
            toks("@test fn f()"),
            vec![("@test", Attribute), ("fn", Keyword), ("f", Function), ("(", Punctuation), (")", Punctuation)]
        );
        assert_eq!(toks("@cfg(windows)")[0], ("@cfg", Attribute));
        assert_eq!(toks("@ x"), vec![("@", Punctuation), ("x", Variable)]);
    }

    #[test]
    fn trojan_source_and_foreign_characters() {
        // Bidirectional controls are flagged even inside comments and strings.
        assert_eq!(
            toks("x // a\u{202E}b"),
            vec![("x", Variable), ("// a", Comment), ("\u{202E}", Invalid), ("b", Comment)]
        );
        assert_eq!(toks("\"\u{2066}\""), vec![("\"", String), ("\u{2066}", Invalid), ("\"", String)]);
        assert_eq!(toks("'\u{061C}'")[1], ("\u{061C}", Invalid));
        // Other characters are fine in strings and comments...
        assert_eq!(toks("\"é — 中文 😀\" // ü"), vec![("\"é — 中文 😀\"", String), ("// ü", Comment)]);
        // ...but not in code (identifiers are ASCII).
        assert_eq!(toks("let é = 1"), vec![("let", Keyword), ("é", Invalid), ("=", Operator), ("1", Number)]);
        assert_eq!(toks("aé"), vec![("a", Variable), ("é", Invalid)]);
        // Control characters must be escaped in strings.
        assert_eq!(toks("\"a\u{7}\""), vec![("\"a", String), ("\u{7}", Invalid), ("\"", String)]);
        assert_eq!(toks("\"\ta\""), vec![("\"\ta\"", String)]);
    }

    #[test]
    fn text_being_typed() {
        assert_eq!(
            toks("let s = \"Bonj"),
            vec![("let", Keyword), ("s", Variable), ("=", Operator), ("\"Bonj", String)]
        );
        assert_eq!(toks("\"a {nom"), vec![("\"a {nom", String)]);
        assert_eq!(
            toks("\"a {nom} \\"),
            vec![
                ("\"a ", String),
                ("{", Interpolation),
                ("nom", Variable),
                ("}", Interpolation),
                (" ", String),
                ("\\", Invalid)
            ]
        );
        assert_eq!(toks("'"), vec![("'", String)]);
        assert_eq!(toks("'a"), vec![("'a", String)]);
        assert_eq!(toks("/* ouvert"), vec![("/* ouvert", Comment)]);
        assert_eq!(toks("x.").last(), Some(&(".", Punctuation)));
        // An unterminated string ends with its line.
        assert_eq!(toks("\"a\nb"), vec![("\"a", String), ("b", Variable)]);
        // A block comment opened in an interpolation ends with the expression.
        assert_eq!(
            toks("\"{a /* b}\" c"),
            vec![
                ("\"", String),
                ("{", Interpolation),
                ("a", Variable),
                ("/* b", Comment),
                ("}", Interpolation),
                ("\"", String),
                ("c", Variable)
            ]
        );
    }

    #[test]
    fn deep_nesting_is_bounded() {
        let mut s = std::string::String::new();
        for _ in 0..10_000 {
            s.push_str("\"{");
        }
        let t = tokens("nexul", &s);
        assert_eq!(t, vec![Token { range: 0..s.len(), kind: String }]);
        let mut ok = std::string::String::new();
        for _ in 0..10 {
            ok.push_str("\"{");
        }
        ok.push('x');
        for _ in 0..10 {
            ok.push_str("}\"");
        }
        assert!(tokens("nexul", &ok).iter().any(|t| t.kind == Variable));
    }
}

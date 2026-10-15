//! A small JSON value, parser and serializer (RFC 8259): enough for the Language Server
//! Protocol, so that the crate needs no dependency.

use std::fmt::{self, Write as _};

/// A JSON value. Integers that fit in `i64` keep their exact value.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Json {
    #[default]
    Null,
    Bool(bool),
    /// A number written without fraction or exponent that fits in `i64`.
    Int(i64),
    /// Any other number.
    Float(f64),
    String(String),
    Array(Vec<Json>),
    /// Members in order. Keys are unique: when parsing, the last duplicate wins.
    Object(Vec<(String, Json)>),
}

/// Error of [`Json::parse`]: what is wrong, and where (byte offset).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid JSON at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Nesting limit of the parser: deeper documents are rejected instead of overflowing the
/// stack.
const MAX_DEPTH: usize = 256;

impl Json {
    /// An object from `(key, value)` pairs.
    pub fn object<K: Into<String>>(members: impl IntoIterator<Item = (K, Json)>) -> Json {
        let mut obj = Json::Object(Vec::new());
        for (k, v) in members {
            obj.insert(k, v);
        }
        obj
    }

    /// Parses a JSON text (surrounding whitespace allowed).
    pub fn parse(text: &str) -> Result<Json, ParseError> {
        let mut p = Parser { s: text.as_bytes(), pos: 0 };
        p.skip_ws();
        let v = p.value(0)?;
        p.skip_ws();
        if p.pos != p.s.len() {
            return Err(p.error("unexpected text after the value"));
        }
        Ok(v)
    }

    /// Member `key` of an object (`None` for other values).
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Follows a path of object keys: `v.path(&["capabilities", "hoverProvider"])`.
    pub fn path(&self, keys: &[&str]) -> Option<&Json> {
        keys.iter().try_fold(self, |v, k| v.get(k))
    }

    /// Element `i` of an array.
    pub fn at(&self, i: usize) -> Option<&Json> {
        self.as_array().and_then(|a| a.get(i))
    }

    /// Sets member `key` of an object (replacing it if present). A `Null` value becomes an
    /// empty object first; other values are left unchanged.
    pub fn insert(&mut self, key: impl Into<String>, value: Json) {
        if self.is_null() {
            *self = Json::Object(Vec::new());
        }
        if let Json::Object(m) = self {
            let key = key.into();
            match m.iter_mut().find(|(k, _)| *k == key) {
                Some((_, v)) => *v = value,
                None => m.push((key, value)),
            }
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The value of an integer (a float with an integral value in range is accepted).
    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Json::Int(i) => Some(i),
            Json::Float(f) if f.fract() == 0.0 && f >= i64::MIN as f64 && f < i64::MAX as f64 => Some(f as i64),
            _ => None,
        }
    }

    /// A non-negative integer that fits in `u32` (LSP `uinteger`: lines, characters).
    pub fn as_u32(&self) -> Option<u32> {
        self.as_i64().and_then(|i| u32::try_from(i).ok())
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Json::Int(i) => Some(i as f64),
            Json::Float(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Vec<(String, Json)>> {
        match self {
            Json::Object(m) => Some(m),
            _ => None,
        }
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Int(i) => {
                let _ = write!(out, "{i}");
            }
            Json::Float(f) => {
                if !f.is_finite() {
                    out.push_str("null");
                } else if *f != 0.0 && (f.abs() >= 1e16 || f.abs() < 1e-5) {
                    let _ = write!(out, "{f:e}");
                } else {
                    let _ = write!(out, "{f}");
                }
            }
            Json::String(s) => write_string(s, out),
            Json::Array(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Json::Object(m) => {
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// Compact serialization (no whitespace); non-finite floats become `null`.
impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        self.write(&mut s);
        f.write_str(&s)
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7F}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

impl From<bool> for Json {
    fn from(v: bool) -> Json {
        Json::Bool(v)
    }
}

impl From<i32> for Json {
    fn from(v: i32) -> Json {
        Json::Int(v.into())
    }
}

impl From<i64> for Json {
    fn from(v: i64) -> Json {
        Json::Int(v)
    }
}

impl From<u32> for Json {
    fn from(v: u32) -> Json {
        Json::Int(v.into())
    }
}

impl From<u64> for Json {
    fn from(v: u64) -> Json {
        i64::try_from(v).map_or(Json::Float(v as f64), Json::Int)
    }
}

impl From<usize> for Json {
    fn from(v: usize) -> Json {
        Json::from(v as u64)
    }
}

impl From<f64> for Json {
    fn from(v: f64) -> Json {
        Json::Float(v)
    }
}

impl From<&str> for Json {
    fn from(v: &str) -> Json {
        Json::String(v.to_string())
    }
}

impl From<String> for Json {
    fn from(v: String) -> Json {
        Json::String(v)
    }
}

impl From<Vec<Json>> for Json {
    fn from(v: Vec<Json>) -> Json {
        Json::Array(v)
    }
}

impl<T: Into<Json>> From<Option<T>> for Json {
    fn from(v: Option<T>) -> Json {
        v.map_or(Json::Null, Into::into)
    }
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn error(&self, message: &str) -> ParseError {
        ParseError { offset: self.pos, message: message.to_string() }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.s.len() && matches!(self.s[self.pos], b' ' | b'\t' | b'\n' | b'\r') {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn expect_word(&mut self, word: &str, value: Json) -> Result<Json, ParseError> {
        if self.s[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.error("unknown literal"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, ParseError> {
        if depth > MAX_DEPTH {
            return Err(self.error("nesting too deep"));
        }
        match self.peek() {
            None => Err(self.error("unexpected end of text")),
            Some(b'n') => self.expect_word("null", Json::Null),
            Some(b't') => self.expect_word("true", Json::Bool(true)),
            Some(b'f') => self.expect_word("false", Json::Bool(false)),
            Some(b'"') => self.string().map(Json::String),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                self.skip_ws();
                if self.peek() == Some(b']') {
                    self.pos += 1;
                    return Ok(Json::Array(items));
                }
                loop {
                    self.skip_ws();
                    items.push(self.value(depth + 1)?);
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b']') => {
                            self.pos += 1;
                            return Ok(Json::Array(items));
                        }
                        _ => return Err(self.error("expected `,` or `]`")),
                    }
                }
            }
            Some(b'{') => {
                self.pos += 1;
                let mut obj = Json::Object(Vec::new());
                self.skip_ws();
                if self.peek() == Some(b'}') {
                    self.pos += 1;
                    return Ok(obj);
                }
                loop {
                    self.skip_ws();
                    if self.peek() != Some(b'"') {
                        return Err(self.error("expected a key"));
                    }
                    let key = self.string()?;
                    self.skip_ws();
                    if self.peek() != Some(b':') {
                        return Err(self.error("expected `:`"));
                    }
                    self.pos += 1;
                    self.skip_ws();
                    let v = self.value(depth + 1)?;
                    obj.insert(key, v);
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b'}') => {
                            self.pos += 1;
                            return Ok(obj);
                        }
                        _ => return Err(self.error("expected `,` or `}`")),
                    }
                }
            }
            Some(_) => Err(self.error("unexpected character")),
        }
    }

    fn number(&mut self) -> Result<Json, ParseError> {
        let start = self.pos;
        let digits = |p: &mut Parser| {
            let s = p.pos;
            while p.peek().is_some_and(|c| c.is_ascii_digit()) {
                p.pos += 1;
            }
            p.pos > s
        };
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                digits(self);
            }
            _ => return Err(self.error("expected a digit")),
        }
        let mut integral = true;
        if self.peek() == Some(b'.') {
            integral = false;
            self.pos += 1;
            if !digits(self) {
                return Err(self.error("expected a digit after `.`"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            integral = false;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !digits(self) {
                return Err(self.error("expected a digit in the exponent"));
            }
        }
        // The slice is ASCII: digits, sign, point, exponent.
        let text = std::str::from_utf8(&self.s[start..self.pos]).unwrap_or("0");
        if integral {
            if let Ok(i) = text.parse::<i64>() {
                return Ok(Json::Int(i));
            }
        }
        text.parse::<f64>().map(Json::Float).map_err(|_| ParseError { offset: start, message: "invalid number".into() })
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let h = self.s.get(self.pos..self.pos + 4).ok_or_else(|| self.error("truncated \\u escape"))?;
        let h = std::str::from_utf8(h).map_err(|_| self.error("invalid \\u escape"))?;
        let v = u32::from_str_radix(h, 16).map_err(|_| self.error("invalid \\u escape"))?;
        self.pos += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.pos += 1; // opening quote
        let mut out = String::new();
        loop {
            // Copy a run of plain characters at once (the input is valid UTF-8, and the
            // run stops only on ASCII bytes).
            let run = self.pos;
            while self.peek().is_some_and(|c| c != b'"' && c != b'\\' && c >= 0x20) {
                self.pos += 1;
            }
            out.push_str(std::str::from_utf8(&self.s[run..self.pos]).unwrap_or(""));
            match self.peek() {
                None => return Err(self.error("unterminated string")),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let c = self.peek().ok_or_else(|| self.error("unterminated escape"))?;
                    self.pos += 1;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let c = if (0xD800..0xDC00).contains(&hi) && self.s[self.pos..].starts_with(b"\\u") {
                                let save = self.pos;
                                self.pos += 2;
                                let lo = self.hex4()?;
                                if (0xDC00..0xE000).contains(&lo) {
                                    char::from_u32(0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00))
                                } else {
                                    self.pos = save;
                                    None
                                }
                            } else {
                                char::from_u32(hi)
                            };
                            // A lone surrogate becomes U+FFFD rather than an error.
                            out.push(c.unwrap_or('\u{FFFD}'));
                        }
                        _ => {
                            self.pos -= 1;
                            return Err(self.error("unknown escape"));
                        }
                    }
                }
                Some(_) => return Err(self.error("control character in a string")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(text: &str) -> String {
        Json::parse(text).unwrap_or_else(|e| panic!("{text}: {e}")).to_string()
    }

    #[test]
    fn values() {
        assert_eq!(Json::parse(" null ").unwrap(), Json::Null);
        assert_eq!(Json::parse("true").unwrap(), Json::Bool(true));
        assert_eq!(Json::parse("-0").unwrap(), Json::Int(0));
        assert_eq!(Json::parse("42").unwrap(), Json::Int(42));
        assert_eq!(Json::parse("9223372036854775807").unwrap(), Json::Int(i64::MAX));
        assert_eq!(Json::parse("9223372036854775808").unwrap(), Json::Float(9223372036854775808.0));
        assert_eq!(Json::parse("1.5e2").unwrap(), Json::Float(150.0));
        assert_eq!(Json::parse("-2.5E-3").unwrap(), Json::Float(-0.0025));
        assert_eq!(
            round_trip(r#"{"a":[1,2.5,"x",{"b":null}],"c":false}"#),
            r#"{"a":[1,2.5,"x",{"b":null}],"c":false}"#
        );
        assert_eq!(round_trip("[ ]"), "[]");
        assert_eq!(round_trip("{ }"), "{}");
        // Duplicate keys: the last one wins, the first position is kept.
        assert_eq!(round_trip(r#"{"a":1,"b":2,"a":3}"#), r#"{"a":3,"b":2}"#);
    }

    #[test]
    fn strings_and_unicode() {
        let s = Json::parse(r#""a\"b\\c\/d\b\f\n\r\t\u00e9\u20AC\ud83d\ude00 é 😀""#).unwrap();
        assert_eq!(s.as_str(), Some("a\"b\\c/d\u{8}\u{C}\n\r\té€😀 é 😀"));
        assert_eq!(Json::from("a\"b\\\n\u{1}é😀").to_string(), r#""a\"b\\\n\u0001é😀""#);
        // Lone surrogates are replaced, not rejected.
        assert_eq!(Json::parse(r#""\ud800x""#).unwrap().as_str(), Some("\u{FFFD}x"));
        assert_eq!(Json::parse(r#""\ud800\u0041""#).unwrap().as_str(), Some("\u{FFFD}A"));
        assert_eq!(Json::parse(r#""\udc00""#).unwrap().as_str(), Some("\u{FFFD}"));
    }

    #[test]
    fn errors() {
        for bad in [
            "",
            "nul",
            "[1,]",
            "{\"a\" 1}",
            "{a:1}",
            "[1 2]",
            "01",
            "1.",
            ".5",
            "-",
            "1e",
            "\"abc",
            "\"\\x\"",
            "\"\u{1}\"",
            "[1]x",
            "\"\\u12\"",
            "{\"a\":1,}",
            "tru",
        ] {
            assert!(Json::parse(bad).is_err(), "{bad:?} accepted");
        }
        assert_eq!(Json::parse("[1, ?]").unwrap_err().offset, 4);
        // Deep nesting is an error, not a stack overflow.
        let deep = "[".repeat(100_000);
        assert!(Json::parse(&deep).unwrap_err().message.contains("deep"));
        let ok = format!("{}{}", "[".repeat(200), "]".repeat(200));
        assert!(Json::parse(&ok).is_ok());
    }

    #[test]
    fn numbers_serialize_as_valid_json() {
        for (v, s) in [
            (Json::Float(1.0), "1"),
            (Json::Float(0.5), "0.5"),
            (Json::Float(1e300), "1e300"),
            (Json::Float(-2.5e-9), "-2.5e-9"),
            (Json::Float(f64::NAN), "null"),
            (Json::Float(f64::INFINITY), "null"),
            (Json::from(u64::MAX), "1.8446744073709552e19"),
            (Json::from(-7i64), "-7"),
        ] {
            assert_eq!(v.to_string(), s);
            assert!(Json::parse(&v.to_string()).is_ok());
        }
    }

    #[test]
    fn accessors_and_builders() {
        let mut v = Json::object([("a", Json::from(1)), ("b", Json::object([("c", Json::from("x"))]))]);
        assert_eq!(v.path(&["b", "c"]).and_then(Json::as_str), Some("x"));
        assert_eq!(v.get("a").and_then(Json::as_u32), Some(1));
        assert_eq!(v.get("zz"), None);
        v.insert("a", Json::from(vec![Json::from(true)]));
        assert_eq!(v.get("a").and_then(|a| a.at(0)).and_then(Json::as_bool), Some(true));
        assert_eq!(Json::Float(3.0).as_i64(), Some(3));
        assert_eq!(Json::Float(3.5).as_i64(), None);
        assert_eq!(Json::from(-1).as_u32(), None);
        assert_eq!(Json::from(None::<i32>), Json::Null);
        let mut n = Json::Null;
        n.insert("k", Json::from("v"));
        assert_eq!(n.to_string(), r#"{"k":"v"}"#);
    }
}

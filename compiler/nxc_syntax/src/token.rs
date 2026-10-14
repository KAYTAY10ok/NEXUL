//! Tokens produced by the lexer.

use crate::source::Span;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    /// The token is the first of its line (used to place "missing" diagnostics).
    pub line_start: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Ident(String),
    /// Integer literal value (always fits in `u64`; larger literals are diagnosed).
    Int(u128),
    Float(f64),
    Char(char),
    Str(Box<StrLit>),

    // Keywords.
    And,
    As,
    Break,
    Const,
    Continue,
    Else,
    Enum,
    Extern,
    False,
    Fn,
    For,
    If,
    Impl,
    Import,
    In,
    Indirect,
    Inout,
    Let,
    Match,
    Module,
    Move,
    Not,
    Or,
    Owned,
    Pub,
    Return,
    SelfValue,
    SelfType,
    Static,
    Struct,
    Trait,
    True,
    Type,
    Unsafe,
    Var,
    Where,
    While,
    Dyn,

    // Operators.
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    PlusWrap,
    MinusWrap,
    StarWrap,
    Amp,
    Pipe,
    Caret,
    Tilde,
    Shl,
    Shr,
    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    PlusWrapEq,
    MinusWrapEq,
    StarWrapEq,
    AmpEq,
    PipeEq,
    CaretEq,
    ShlEq,
    ShrEq,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,

    // Delimiters and punctuation.
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Dot,
    DotDot,
    DotDotEq,
    Colon,
    Semi,
    Arrow,
    FatArrow,
    Question,
    At,

    /// A significant line break (see spec §1.7).
    Newline,
    Eof,
}

/// A string literal, split into literal text and interpolated expressions.
#[derive(Clone, Debug, PartialEq)]
pub struct StrLit {
    pub parts: Vec<StrPart>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Text(String),
    Interp { tokens: Vec<Token>, spec: Option<FormatSpec>, span: Span },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FormatKind {
    LowerHex,
    UpperHex,
    Octal,
    Binary,
    Exponent,
}

/// `[[fill]align][+][0][width][.precision][type]`
#[derive(Clone, Debug, PartialEq)]
pub struct FormatSpec {
    pub fill: char,
    pub align: Option<Align>,
    pub plus: bool,
    pub zero: bool,
    pub width: Option<u32>,
    pub precision: Option<u32>,
    pub kind: Option<FormatKind>,
    pub span: Span,
}

pub fn keyword(s: &str) -> Option<TokenKind> {
    use TokenKind::*;
    Some(match s {
        "and" => And,
        "as" => As,
        "break" => Break,
        "const" => Const,
        "continue" => Continue,
        "else" => Else,
        "enum" => Enum,
        "extern" => Extern,
        "false" => False,
        "fn" => Fn,
        "for" => For,
        "if" => If,
        "impl" => Impl,
        "import" => Import,
        "in" => In,
        "indirect" => Indirect,
        "inout" => Inout,
        "let" => Let,
        "match" => Match,
        "module" => Module,
        "move" => Move,
        "not" => Not,
        "or" => Or,
        "owned" => Owned,
        "pub" => Pub,
        "return" => Return,
        "self" => SelfValue,
        "Self" => SelfType,
        "static" => Static,
        "struct" => Struct,
        "trait" => Trait,
        "true" => True,
        "type" => Type,
        "unsafe" => Unsafe,
        "var" => Var,
        "where" => Where,
        "while" => While,
        "dyn" => Dyn,
        _ => return None,
    })
}

/// Keywords reserved for future versions of the language.
pub const RESERVED: &[&str] = &[
    "async", "await", "class", "defer", "interface", "loop", "macro", "ref", "try", "union", "yield",
];

impl TokenKind {
    /// Can a statement end with this token? (spec §1.7, rule 2)
    pub fn can_end_statement(&self) -> bool {
        use TokenKind::*;
        matches!(
            self,
            Ident(_)
                | Int(_)
                | Float(_)
                | Char(_)
                | Str(_)
                | True
                | False
                | SelfValue
                | SelfType
                | Return
                | Break
                | Continue
                | RParen
                | RBracket
                | RBrace
                | Question
        )
    }

    pub fn is_terminator(&self) -> bool {
        matches!(self, TokenKind::Newline | TokenKind::Semi)
    }

    /// Human-readable description used in "expected X, found Y" messages.
    pub fn describe(&self) -> String {
        use TokenKind::*;
        match self {
            Ident(name) => format!("identifier `{}`", name),
            Int(_) => "integer literal".into(),
            Float(_) => "float literal".into(),
            Char(_) => "character literal".into(),
            Str(_) => "string literal".into(),
            Newline => "end of line".into(),
            Eof => "end of file".into(),
            other => format!("`{}`", other),
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TokenKind::*;
        let s = match self {
            Ident(n) => return write!(f, "{}", n),
            Int(v) => return write!(f, "{}", v),
            Float(v) => return write!(f, "{}", v),
            Char(c) => return write!(f, "{:?}", c),
            Str(_) => "\"...\"",
            And => "and",
            As => "as",
            Break => "break",
            Const => "const",
            Continue => "continue",
            Else => "else",
            Enum => "enum",
            Extern => "extern",
            False => "false",
            Fn => "fn",
            For => "for",
            If => "if",
            Impl => "impl",
            Import => "import",
            In => "in",
            Indirect => "indirect",
            Inout => "inout",
            Let => "let",
            Match => "match",
            Module => "module",
            Move => "move",
            Not => "not",
            Or => "or",
            Owned => "owned",
            Pub => "pub",
            Return => "return",
            SelfValue => "self",
            SelfType => "Self",
            Static => "static",
            Struct => "struct",
            Trait => "trait",
            True => "true",
            Type => "type",
            Unsafe => "unsafe",
            Var => "var",
            Where => "where",
            While => "while",
            Dyn => "dyn",
            Plus => "+",
            Minus => "-",
            Star => "*",
            Slash => "/",
            Percent => "%",
            PlusWrap => "+%",
            MinusWrap => "-%",
            StarWrap => "*%",
            Amp => "&",
            Pipe => "|",
            Caret => "^",
            Tilde => "~",
            Shl => "<<",
            Shr => ">>",
            Eq => "=",
            PlusEq => "+=",
            MinusEq => "-=",
            StarEq => "*=",
            SlashEq => "/=",
            PercentEq => "%=",
            PlusWrapEq => "+%=",
            MinusWrapEq => "-%=",
            StarWrapEq => "*%=",
            AmpEq => "&=",
            PipeEq => "|=",
            CaretEq => "^=",
            ShlEq => "<<=",
            ShrEq => ">>=",
            EqEq => "==",
            NotEq => "!=",
            Lt => "<",
            Le => "<=",
            Gt => ">",
            Ge => ">=",
            LParen => "(",
            RParen => ")",
            LBracket => "[",
            RBracket => "]",
            LBrace => "{",
            RBrace => "}",
            Comma => ",",
            Dot => ".",
            DotDot => "..",
            DotDotEq => "..=",
            Colon => ":",
            Semi => ";",
            Arrow => "->",
            FatArrow => "=>",
            Question => "?",
            At => "@",
            Newline => "end of line",
            Eof => "end of file",
        };
        f.write_str(s)
    }
}

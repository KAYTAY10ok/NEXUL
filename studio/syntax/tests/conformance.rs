//! Agreement with the lexer of the compiler (`nxc_syntax`, read only), token by token, on
//! every `.nx` file of the repository that it lexes without diagnostic.
//!
//! If the language changes (a keyword, an operator, a literal form), this test fails until
//! the highlighting follows.

mod common;

use common::*;
use nexul_studio_syntax::{nexul, tokens, Kind};
use nxc_syntax::token::{StrPart, Token as CToken, TokenKind as K};
use nxc_syntax::{lexer, DiagSink, SourceMap};
use std::collections::HashMap;
use std::ops::Range;

/// Lexical errors that the highlighting flags as [`Kind::Invalid`]: lone carriage return,
/// bidirectional or other control character, unknown escape, invalid format spec, empty
/// interpolation, malformed character literal, bad digits or suffix, unexpected character.
/// The others are not flagged on purpose (see the README): unterminated comment, string or
/// interpolation (text being typed), misplaced `_` in a number, reserved keyword used as a
/// name (shown as a keyword), literal too large.
const SHOWN: &[&str] = &["E0002", "E0003", "E0004", "E0007", "E0009", "E0014", "E0015", "E0016", "E0017"];

/// What the compiler says about one range of the text.
struct Expected {
    /// Single tokens (names, keywords, numbers, operators) and the classes allowed for them.
    atoms: Vec<(Range<usize>, &'static [Kind])>,
    /// String and character literals.
    literals: Vec<Range<usize>>,
}

fn range(t: &CToken) -> Range<usize> {
    t.span.lo as usize..t.span.hi as usize
}

fn allowed(kind: &K) -> &'static [Kind] {
    match kind {
        K::Ident(_) => &[Kind::Variable, Kind::Function, Kind::Type, Kind::Constant],
        K::Int(_) | K::Float(_) => &[Kind::Number],
        K::True | K::False => &[Kind::Constant],
        K::SelfType => &[Kind::Type],
        K::If | K::Else | K::Match | K::While | K::For | K::Return | K::Break | K::Continue => &[Kind::Control],
        K::And
        | K::As
        | K::Const
        | K::Dyn
        | K::Enum
        | K::Extern
        | K::Fn
        | K::Impl
        | K::Import
        | K::In
        | K::Indirect
        | K::Inout
        | K::Let
        | K::Module
        | K::Move
        | K::Not
        | K::Or
        | K::Owned
        | K::Pub
        | K::SelfValue
        | K::Static
        | K::Struct
        | K::Trait
        | K::Type
        | K::Unsafe
        | K::Var
        | K::Where => &[Kind::Keyword],
        K::LParen
        | K::RParen
        | K::LBracket
        | K::RBracket
        | K::LBrace
        | K::RBrace
        | K::Comma
        | K::Dot
        | K::Colon
        | K::Semi
        | K::At => &[Kind::Punctuation],
        _ => &[Kind::Operator],
    }
}

fn is_word(kind: &K) -> bool {
    matches!(kind, K::Ident(_)) || kind.to_string().bytes().all(|c| c.is_ascii_alphabetic())
}

fn collect(toks: &[CToken], out: &mut Expected) {
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        match &t.kind {
            K::Newline | K::Eof => {}
            K::Str(s) => {
                out.literals.push(range(t));
                for part in &s.parts {
                    if let StrPart::Interp { tokens, .. } = part {
                        collect(tokens, out);
                    }
                }
            }
            K::Char(_) => out.literals.push(range(t)),
            // `@name` is one attribute token.
            K::At if toks.get(i + 1).is_some_and(|n| n.span.lo == t.span.hi && is_word(&n.kind)) => {
                out.atoms.push((t.span.lo as usize..toks[i + 1].span.hi as usize, &[Kind::Attribute]));
                i += 1;
            }
            kind => out.atoms.push((range(t), allowed(kind))),
        }
        i += 1;
    }
}

#[test]
fn agrees_with_the_compiler_lexer() {
    let files = nx_files();
    let (mut checked, mut skipped, mut atoms_checked, mut errors_checked) = (0, Vec::new(), 0, 0);
    for path in &files {
        let text = read(path);
        // Edition 1 (ADR 0010) is not highlighted yet: its files are checked once it is.
        if text.lines().take(3).any(|l| l.trim() == "// edition: 1") {
            continue;
        }
        let mut sm = SourceMap::new();
        let file = sm.add(path.display().to_string(), text.clone());
        let diags = DiagSink::new();
        let ctoks = lexer::lex(&text, file, &diags);
        if !diags.is_empty() {
            // Diagnostics tests with lexical errors on purpose: the errors that the
            // highlighting shows are flagged where the compiler reports them.
            let mine = tokens("nexul", &text);
            for d in diags.take_sorted() {
                let Some(span) = d.primary_span() else { continue };
                if SHOWN.contains(&d.code) {
                    errors_checked += 1;
                    let (lo, hi) = (span.lo as usize, span.hi as usize);
                    assert!(
                        mine.iter()
                            .any(|t| t.kind == Kind::Invalid && t.range.start < hi.max(lo + 1) && lo < t.range.end),
                        "{}: {} at {:?} is not flagged",
                        path.display(),
                        d.code,
                        &text[lo..hi]
                    );
                }
            }
            skipped.push(path.strip_prefix(repo_root()).unwrap_or(path).display().to_string());
            continue;
        }
        let mut expected = Expected { atoms: Vec::new(), literals: Vec::new() };
        collect(&ctoks, &mut expected);
        let mine = tokens("nexul", &text);
        let by_range: HashMap<Range<usize>, Kind> = mine.iter().map(|t| (t.range.clone(), t.kind)).collect();
        let at = |r: &Range<usize>| {
            format!("{}:{}: {:?}", path.display(), text[..r.start].matches('\n').count() + 1, &text[r.clone()])
        };

        // Every token of the compiler is a token here, with a compatible class.
        for (r, kinds) in &expected.atoms {
            match by_range.get(r) {
                Some(k) => assert!(kinds.contains(k), "{}: {k:?}, expected one of {kinds:?}", at(r)),
                None => panic!("{}: no token with this range", at(r)),
            }
            atoms_checked += 1;
        }
        // Literals start and end with string tokens.
        for r in &expected.literals {
            assert_eq!(
                by_range.iter().find(|(m, _)| m.start == r.start).map(|x| *x.1),
                Some(Kind::String),
                "{}",
                at(r)
            );
            assert!(mine.iter().any(|t| t.range.end == r.end && t.kind == Kind::String), "{}", at(r));
        }
        // And every token here is one of the compiler's, a part of a literal, or a comment
        // outside of any token of the compiler.
        let atom_ranges: HashMap<&Range<usize>, ()> = expected.atoms.iter().map(|(r, _)| (r, ())).collect();
        let inside_literal = |r: &Range<usize>| expected.literals.iter().any(|l| l.start <= r.start && r.end <= l.end);
        let overlaps_compiler = |r: &Range<usize>| {
            expected.atoms.iter().map(|(a, _)| a).chain(&expected.literals).any(|a| a.start < r.end && r.start < a.end)
        };
        for t in &mine {
            let r = &t.range;
            let ok = match t.kind {
                Kind::Invalid => false,
                Kind::String | Kind::Escape | Kind::Interpolation => inside_literal(r),
                Kind::Comment | Kind::DocComment => !overlaps_compiler(r),
                _ => atom_ranges.contains_key(r),
            };
            assert!(ok, "{}: unexpected {:?} token", at(r), t.kind);
        }
        checked += 1;
    }
    eprintln!("{checked} files agree ({atoms_checked} tokens); {errors_checked} lexical errors flagged in {skipped:?}");
    assert!(checked >= 100, "only {checked} files checked");
    assert!(errors_checked >= 6, "only {errors_checked} lexical errors checked");
    // The files named by the coordination document are all checked.
    assert!(skipped.iter().all(|p| p.starts_with("tests") && p.contains("ui")), "{skipped:?}");
}

/// The word lists of the highlighter are the compiler's keywords, and nothing else.
#[test]
fn keyword_lists_match_the_compiler() {
    use nxc_syntax::token::{keyword, RESERVED};
    let words: Vec<&str> =
        nexul::KEYWORDS.iter().chain(nexul::CONTROL).chain(nexul::LITERAL_KEYWORDS).copied().chain(["Self"]).collect();
    for w in &words {
        assert!(keyword(w).is_some(), "`{w}` is not a keyword of the compiler");
    }
    // The 38 keywords of spec §1.4 (and of `TokenKind`), each listed once.
    let mut unique = words.clone();
    unique.sort();
    unique.dedup();
    assert_eq!((words.len(), unique.len()), (38, 38), "{words:?}");
    assert_eq!(nexul::RESERVED, RESERVED);
    for w in nexul::PRIMITIVE_TYPES {
        assert!(*w == "Self" || keyword(w).is_none(), "`{w}`");
    }
}

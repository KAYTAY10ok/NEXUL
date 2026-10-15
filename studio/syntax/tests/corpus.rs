//! The tokenizer on the NEXUL code of the repository, on text being typed, on random text,
//! and on a document of 100,000 lines.

mod common;

use common::*;
use nexul_studio_syntax::{tokens, Highlighter, Kind};
use std::time::Instant;

/// The files the coordination document names (`examples/*.nx`, `tests/run/*.nx`) and the
/// standard library are valid NEXUL: no text lost, nothing flagged invalid.
#[test]
fn valid_code_is_tokenized_without_error_or_loss() {
    let mut files = nx_files_in("examples");
    files.extend(nx_files_in("tests/run"));
    files.extend(nx_files().into_iter().filter(|p| p.starts_with(repo_root().join("lib"))));
    assert!(files.len() >= 40, "corpus not found: {} files", files.len());
    let mut total = 0;
    for path in &files {
        let text = read(path);
        let toks = tokens("nexul", &text);
        check_invariants(&text, &toks).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        // Edition 1 (ADR 0010) is not highlighted yet: its signs (`!`, `#`, `|>`) are invalid in 0.2.
        if text.lines().take(3).any(|l| l.trim() == "// edition: 1") {
            total += toks.len();
            continue;
        }
        let invalid = invalid_tokens(&text, &toks);
        assert!(invalid.is_empty(), "{}: invalid tokens {invalid:?}", path.display());
        assert!(!toks.is_empty(), "{}", path.display());
        total += toks.len();
    }
    eprintln!("{} files, {total} tokens", files.len());
}

/// Every `.nx` file, including the diagnostics tests (`tests/ui`), whose code is wrong on
/// purpose: the guarantees hold, and the incremental highlighter agrees with `tokens`.
#[test]
fn every_file_keeps_the_guarantees() {
    let files = nx_files();
    assert!(files.len() >= 100, "corpus not found: {} files", files.len());
    for path in &files {
        let text = read(path);
        let toks = tokens("nexul", &text);
        check_invariants(&text, &toks).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let h = Highlighter::new("nexul", &text);
        assert_eq!(h.text(), text);
        assert_eq!(h.tokens(), toks, "{}", path.display());
    }
}

/// Every prefix of real files, as when the code is typed from the start.
#[test]
fn every_prefix_of_the_examples() {
    for path in nx_files_in("examples") {
        let text = read(&path);
        let limit = text.len().min(4000);
        for cut in (0..=limit).filter(|&i| text.is_char_boundary(i)) {
            let prefix = &text[..cut];
            check_invariants(prefix, &tokens("nexul", prefix))
                .unwrap_or_else(|e| panic!("{} cut at {cut}: {e}", path.display()));
        }
    }
}

/// Typing a file character by character in the incremental highlighter, then deleting it.
#[test]
fn typing_in_the_highlighter() {
    let text = read(&repo_root().join("examples").join("calculatrice.nx"));
    let mut h = Highlighter::new("nexul", "");
    let mut typed = 0;
    for (i, ch) in text.char_indices() {
        let mut buf = [0u8; 4];
        h.replace(i..i, ch.encode_utf8(&mut buf)).expect("valid position");
        typed = i + ch.len_utf8();
        if i % 97 == 0 {
            assert_eq!(h.tokens(), tokens("nexul", &text[..typed]), "after {typed} bytes");
        }
    }
    assert_eq!(h.text(), text);
    assert_eq!(h.tokens(), tokens("nexul", &text));
    // Backspace from the end.
    while typed > 0 {
        let start = text[..typed].char_indices().last().map(|(i, _)| i).unwrap();
        h.replace(start..typed, "").expect("valid range");
        typed = start;
        if typed % 89 == 0 {
            assert_eq!(h.tokens(), tokens("nexul", &text[..typed]), "after deleting down to {typed}");
        }
    }
    assert_eq!(h.text(), "");
    assert!(h.tokens().is_empty());
}

/// Random text: no panic, the guarantees hold.
#[test]
fn random_text_keeps_the_guarantees() {
    let mut rng = Rng::new(0x5EED_1234);
    for case in 0..5000 {
        let text = random_text(&mut rng, 40);
        let toks = tokens("nexul", &text);
        check_invariants(&text, &toks).unwrap_or_else(|e| panic!("case {case} {text:?}: {e}"));
    }
}

/// Random edits in the incremental highlighter give the same tokens as a full
/// tokenization.
#[test]
fn random_edits_match_full_tokenization() {
    let mut rng = Rng::new(42);
    let base = read(&repo_root().join("examples").join("arbre.nx"));
    let mut h = Highlighter::new("nexul", &base);
    let mut text = base.clone();
    for step in 0..1500 {
        // A random range on character boundaries, replaced with random fragments.
        let mut a = rng.below(text.len() + 1);
        let mut b = (a + rng.below(30)).min(text.len());
        while !text.is_char_boundary(a) {
            a -= 1;
        }
        while !text.is_char_boundary(b) {
            b += 1;
        }
        let new = random_text(&mut rng, 4);
        h.replace(a..b, &new).expect("valid range");
        text.replace_range(a..b, &new);
        assert_eq!(h.text(), text, "step {step}");
        let full = tokens("nexul", &text);
        assert_eq!(h.tokens(), full, "step {step}: {text:?}");
        check_invariants(&text, &full).unwrap_or_else(|e| panic!("step {step}: {e}"));
    }
}

/// A document of more than 100,000 lines made of the valid code of the repository.
fn big_document() -> String {
    let mut corpus = String::new();
    let lib = repo_root().join("lib");
    let mut files = nx_files_in("examples");
    files.extend(nx_files_in("tests/run"));
    files.extend(nx_files().into_iter().filter(|p| p.starts_with(&lib)));
    for p in files {
        corpus.push_str(&read(&p));
        corpus.push('\n');
    }
    let mut doc = String::new();
    while doc.matches('\n').count() < 100_000 {
        doc.push_str(&corpus);
    }
    doc
}

#[test]
fn one_hundred_thousand_lines() {
    let doc = big_document();
    let lines = doc.matches('\n').count() + 1;
    let t0 = Instant::now();
    let toks = tokens("nexul", &doc);
    let full = t0.elapsed();
    check_invariants(&doc, &toks).unwrap();

    let t0 = Instant::now();
    let mut h = Highlighter::new("nexul", &doc);
    let build = t0.elapsed();

    // Typing in a line of the middle re-tokenizes that line only.
    let mid = doc[..doc.len() / 2].rfind('\n').unwrap() + 1;
    let t0 = Instant::now();
    let changed = h.replace(mid..mid, "x").unwrap();
    let keystroke = t0.elapsed();
    assert_eq!(changed.len(), 1, "{changed:?}");

    let after_keystroke = tokens("nexul", &h.text());
    assert_eq!(h.tokens(), after_keystroke);

    // Opening a block comment at the top re-tokenizes what follows (comments nest, so up to
    // the end of the document), and closing it again too.
    let t0 = Instant::now();
    let opened = h.replace(0..0, "/*").unwrap();
    let reopen = t0.elapsed();
    assert_eq!(opened.start, 0);
    assert!(opened.len() > lines / 2, "{opened:?}");
    assert!(h.line_tokens(1).iter().all(|t| t.kind == Kind::Comment));
    assert_eq!(h.tokens(), tokens("nexul", &h.text()));
    assert_eq!(h.replace(0..2, "").unwrap(), opened);
    assert_eq!(h.tokens(), after_keystroke);

    eprintln!(
        "{lines} lines, {} bytes, {} tokens: full {:?} ({:.0} MB/s), highlighter {build:?}, keystroke {keystroke:?}, \
         re-tokenize all {reopen:?}",
        doc.len(),
        toks.len(),
        full,
        doc.len() as f64 / full.as_secs_f64() / 1e6
    );
    if !cfg!(debug_assertions) {
        assert!(full.as_millis() < 500, "100,000 lines in {full:?}");
        assert!(keystroke.as_millis() < 20, "keystroke in {keystroke:?}");
    }
}

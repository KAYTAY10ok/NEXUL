//! Diagnostics: structured errors and warnings, and their textual rendering.
//!
//! The rendering format follows the long-standing convention of modern compilers:
//!
//! ```text
//! error[E0402]: use of moved value `s`
//!  --> main.nx:4:13
//!   |
//! 3 |     let t = s
//!   |             - value moved here
//! 4 |     println(s)
//!   |             ^ value used here after move
//!   |
//!   = help: ...
//! ```

use crate::source::{SourceMap, Span};
use std::cell::RefCell;
use std::fmt::Write;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
}

#[derive(Clone, Debug)]
pub struct Label {
    pub span: Span,
    pub message: String,
    pub primary: bool,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub level: Level,
    pub code: &'static str,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub helps: Vec<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            level: Level::Error,
            code,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            helps: Vec::new(),
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Diagnostic {
        Diagnostic { level: Level::Warning, ..Diagnostic::error(code, message) }
    }

    /// The main location of the problem.
    pub fn primary(mut self, span: Span, message: impl Into<String>) -> Diagnostic {
        self.labels.push(Label { span, message: message.into(), primary: true });
        self
    }

    /// Additional context ("value moved here").
    pub fn secondary(mut self, span: Span, message: impl Into<String>) -> Diagnostic {
        self.labels.push(Label { span, message: message.into(), primary: false });
        self
    }

    pub fn note(mut self, message: impl Into<String>) -> Diagnostic {
        self.notes.push(message.into());
        self
    }

    pub fn help(mut self, message: impl Into<String>) -> Diagnostic {
        self.helps.push(message.into());
        self
    }

    pub fn primary_span(&self) -> Option<Span> {
        self.labels.iter().find(|l| l.primary).map(|l| l.span)
    }
}

/// Collects diagnostics. Interior mutability lets every compiler pass report through a
/// shared reference.
#[derive(Default)]
pub struct DiagSink {
    list: RefCell<Vec<Diagnostic>>,
}

impl DiagSink {
    pub fn new() -> DiagSink {
        DiagSink { list: RefCell::new(Vec::new()) }
    }

    pub fn emit(&self, d: Diagnostic) {
        self.list.borrow_mut().push(d);
    }

    pub fn error_count(&self) -> usize {
        self.list.borrow().iter().filter(|d| d.level == Level::Error).count()
    }

    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }

    pub fn len(&self) -> usize {
        self.list.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.borrow().is_empty()
    }

    /// Removes and returns all diagnostics, sorted by position (errors and warnings
    /// interleaved in source order, which is what a reader expects).
    pub fn take_sorted(&self) -> Vec<Diagnostic> {
        let mut v = std::mem::take(&mut *self.list.borrow_mut());
        v.sort_by_key(|d| d.primary_span().map(|s| (s.file, s.lo)).unwrap_or((crate::FileId(u32::MAX), 0)));
        // Exact duplicates can arise when the same construct is analysed twice (for
        // example a generic body); report them once.
        let mut out: Vec<Diagnostic> = Vec::with_capacity(v.len());
        for d in v {
            let dup = out.iter().any(|o| {
                o.code == d.code && o.message == d.message && o.primary_span() == d.primary_span()
            });
            if !dup {
                out.push(d);
            }
        }
        out
    }
}

struct Style {
    color: bool,
}

impl Style {
    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{}m{}\x1b[0m", code, text)
        } else {
            text.to_string()
        }
    }
    fn bold(&self, t: &str) -> String {
        self.paint("1", t)
    }
    fn red(&self, t: &str) -> String {
        self.paint("1;31", t)
    }
    fn yellow(&self, t: &str) -> String {
        self.paint("1;33", t)
    }
    fn blue(&self, t: &str) -> String {
        self.paint("1;34", t)
    }
}

/// Prepares a source line for display: tabs become spaces, and invisible or
/// direction-changing characters become U+FFFD (one column, like the original).
fn expand_tabs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\t' => out.push_str("    "),
            '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => out.push('\u{FFFD}'),
            c if (c as u32) < 0x20 || c == '\u{7f}' => out.push('\u{FFFD}'),
            c => out.push(c),
        }
    }
    out
}

/// Display width of a prefix of a source line (tabs count as 4 columns).
fn display_width(s: &str) -> usize {
    s.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum()
}

/// Columns of a source line shown in a snippet; a longer line is shown as a window around
/// its first label, so that the size of the output stays proportional to the number of
/// diagnostics (a 300 KB generated line quoted by 20 000 warnings would weigh gigabytes).
const MAX_SHOWN_COLUMNS: usize = 160;
/// Columns kept before the first label of a long line.
const COLUMNS_BEFORE_LABEL: usize = 40;

/// The part of a displayed line (after `expand_tabs`) to show: the text, with `...`
/// where it is cut, the first column shown, and the shift of the columns of the labels
/// (the width of the leading `...`).
fn window(line: &str, first_label_col: usize) -> (String, usize, usize) {
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= MAX_SHOWN_COLUMNS {
        return (line.to_string(), 0, 0);
    }
    let start = first_label_col.saturating_sub(COLUMNS_BEFORE_LABEL).min(chars.len() - MAX_SHOWN_COLUMNS);
    let end = start + MAX_SHOWN_COLUMNS;
    let mut shown = String::new();
    if start > 0 {
        shown.push_str("...");
    }
    shown.extend(&chars[start..end]);
    if end < chars.len() {
        shown.push_str("...");
    }
    (shown, start, if start > 0 { 3 } else { 0 })
}

/// Renders one diagnostic to text.
pub fn render(d: &Diagnostic, sm: &SourceMap, color: bool) -> String {
    let st = Style { color };
    let mut out = String::new();
    let (word, paint): (&str, fn(&Style, &str) -> String) = match d.level {
        Level::Error => ("error", Style::red),
        Level::Warning => ("warning", Style::yellow),
    };
    let head = if d.code.is_empty() { word.to_string() } else { format!("{}[{}]", word, d.code) };
    let _ = writeln!(out, "{}{}", paint(&st, &head), st.bold(&format!(": {}", d.message)));

    // Group labels by file, keeping the file of the primary label first.
    let mut files: Vec<crate::FileId> = Vec::new();
    if let Some(p) = d.primary_span() {
        if !p.is_dummy() {
            files.push(p.file);
        }
    }
    for l in &d.labels {
        if !l.span.is_dummy() && !files.contains(&l.span.file) {
            files.push(l.span.file);
        }
    }

    // Width of the line-number gutter, shared by all snippets of this diagnostic.
    let mut max_line = 0;
    for l in &d.labels {
        if let Some(f) = sm.try_get(l.span.file) {
            max_line = max_line.max(f.line_col(l.span.lo).0);
        }
    }
    let gutter = max_line.to_string().len().max(1);
    let pad = " ".repeat(gutter);

    for file_id in files {
        let file = sm.get(file_id);
        let labels: Vec<&Label> = d.labels.iter().filter(|l| l.span.file == file_id).collect();
        let anchor = labels.iter().find(|l| l.primary).copied().unwrap_or(labels[0]);
        let (aline, acol) = file.line_col(anchor.span.lo);
        let _ = writeln!(out, "{}{} {}:{}:{}", pad, st.blue("-->"), file.name, aline, acol);
        let _ = writeln!(out, "{} {}", pad, st.blue("|"));

        // Lines to show, in order.
        let mut lines: Vec<usize> = labels.iter().map(|l| file.line_index(l.span.lo)).collect();
        lines.sort_unstable();
        lines.dedup();
        let mut prev: Option<usize> = None;
        for &li in &lines {
            if let Some(p) = prev {
                if li > p + 2 {
                    let _ = writeln!(out, "{}", st.blue("..."));
                } else if li == p + 2 {
                    // A single skipped line is cheaper to show than an ellipsis.
                    let (text, _, _) = window(expand_tabs(file.line_text(p + 1)).trim_end(), 0);
                    let _ = writeln!(out, "{} {} {}", st.blue(&format!("{:>w$}", p + 2, w = gutter)), st.blue("|"), text);
                }
            }
            let text = file.line_text(li);
            // Labels starting on this line: primary first, then by column.
            let mut on_line: Vec<&Label> = labels.iter().copied().filter(|l| file.line_index(l.span.lo) == li).collect();
            on_line.sort_by_key(|l| (!l.primary, l.span.lo));
            let line_start = file.line_start(li) as usize;
            let label_col = |l: &Label| display_width(file.text.get(line_start..(l.span.lo as usize).max(line_start)).unwrap_or(""));
            let first_col = on_line.iter().map(|l| label_col(l)).min().unwrap_or(0);
            let (shown, first_shown, shift) = window(expand_tabs(text).trim_end(), first_col);
            let _ = writeln!(out, "{} {} {}", st.blue(&format!("{:>w$}", li + 1, w = gutter)), st.blue("|"), shown);
            let last_shown = first_shown + MAX_SHOWN_COLUMNS;
            for l in on_line {
                let lo = (l.span.lo as usize).max(line_start);
                let line_end = line_start + text.len();
                let hi = (l.span.hi as usize).min(line_end).max(lo);
                // Columns in the window shown (a label outside it is drawn at its edge).
                let col_full = label_col(l);
                let end_full = col_full + display_width(file.text.get(lo..hi).unwrap_or(""));
                let col = col_full.clamp(first_shown, last_shown) - first_shown + shift;
                let width = (end_full.min(last_shown).saturating_sub(col_full.max(first_shown))).max(1);
                let ch = if l.primary { "^" } else { "-" };
                let marks = ch.repeat(width);
                let body = if l.message.is_empty() { marks } else { format!("{} {}", marks, l.message) };
                let painted = if l.primary { paint(&st, &body) } else { st.blue(&body) };
                let _ = writeln!(out, "{} {} {}{}", pad, st.blue("|"), " ".repeat(col), painted);
            }
            prev = Some(li);
        }
    }
    if !d.notes.is_empty() || !d.helps.is_empty() {
        if !d.labels.is_empty() {
            let _ = writeln!(out, "{} {}", pad, st.blue("|"));
        }
        for n in &d.notes {
            let _ = writeln!(out, "{} {} {}", pad, st.blue("="), st.bold("note:").to_string() + " " + n);
        }
        for h in &d.helps {
            let _ = writeln!(out, "{} {} {}", pad, st.blue("="), st.bold("help:").to_string() + " " + h);
        }
    }
    out
}

/// Renders every diagnostic followed by a summary line.
pub fn render_all(diags: &[Diagnostic], sm: &SourceMap, color: bool) -> String {
    let mut out = String::new();
    for d in diags {
        out.push_str(&render(d, sm, color));
        out.push('\n');
    }
    let errors = diags.iter().filter(|d| d.level == Level::Error).count();
    let warnings = diags.len() - errors;
    if errors > 0 {
        let st = Style { color };
        let msg = format!(
            "could not compile due to {} error{}{}",
            errors,
            if errors == 1 { "" } else { "s" },
            if warnings > 0 { format!("; {} warning{} emitted", warnings, if warnings == 1 { "" } else { "s" }) } else { String::new() }
        );
        out.push_str(&format!("{}{}\n", st.red("error"), st.bold(&format!(": {}", msg))));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_primary_and_secondary() {
        let mut sm = SourceMap::new();
        let f = sm.add("main.nx", "fn main() {\n    let t = s\n    println(s)\n}\n");
        let t = sm.get(f).text.clone();
        let first = t.find("= s").unwrap() as u32 + 2;
        let second = t.find("(s)").unwrap() as u32 + 1;
        let d = Diagnostic::error("E0402", "use of moved value `s`")
            .primary(Span::new(f, second, second + 1), "value used here after move")
            .secondary(Span::new(f, first, first + 1), "value moved here")
            .help("use `s.clone()`");
        let text = render(&d, &sm, false);
        let expected = "error[E0402]: use of moved value `s`\n --> main.nx:3:13\n  |\n2 |     let t = s\n  |             - value moved here\n3 |     println(s)\n  |             ^ value used here after move\n  |\n  = help: use `s.clone()`\n";
        assert_eq!(text, expected);
    }

    #[test]
    fn a_long_line_is_shown_as_a_window_around_the_label() {
        let mut sm = SourceMap::new();
        let line = format!("let v = [{}target, {}0]\n", "1, ".repeat(2000), "2, ".repeat(2000));
        let f = sm.add("main.nx", &line);
        let at = line.find("target").unwrap() as u32;
        let d = Diagnostic::error("E0202", "cannot find `target` in this scope").primary(Span::new(f, at, at + 6), "not found");
        let text = render(&d, &sm, false);
        let lines: Vec<&str> = text.lines().collect();
        // The source line is cut on both sides...
        assert!(lines[3].starts_with("1 | ...") && lines[3].ends_with("..."), "{}", lines[3]);
        assert!(lines[3].len() < 200, "{}", lines[3]);
        // ...and the marks stay under the name.
        let name_col = lines[3].find("target").unwrap();
        assert_eq!(lines[4].find('^'), Some(name_col), "\n{}\n{}", lines[3], lines[4]);
        assert!(lines[4].contains("^^^^^^ not found"));
    }
}

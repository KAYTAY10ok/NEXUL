//! The client against real language servers, when they are installed on the computer
//! (the test is skipped, with a message, otherwise).

mod common;

use common::*;
use nexul_studio_lsp::registry::{self, Detector};
use nexul_studio_lsp::*;
use std::time::{Duration, Instant};

/// Waiting for a real server to load a project can take long on a busy machine.
const LOAD: Duration = Duration::from_secs(180);

/// Repeats `f` until it gives a value (a server answers `null` or `ContentModified` while
/// it is still loading the project).
fn retry<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
    let t0 = Instant::now();
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(t0.elapsed() < LOAD, "{what}: no answer after {LOAD:?}");
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Sample without the standard library (its sources, `rust-src`, may not be installed).
const MAIN_RS: &str = "struct Compteur {
    valeur: i32,
}

impl Compteur {
    fn incrementer(&mut self) -> i32 {
        self.valeur += 1;
        self.valeur
    }
}

fn double(x: i32) -> i32 {
    x * 2
}

fn main() {
    let mut c = Compteur { valeur: 0 };
    let n = double(c.incrementer());
    let _ = double(n, 3);
}
";

#[test]
fn rust_analyzer() {
    let Some(server) = Detector::from_env().server(registry::server_spec("rust-analyzer").unwrap()) else {
        eprintln!("rust-analyzer is not installed: test skipped");
        return;
    };
    eprintln!("{} {:?} ({})", server.spec.name, server.version, server.command.program.display());
    let dir = TempDir::new();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    let main = dir.path().join("src").join("main.rs");
    std::fs::write(&main, MAIN_RS).unwrap();

    let options = StartOptions { init_timeout: LOAD, ..Default::default() };
    let (client, events) = Client::start(&server.command, dir.path(), options).unwrap();
    let caps = client.capabilities();
    eprintln!("server {:?} {:?}, positions {:?}", caps.server_name, caps.server_version, client.position_encoding());
    assert!(caps.hover && caps.completion && caps.definition && caps.references && caps.rename && caps.formatting);
    let enc = client.position_encoding();
    let lang = registry::language_for_path(&main).unwrap().lsp_language_id(&main);
    client.did_open(&main, lang, MAIN_RS).unwrap();

    // Native diagnostic: `double` takes one argument, not two (line 18).
    let t0 = Instant::now();
    let diagnostic = loop {
        match events.recv_timeout(LOAD.saturating_sub(t0.elapsed())) {
            Ok(Event::Diagnostics { path: Some(p), diagnostics, .. }) if p == main => {
                if let Some(d) = diagnostics.into_iter().find(|d| d.range.start.line == 18) {
                    break d;
                }
            }
            Ok(_) => {}
            Err(_) => panic!("no diagnostic on line 18 after {LOAD:?}"),
        }
    };
    eprintln!("diagnostic: {:?} {:?} {}", diagnostic.severity, diagnostic.code, diagnostic.message);
    assert_eq!(diagnostic.severity, Some(Severity::Error));

    // Hover and definition of `double` at its first call (line 17, column 12).
    let call = Position::new(17, 12);
    let hover = retry("hover", || client.hover(&main, call).ok()?.wait(T).ok().flatten());
    eprintln!("hover: {:?}", hover.contents);
    assert!(hover.contents.contains("fn double(x: i32) -> i32"), "{}", hover.contents);
    let defs = retry("definition", || client.definition(&main, call).ok()?.wait(T).ok().filter(|d| !d.is_empty()));
    assert_eq!(defs[0].path().as_deref(), Some(main.as_path()));
    assert_eq!(defs[0].range.start.line, 11);

    // References: the declaration and two calls.
    let refs = client.references(&main, call, true).unwrap().wait(T).unwrap();
    let mut lines: Vec<u32> = refs.iter().map(|l| l.range.start.line).collect();
    lines.sort();
    assert_eq!(lines, [11, 17, 18]);

    // Completion after `c.` (line 17), in front of the existing `incrementer()`: the
    // method of the user's type is proposed.
    let items = retry("completion", || {
        let c = client.completion(&main, Position::new(17, 21)).ok()?.wait(T).ok()?;
        (!c.items.is_empty()).then_some(c.items)
    });
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    eprintln!("completion: {labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("incrementer")), "{labels:?}");

    // Rename `double` everywhere.
    let edit = client.rename(&main, call, "doubler").unwrap().wait(T).unwrap();
    let file = edit.files.iter().find(|f| f.path().as_deref() == Some(main.as_path())).unwrap();
    let renamed = apply_edits(MAIN_RS, &file.edits, enc).unwrap();
    assert_eq!(renamed, MAIN_RS.replace("double", "doubler"));

    // Formatting (rustfmt): a badly formatted body comes back as it was.
    let messy = MAIN_RS.replace("    x * 2\n", "x*2\n");
    client.did_change(&main, &messy).unwrap();
    let edits = retry("formatting", || client.formatting(&main, FormattingOptions::default()).ok()?.wait(T).ok());
    assert_eq!(apply_edits(&messy, &edits, enc).as_deref(), Some(MAIN_RS));

    client.did_close(&main).unwrap();
    assert_eq!(client.shutdown(T), Some(0));
}

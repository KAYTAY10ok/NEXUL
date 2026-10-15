//! The client against the fake language server (`src/bin/fake_server.rs`).

mod common;

use common::*;
use nexul_studio_lsp::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn range(l1: u32, c1: u32, l2: u32, c2: u32) -> Range {
    Range::new(Position::new(l1, c1), Position::new(l2, c2))
}

#[test]
fn full_session() {
    let dir = TempDir::new();
    let options =
        StartOptions { configuration: Json::parse(r#"{"fake":{"answer":42}}"#).unwrap(), ..Default::default() };
    let (client, events) = Client::start(&fake(&[]), dir.path(), options).unwrap();

    // Capabilities announced by the server.
    let caps = client.capabilities();
    assert_eq!((caps.server_name.as_deref(), caps.server_version.as_deref()), (Some("fake-lsp-server"), Some("1.2.3")));
    assert_eq!(client.position_encoding(), PositionEncoding::Utf16);
    assert_eq!((caps.sync, caps.open_close, caps.save), (SyncKind::Incremental, true, Some(true)));
    assert!(caps.hover && caps.completion && caps.definition && caps.references && caps.rename && caps.formatting);
    assert_eq!(caps.completion_triggers, ["."]);
    wait_for(&events, |e| matches!(e, Event::Log { message, .. } if message == "fake server ready").then_some(()));

    // The parameters of `initialize`, and the answers to the server's own requests.
    let s = state(&client);
    let init = s.get("initialize").unwrap();
    assert_eq!(init.get("processId").and_then(Json::as_i64), Some(std::process::id() as i64));
    assert_eq!(init.get("rootUri").and_then(Json::as_str), Some(path_to_uri(dir.path()).as_str()));
    assert_eq!(init.path(&["clientInfo", "name"]).and_then(Json::as_str), Some("NEXUL Studio"));
    assert_eq!(s.path(&["replies", "9001"]).unwrap().to_string(), r#"[{"answer":42},42,null]"#);
    assert_eq!(s.path(&["replies", "9002", "code"]).and_then(Json::as_i64), Some(codes::METHOD_NOT_FOUND));

    // Open: diagnostics come back as events.
    let file = dir.path().join("main été.txt");
    let uri = path_to_uri(&file);
    client.did_open(&file, "plaintext", "let été = 1\nerror here\n").unwrap();
    assert_eq!(client.version(&file), Some(1));
    let d = wait_diagnostics(&events, &file, Some(1));
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].range, range(1, 0, 1, 5));
    assert_eq!(
        (d[0].severity, d[0].code.as_deref(), d[0].source.as_deref(), d[0].message.as_str()),
        (Some(Severity::Error), Some("E1"), Some("fake"), "found error")
    );

    // Hover: `été` is at UTF-16 columns 4 to 7.
    let h = client.hover(&file, Position::new(0, 5)).unwrap().wait(T).unwrap().unwrap();
    assert_eq!((h.contents.as_str(), h.markdown, h.range), ("**été**", true, Some(range(0, 4, 0, 7))));
    assert_eq!(client.hover(&file, Position::new(0, 3)).unwrap().wait(T).unwrap(), None);

    // Completion.
    let c = client.completion(&file, Position::new(0, 0)).unwrap().wait(T).unwrap();
    let labels: Vec<&str> = c.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, ["1", "error", "here", "let", "été"]);
    assert_eq!((c.incomplete, c.items[0].kind_name()), (false, "Variable"));

    // Incremental changes, applied one after the other by the server.
    let edit = |r: Range, t: &str| TextEdit { range: r, new_text: t.into() };
    client.did_change_incremental(&file, &[edit(range(0, 4, 0, 7), "😀x")]).unwrap();
    assert_eq!(wait_diagnostics(&events, &file, Some(2)).len(), 1);
    assert_eq!(state(&client).path(&["docs", &uri, "text"]).and_then(Json::as_str), Some("let 😀x = 1\nerror here\n"));
    client.did_change_incremental(&file, &[edit(range(1, 0, 1, 5), "fine"), edit(range(0, 6, 0, 7), "y")]).unwrap();
    assert!(wait_diagnostics(&events, &file, Some(3)).is_empty());
    assert_eq!(state(&client).path(&["docs", &uri, "text"]).and_then(Json::as_str), Some("let 😀y = 1\nfine here\n"));

    // Whole text.
    client.did_change(&file, "a b a\n").unwrap();
    assert_eq!(client.version(&file), Some(4));
    wait_diagnostics(&events, &file, Some(4));

    // Definition, references, rename.
    let defs = client.definition(&file, Position::new(0, 4)).unwrap().wait(T).unwrap();
    assert_eq!(defs, vec![Location { uri: uri.clone(), range: range(0, 0, 0, 1) }]);
    assert_eq!(defs[0].path().as_deref(), Some(file.as_path()));
    let refs = client.references(&file, Position::new(0, 4), false).unwrap().wait(T).unwrap();
    assert_eq!(refs.iter().map(|l| l.range).collect::<Vec<_>>(), [range(0, 4, 0, 5)]);
    assert_eq!(client.references(&file, Position::new(0, 4), true).unwrap().wait(T).unwrap().len(), 2);
    let w = client.rename(&file, Position::new(0, 0), "zz").unwrap().wait(T).unwrap();
    assert_eq!(w.files.len(), 1);
    assert_eq!(w.files[0].path().as_deref(), Some(file.as_path()));
    assert_eq!(apply_edits("a b a\n", &w.files[0].edits, client.position_encoding()).as_deref(), Some("zz b zz\n"));
    // An error of the server reaches the caller.
    match client.rename(&file, Position::new(0, 1), "x").unwrap().wait(T) {
        Err(Error::Response { code: -32602, message, .. }) => assert_eq!(message, "no symbol here"),
        other => panic!("{other:?}"),
    }

    // Formatting.
    client.did_change(&file, "x  \ny\t\n").unwrap();
    let edits = client.formatting(&file, FormattingOptions::default()).unwrap().wait(T).unwrap();
    assert_eq!(apply_edits("x  \ny\t\n", &edits, client.position_encoding()).as_deref(), Some("x\ny\n"));

    // Save (the server wants the text), close.
    client.did_save(&file, Some("x  \ny\t\n")).unwrap();
    wait_for(&events, |e| matches!(e, Event::Log { message, .. } if message == "saved with text: true").then_some(()));
    client.did_close(&file).unwrap();
    assert!(wait_diagnostics(&events, &file, None).is_empty());
    assert_eq!(state(&client).get("docs"), Some(&Json::Object(vec![])));
    assert!(matches!(client.did_change(&file, ""), Err(Error::NotOpen(_))));
    assert!(matches!(client.did_save(&file, None), Err(Error::NotOpen(_))));
    assert_eq!(client.version(&file), None);

    // Proper shutdown: `shutdown` then `exit`, exit code 0.
    assert_eq!(client.shutdown(T), Some(0));
    wait_for(&events, |e| matches!(e, Event::Exited { code: Some(0), expected: true }).then_some(()));
    assert!(matches!(client.hover(&file, Position::default()), Err(Error::Closed)));
    assert!(matches!(client.request("x", Json::Null), Err(Error::Closed)));
}

#[test]
fn utf8_positions_when_the_server_chooses_them() {
    let dir = TempDir::new();
    let (client, _events) = start(&[("FAKE_LSP_ENCODING", "utf-8")], &dir);
    assert_eq!(client.position_encoding(), PositionEncoding::Utf8);
    let file = dir.path().join("a.txt");
    client.did_open(&file, "plaintext", "été x").unwrap();
    // `été` is 5 bytes.
    let h = client.hover(&file, Position::new(0, 2)).unwrap().wait(T).unwrap().unwrap();
    assert_eq!(h.range, Some(range(0, 0, 0, 5)));
    assert_eq!(state(&client).get("encoding").and_then(Json::as_str), Some("utf-8"));
    assert_eq!(client.shutdown(T), Some(0));
}

#[test]
fn timeouts_cancellation_and_polling() {
    let dir = TempDir::new();
    let (client, _events) = start(&[], &dir);
    let cancelled = |client: &Client| -> Vec<i64> {
        state(client).get("cancelled").and_then(Json::as_array).unwrap().iter().filter_map(Json::as_i64).collect()
    };

    // No answer in time: `Timeout`, and the server is told to cancel.
    let p = client.request("fake/slow", Json::Null).unwrap();
    let id = p.id();
    let t0 = Instant::now();
    assert!(matches!(p.wait(Duration::from_millis(200)), Err(Error::Timeout)));
    assert!(t0.elapsed() < Duration::from_secs(5));
    assert!(cancelled(&client).contains(&id));

    // Dropping a pending answer cancels it; so does `cancel`.
    let p = client.request("fake/slow", Json::Null).unwrap();
    let id = p.id();
    drop(p);
    assert!(cancelled(&client).contains(&id));
    let p = client.request("fake/slow", Json::Null).unwrap();
    let id = p.id();
    p.cancel();
    assert!(cancelled(&client).contains(&id));

    // Polling without blocking.
    let mut p = client.request("fake/slow", Json::object([("ms", Json::from(150))])).unwrap();
    assert!(p.try_take().is_none());
    let t0 = Instant::now();
    let answer = loop {
        if let Some(r) = p.try_take() {
            break r.unwrap();
        }
        assert!(t0.elapsed() < T);
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(answer, Json::from("slow done"));
    assert!(p.try_take().is_none());

    // The late answers to cancelled requests are ignored; the client still works.
    assert!(state(&client).get("docs").is_some());
    assert_eq!(client.shutdown(T), Some(0));
}

#[test]
fn server_crash() {
    let dir = TempDir::new();
    let (client, events) = start(&[], &dir);
    let pending = client.request("fake/slow", Json::Null).unwrap();
    let crash = client.request("fake/crash", Json::Null).unwrap();
    assert!(matches!(crash.wait(T), Err(Error::ServerExited)));
    // Every pending request fails, nothing waits forever.
    assert!(matches!(pending.wait(T), Err(Error::ServerExited)));
    wait_for(&events, |e| matches!(e, Event::Exited { code: Some(3), expected: false }).then_some(()));
    assert!(!client.is_running());
    assert!(matches!(client.request("fake/state", Json::Null), Err(Error::ServerExited)));
    let file = dir.path().join("a.txt");
    assert!(matches!(client.did_open(&file, "plaintext", ""), Err(Error::ServerExited)));
    // Shutting down a dead server is harmless.
    assert_eq!(client.shutdown(Duration::from_secs(1)), Some(3));
}

#[test]
fn malformed_messages_and_stderr() {
    let dir = TempDir::new();
    let (client, events) = start(&[], &dir);
    let answer = client.request("fake/garbage", Json::Null).unwrap().wait(T).unwrap();
    assert_eq!(answer, Json::from("after garbage"));
    wait_for(&events, |e| matches!(e, Event::Malformed(_)).then_some(()));
    client.request("fake/stderr", Json::Null).unwrap().wait(T).unwrap();
    wait_for(&events, |e| matches!(e, Event::Stderr(l) if l == "fake stderr line").then_some(()));
    assert_eq!(client.shutdown(T), Some(0));
}

#[test]
fn errors() {
    let dir = TempDir::new();
    // A program that does not exist.
    let missing = ServerCommand::new(dir.path().join("missing-server.exe"), &[]);
    assert!(matches!(Client::start(&missing, dir.path(), StartOptions::default()), Err(Error::Spawn(_))));

    // A server that never answers `initialize`.
    let options = StartOptions { init_timeout: Duration::from_millis(300), ..Default::default() };
    let t0 = Instant::now();
    assert!(matches!(Client::start(&fake(&[("FAKE_LSP_MUTE", "1")]), dir.path(), options), Err(Error::Timeout)));
    assert!(t0.elapsed() < Duration::from_secs(10));

    // A server that wants whole texts.
    let (client, _events) = start(&[("FAKE_LSP_SYNC", "full")], &dir);
    assert_eq!(client.capabilities().sync, SyncKind::Full);
    let file = dir.path().join("a.txt");
    client.did_open(&file, "plaintext", "a").unwrap();
    let e = TextEdit { range: range(0, 0, 0, 1), new_text: "b".into() };
    assert!(matches!(client.did_change_incremental(&file, &[e]), Err(Error::Unsupported(_))));
    client.did_change(&file, "b").unwrap();
    assert_eq!(state(&client).path(&["docs", &path_to_uri(&file), "text"]).and_then(Json::as_str), Some("b"));
    // Opening an open document sends its text as a change.
    client.did_open(&file, "plaintext", "c").unwrap();
    assert_eq!(client.version(&file), Some(3));
    assert_eq!(state(&client).path(&["docs", &path_to_uri(&file), "version"]).and_then(Json::as_i64), Some(3));
    // An unknown method.
    match client.request("nothing/here", Json::Null).unwrap().wait(T) {
        Err(Error::Response { code, .. }) => assert_eq!(code, codes::METHOD_NOT_FOUND),
        other => panic!("{other:?}"),
    }
    assert_eq!(client.shutdown(T), Some(0));
}

#[test]
fn dropping_the_client_stops_the_server() {
    let dir = TempDir::new();
    let (client, events) = start(&[], &dir);
    drop(client);
    // `exit` without `shutdown`: the fake server exits with code 1.
    wait_for(&events, |e| matches!(e, Event::Exited { code: Some(1), expected: true }).then_some(()));
}

#[test]
fn concurrent_requests() {
    let dir = TempDir::new();
    let (client, _events) = start(&[], &dir);
    let client = Arc::new(client);
    let file = dir.path().join("words.txt");
    let words = ["alpha", "beta", "gamma", "delta"];
    client.did_open(&file, "plaintext", &words.join(" ")).unwrap();
    let threads: Vec<_> = (0..8)
        .map(|t| {
            let (client, file) = (Arc::clone(&client), file.clone());
            std::thread::spawn(move || {
                for i in 0..50 {
                    let w = (t + i) % words.len();
                    let col = words[..w].iter().map(|x| x.len() + 1).sum::<usize>() as u32;
                    let h = client.hover(&file, Position::new(0, col)).unwrap().wait(T).unwrap().unwrap();
                    assert_eq!(h.contents, format!("**{}**", words[w]));
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(client.shutdown(T), Some(0));
}

/// Stopping a server started through a `.cmd` script kills the real server too (a child
/// of `cmd.exe`), not only the script.
#[cfg(windows)]
#[test]
fn killing_a_script_server_kills_the_whole_tree() {
    let dir = TempDir::new();
    let script = dir.path().join("mute.cmd");
    std::fs::write(&script, format!("@echo off\r\n\"{}\" %*\r\n", fake_exe().display())).unwrap();
    let pidfile = dir.path().join("pid.txt");
    let mut command = ServerCommand::new(&script, &[]);
    command.env =
        vec![("FAKE_LSP_MUTE".into(), "1".into()), ("FAKE_LSP_PIDFILE".into(), pidfile.display().to_string())];
    let options = StartOptions { init_timeout: Duration::from_millis(500), ..Default::default() };
    assert!(matches!(Client::start(&command, dir.path(), options), Err(Error::Timeout)));
    let pid = std::fs::read_to_string(&pidfile).unwrap();
    let alive = || {
        let out =
            std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).contains(pid.trim())
    };
    let t0 = Instant::now();
    while alive() {
        assert!(t0.elapsed() < Duration::from_secs(10), "the server behind the script survived");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Servers installed by npm are `.cmd` scripts on Windows.
#[cfg(windows)]
#[test]
fn a_cmd_script_as_server() {
    let dir = TempDir::new();
    let script = dir.path().join("fake server.cmd");
    std::fs::write(&script, format!("@echo off\r\n\"{}\" %*\r\n", fake_exe().display())).unwrap();
    let (client, _events) =
        Client::start(&ServerCommand::new(&script, &[]), dir.path(), StartOptions::default()).unwrap();
    assert_eq!(client.capabilities().server_name.as_deref(), Some("fake-lsp-server"));
    assert_eq!(client.shutdown(T), Some(0));
}

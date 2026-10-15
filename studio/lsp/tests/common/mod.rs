//! Helpers shared by the integration tests.

#![allow(dead_code)]

use nexul_studio_lsp::{Client, Diagnostic, Event, Json, ServerCommand, StartOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// Time limit of every wait in the tests (generous: the machine may be busy).
pub const T: Duration = Duration::from_secs(20);

/// The fake language server built with this crate.
pub fn fake_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fake-lsp-server"))
}

pub fn fake(env: &[(&str, &str)]) -> ServerCommand {
    let mut c = ServerCommand::new(fake_exe(), &[]);
    c.env = env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    c
}

/// A temporary folder, removed at the end of the test.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> TempDir {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "nexul-lsp-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn start(env: &[(&str, &str)], dir: &TempDir) -> (Client, Receiver<Event>) {
    Client::start(&fake(env), dir.path(), StartOptions::default()).expect("fake server starts")
}

/// Waits for an event accepted by `f`, skipping the others.
pub fn wait_for<R>(events: &Receiver<Event>, mut f: impl FnMut(&Event) -> Option<R>) -> R {
    let deadline = Instant::now() + T;
    let mut seen = Vec::new();
    loop {
        match events.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(e) => {
                if let Some(r) = f(&e) {
                    return r;
                }
                seen.push(e);
            }
            Err(_) => panic!("expected event not received; received: {seen:#?}"),
        }
    }
}

/// Waits for the diagnostics of `path` at `version`.
pub fn wait_diagnostics(events: &Receiver<Event>, path: &Path, version: Option<i64>) -> Vec<Diagnostic> {
    wait_for(events, |e| match e {
        Event::Diagnostics { path: Some(p), version: v, diagnostics, .. } if p == path && *v == version => {
            Some(diagnostics.clone())
        }
        _ => None,
    })
}

/// What the fake server has received.
pub fn state(client: &Client) -> Json {
    client.request("fake/state", Json::Null).unwrap().wait(T).unwrap()
}

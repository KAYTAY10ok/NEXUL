//! A fake language server for the tests of the client: it keeps the text of open
//! documents (applying full and incremental changes), publishes diagnostics for the word
//! `error`, and answers hover, completion, definition, references, rename and formatting
//! from the words of the document. Test methods: `fake/state` (what it received),
//! `fake/slow` (answers after `ms`, or only once cancelled), `fake/crash` (exits at once),
//! `fake/garbage` (writes an invalid message first), `fake/stderr`.
//!
//! Environment: `FAKE_LSP_ENCODING=utf-8` chooses UTF-8 positions if offered,
//! `FAKE_LSP_SYNC=full` asks for whole texts, `FAKE_LSP_MUTE=1` never answers,
//! `FAKE_LSP_PIDFILE=<file>` writes its process id to the file.
//! `fake-lsp-server --version` prints `fake-lsp-server 1.2.3`.

use nexul_studio_lsp::json::Json;
use nexul_studio_lsp::types::{
    apply_edits, offset_to_position, position_to_offset, Position, PositionEncoding, Range, TextEdit,
};
use nexul_studio_lsp::{codes, rpc};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Out = Arc<Mutex<io::Stdout>>;

fn send(out: &Out, msg: Json) {
    let mut o = out.lock().unwrap();
    rpc::write_message(&mut *o, &msg).ok();
}

fn respond(out: &Out, id: &Json, result: Json) {
    send(out, Json::object([("jsonrpc", Json::from("2.0")), ("id", id.clone()), ("result", result)]));
}

fn respond_error(out: &Out, id: &Json, code: i64, message: &str) {
    let error = Json::object([("code", Json::from(code)), ("message", Json::from(message))]);
    send(out, Json::object([("jsonrpc", Json::from("2.0")), ("id", id.clone()), ("error", error)]));
}

fn notify(out: &Out, method: &str, params: Json) {
    send(out, Json::object([("jsonrpc", Json::from("2.0")), ("method", Json::from(method)), ("params", params)]));
}

struct Doc {
    version: i64,
    text: String,
}

struct Server {
    out: Out,
    enc: PositionEncoding,
    full_sync: bool,
    docs: BTreeMap<String, Doc>,
    shutdown: bool,
    init_params: Json,
    /// Answers of the client to our requests, by id.
    replies: BTreeMap<i64, Json>,
    cancelled: BTreeSet<i64>,
    /// `fake/slow` requests waiting for their cancellation.
    slow: BTreeSet<i64>,
    saved: Vec<Json>,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Byte ranges of the words of `text`.
fn words(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices().chain([(text.len(), ' ')]) {
        match (is_word(c), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    out
}

impl Server {
    fn range(&self, text: &str, a: usize, b: usize) -> Json {
        Range::new(offset_to_position(text, a, self.enc), offset_to_position(text, b, self.enc)).to_json()
    }

    fn publish(&self, uri: &str) {
        let (diagnostics, version) = match self.docs.get(uri) {
            Some(doc) => {
                let d: Vec<Json> = words(&doc.text)
                    .into_iter()
                    .filter(|&(a, b)| &doc.text[a..b] == "error")
                    .map(|(a, b)| {
                        Json::object([
                            ("range", self.range(&doc.text, a, b)),
                            ("severity", Json::from(1)),
                            ("code", Json::from("E1")),
                            ("source", Json::from("fake")),
                            ("message", Json::from("found error")),
                        ])
                    })
                    .collect();
                (d, Json::from(doc.version))
            }
            None => (Vec::new(), Json::Null),
        };
        notify(
            &self.out,
            "textDocument/publishDiagnostics",
            Json::object([("uri", Json::from(uri)), ("version", version), ("diagnostics", Json::from(diagnostics))]),
        );
    }

    /// The document and the byte range of the word at the position of `params`.
    fn word_at(&self, params: &Json) -> Option<(String, usize, usize)> {
        let uri = params.path(&["textDocument", "uri"])?.as_str()?;
        let pos = Position::from_json(params.get("position")?)?;
        let doc = self.docs.get(uri)?;
        let off = position_to_offset(&doc.text, pos, self.enc);
        words(&doc.text).into_iter().find(|&(a, b)| a <= off && off < b).map(|(a, b)| (uri.to_string(), a, b))
    }

    fn occurrences(&self, uri: &str, word: &str) -> Vec<(usize, usize)> {
        let text = &self.docs[uri].text;
        words(text).into_iter().filter(|&(a, b)| &text[a..b] == word).collect()
    }

    fn request(&mut self, id: &Json, method: &str, params: &Json) {
        match method {
            "initialize" => {
                self.init_params = params.clone();
                let offered = params.path(&["capabilities", "general", "positionEncodings"]);
                let utf8 = std::env::var("FAKE_LSP_ENCODING").as_deref() == Ok("utf-8")
                    && offered.and_then(Json::as_array).is_some_and(|a| a.iter().any(|e| e.as_str() == Some("utf-8")));
                self.enc = if utf8 { PositionEncoding::Utf8 } else { PositionEncoding::Utf16 };
                let caps = Json::parse(&format!(
                    r#"{{"positionEncoding":"{}","textDocumentSync":{{"openClose":true,"change":{},"save":{{"includeText":true}}}},
                    "hoverProvider":true,"completionProvider":{{"triggerCharacters":["."]}},"definitionProvider":true,
                    "referencesProvider":{{}},"renameProvider":true,"documentFormattingProvider":true}}"#,
                    self.enc.name(),
                    if self.full_sync { 1 } else { 2 }
                ))
                .unwrap();
                let info = Json::object([("name", Json::from("fake-lsp-server")), ("version", Json::from("1.2.3"))]);
                respond(&self.out, id, Json::object([("capabilities", caps), ("serverInfo", info)]));
            }
            "shutdown" => {
                self.shutdown = true;
                respond(&self.out, id, Json::Null);
            }
            "textDocument/hover" => {
                let result = match self.word_at(params) {
                    Some((uri, a, b)) => {
                        let text = &self.docs[&uri].text;
                        let contents = Json::object([
                            ("kind", Json::from("markdown")),
                            ("value", Json::from(format!("**{}**", &text[a..b]))),
                        ]);
                        Json::object([("contents", contents), ("range", self.range(text, a, b))])
                    }
                    None => Json::Null,
                };
                respond(&self.out, id, result);
            }
            "textDocument/completion" => {
                let uri = params.path(&["textDocument", "uri"]).and_then(Json::as_str).unwrap_or("");
                let mut labels = BTreeSet::new();
                if let Some(doc) = self.docs.get(uri) {
                    for (a, b) in words(&doc.text) {
                        labels.insert(doc.text[a..b].to_string());
                    }
                }
                let items: Vec<Json> = labels
                    .into_iter()
                    .map(|l| Json::object([("label", Json::from(l)), ("kind", Json::from(6))]))
                    .collect();
                respond(
                    &self.out,
                    id,
                    Json::object([("isIncomplete", Json::from(false)), ("items", Json::from(items))]),
                );
            }
            "textDocument/definition" => {
                let result = match self.word_at(params) {
                    Some((uri, a, b)) => {
                        let text = &self.docs[&uri].text;
                        let (x, y) = self.occurrences(&uri, &text[a..b])[0];
                        Json::object([("uri", Json::from(uri.as_str())), ("range", self.range(text, x, y))])
                    }
                    None => Json::Null,
                };
                respond(&self.out, id, result);
            }
            "textDocument/references" => {
                let with_decl = params.path(&["context", "includeDeclaration"]).and_then(Json::as_bool).unwrap_or(true);
                let result = match self.word_at(params) {
                    Some((uri, a, b)) => {
                        let text = &self.docs[&uri].text;
                        let occ = self.occurrences(&uri, &text[a..b]);
                        let skip = if with_decl { 0 } else { 1 };
                        Json::from(
                            occ.into_iter()
                                .skip(skip)
                                .map(|(x, y)| {
                                    Json::object([("uri", Json::from(uri.as_str())), ("range", self.range(text, x, y))])
                                })
                                .collect::<Vec<_>>(),
                        )
                    }
                    None => Json::Null,
                };
                respond(&self.out, id, result);
            }
            "textDocument/rename" => {
                let new_name = params.get("newName").and_then(Json::as_str).unwrap_or("").to_string();
                match self.word_at(params) {
                    Some((uri, a, b)) => {
                        let text = &self.docs[&uri].text;
                        let edits: Vec<Json> = self
                            .occurrences(&uri, &text[a..b])
                            .into_iter()
                            .map(|(x, y)| {
                                Json::object([
                                    ("range", self.range(text, x, y)),
                                    ("newText", Json::from(new_name.as_str())),
                                ])
                            })
                            .collect();
                        let changes = Json::object([(uri.as_str(), Json::from(edits))]);
                        respond(&self.out, id, Json::object([("changes", changes)]));
                    }
                    None => respond_error(&self.out, id, -32602, "no symbol here"),
                }
            }
            "textDocument/formatting" => {
                // Removes the blanks at the end of lines.
                let uri = params.path(&["textDocument", "uri"]).and_then(Json::as_str).unwrap_or("");
                let mut edits = Vec::new();
                if let Some(doc) = self.docs.get(uri) {
                    let mut start = 0;
                    for line in doc.text.split('\n') {
                        let kept = line.trim_end_matches([' ', '\t']).len();
                        if kept < line.len() {
                            edits.push(Json::object([
                                ("range", self.range(&doc.text, start + kept, start + line.len())),
                                ("newText", Json::from("")),
                            ]));
                        }
                        start += line.len() + 1;
                    }
                }
                respond(&self.out, id, Json::from(edits));
            }
            "fake/state" => {
                let docs = Json::object(self.docs.iter().map(|(u, d)| {
                    (
                        u.as_str(),
                        Json::object([("version", Json::from(d.version)), ("text", Json::from(d.text.as_str()))]),
                    )
                }));
                let replies = Json::object(self.replies.iter().map(|(k, v)| (k.to_string(), v.clone())));
                let cancelled = Json::from(self.cancelled.iter().map(|&c| Json::from(c)).collect::<Vec<_>>());
                respond(
                    &self.out,
                    id,
                    Json::object([
                        ("docs", docs),
                        ("initialize", self.init_params.clone()),
                        ("replies", replies),
                        ("cancelled", cancelled),
                        ("saved", Json::from(self.saved.clone())),
                        ("encoding", Json::from(self.enc.name())),
                    ]),
                );
            }
            "fake/slow" => match params.get("ms").and_then(Json::as_i64) {
                Some(ms) => {
                    let (out, id) = (Arc::clone(&self.out), id.clone());
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(ms as u64));
                        respond(&out, &id, Json::from("slow done"));
                    });
                }
                None => {
                    self.slow.insert(id.as_i64().unwrap_or(-1));
                }
            },
            "fake/crash" => std::process::exit(3),
            "fake/garbage" => {
                let mut o = self.out.lock().unwrap();
                o.write_all(b"Content-Length: 9\r\n\r\n{garbage}").ok();
                o.flush().ok();
                drop(o);
                respond(&self.out, id, Json::from("after garbage"));
            }
            "fake/stderr" => {
                eprintln!("fake stderr line");
                respond(&self.out, id, Json::Null);
            }
            _ => respond_error(&self.out, id, codes::METHOD_NOT_FOUND, "unknown method"),
        }
    }

    fn notification(&mut self, method: &str, params: &Json) {
        let uri = params.path(&["textDocument", "uri"]).and_then(Json::as_str).unwrap_or("").to_string();
        match method {
            "initialized" => {
                // Requests to the client, which must answer them.
                let items =
                    Json::parse(r#"{"items":[{"section":"fake"},{"section":"fake.answer"},{"section":"none"}]}"#)
                        .unwrap();
                send(
                    &self.out,
                    Json::object([
                        ("jsonrpc", Json::from("2.0")),
                        ("id", Json::from(9001)),
                        ("method", Json::from("workspace/configuration")),
                        ("params", items),
                    ]),
                );
                send(
                    &self.out,
                    Json::object([
                        ("jsonrpc", Json::from("2.0")),
                        ("id", Json::from(9002)),
                        ("method", Json::from("fake/unknownServerRequest")),
                    ]),
                );
                notify(
                    &self.out,
                    "window/logMessage",
                    Json::object([("type", Json::from(3)), ("message", Json::from("fake server ready"))]),
                );
            }
            "exit" => std::process::exit(if self.shutdown { 0 } else { 1 }),
            "$/cancelRequest" => {
                let id = params.get("id").and_then(Json::as_i64).unwrap_or(-1);
                self.cancelled.insert(id);
                if self.slow.remove(&id) {
                    respond_error(&self.out, &Json::from(id), codes::REQUEST_CANCELLED, "cancelled");
                }
            }
            "textDocument/didOpen" => {
                let d = params.get("textDocument").unwrap();
                let text = d.get("text").and_then(Json::as_str).unwrap_or("").to_string();
                let version = d.get("version").and_then(Json::as_i64).unwrap_or(0);
                self.docs.insert(uri.clone(), Doc { version, text });
                self.publish(&uri);
            }
            "textDocument/didChange" => {
                let version = params.path(&["textDocument", "version"]).and_then(Json::as_i64).unwrap_or(0);
                let enc = self.enc;
                if let Some(doc) = self.docs.get_mut(&uri) {
                    for change in params.get("contentChanges").and_then(Json::as_array).cloned().unwrap_or_default() {
                        let text = change.get("text").and_then(Json::as_str).unwrap_or("").to_string();
                        match change.get("range").and_then(Range::from_json) {
                            Some(range) => {
                                doc.text = apply_edits(&doc.text, &[TextEdit { range, new_text: text }], enc).unwrap();
                            }
                            None => doc.text = text,
                        }
                    }
                    doc.version = version;
                }
                self.publish(&uri);
            }
            "textDocument/didSave" => {
                self.saved.push(params.clone());
                let with_text = params.get("text").is_some();
                notify(
                    &self.out,
                    "window/logMessage",
                    Json::object([
                        ("type", Json::from(4)),
                        ("message", Json::from(format!("saved with text: {with_text}"))),
                    ]),
                );
            }
            "textDocument/didClose" => {
                self.docs.remove(&uri);
                self.publish(&uri);
            }
            _ => {}
        }
    }
}

fn main() {
    if std::env::args().any(|a| a == "--version") {
        println!("fake-lsp-server 1.2.3");
        return;
    }
    if let Some(file) = std::env::var_os("FAKE_LSP_PIDFILE") {
        std::fs::write(file, std::process::id().to_string()).ok();
    }
    let mute = std::env::var("FAKE_LSP_MUTE").is_ok();
    let mut server = Server {
        out: Arc::new(Mutex::new(io::stdout())),
        enc: PositionEncoding::Utf16,
        full_sync: std::env::var("FAKE_LSP_SYNC").as_deref() == Ok("full"),
        docs: BTreeMap::new(),
        shutdown: false,
        init_params: Json::Null,
        replies: BTreeMap::new(),
        cancelled: BTreeSet::new(),
        slow: BTreeSet::new(),
        saved: Vec::new(),
    };
    let mut input = BufReader::new(io::stdin());
    while let Ok(Some(msg)) = rpc::read_message(&mut input) {
        let Ok(msg) = msg else { continue };
        if mute {
            continue;
        }
        let method = msg.get("method").and_then(Json::as_str).map(str::to_string);
        let params = msg.get("params").cloned().unwrap_or(Json::Null);
        match (method, msg.get("id")) {
            (Some(m), Some(id)) => server.request(&id.clone(), &m, &params),
            (Some(m), None) => server.notification(&m, &params),
            (None, Some(id)) => {
                // An answer of the client to one of our requests.
                let v = msg.get("result").or_else(|| msg.get("error")).cloned().unwrap_or(Json::Null);
                server.replies.insert(id.as_i64().unwrap_or(-1), v);
            }
            (None, None) => {}
        }
    }
    // Input closed without `exit`.
    std::process::exit(if server.shutdown { 0 } else { 2 });
}

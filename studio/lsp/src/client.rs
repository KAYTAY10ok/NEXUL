//! The client: a language server process, driven through threads and channels.
//!
//! [`Client::start`] launches the server and performs `initialize`. Requests return a
//! [`Pending`] answer, which the caller waits for (with a time limit) or polls; what the
//! server sends on its own (diagnostics, messages, its exit) arrives as [`Event`]s on a
//! channel. A reader thread owns the output of the server; requests are written from the
//! caller's thread. Everything is `Send + Sync`: an interface keeps the client in an `Arc`
//! and waits for answers on background threads.

use crate::json::Json;
use crate::rpc;
use crate::types::{
    path_to_uri, uri_to_path, Completions, Diagnostic, FormattingOptions, Hover, Location, Position, PositionEncoding,
    TextEdit, WorkspaceEdit,
};
use std::collections::HashMap;
use std::fmt;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

/// JSON-RPC error codes used here.
pub mod codes {
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const REQUEST_CANCELLED: i64 = -32800;
    pub const CONTENT_MODIFIED: i64 = -32801;
}

/// How to start a language server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerCommand {
    /// The executable (a `.cmd` or `.bat` file works on Windows).
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Extra environment variables.
    pub env: Vec<(String, String)>,
}

impl ServerCommand {
    pub fn new(program: impl Into<PathBuf>, args: &[&str]) -> ServerCommand {
        ServerCommand { program: program.into(), args: args.iter().map(|a| a.to_string()).collect(), env: Vec::new() }
    }
}

/// Options of [`Client::start`].
#[derive(Clone, Debug)]
pub struct StartOptions {
    /// `initializationOptions`, specific to each server (`null` by default).
    pub initialization_options: Json,
    /// Answers to the `workspace/configuration` requests of the server: an object whose
    /// members are the settings sections (`{"rust-analyzer": {...}}`); a dotted section
    /// (`"python.analysis"`) is looked up member by member. Missing sections are `null`.
    pub configuration: Json,
    /// Time allowed to `initialize` (30 s by default).
    pub init_timeout: Duration,
    /// Name and version sent in `clientInfo`.
    pub client_name: String,
    pub client_version: String,
}

impl Default for StartOptions {
    fn default() -> StartOptions {
        StartOptions {
            initialization_options: Json::Null,
            configuration: Json::Null,
            init_timeout: Duration::from_secs(30),
            client_name: "NEXUL Studio".into(),
            client_version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

/// Errors of the client.
#[derive(Debug)]
pub enum Error {
    /// The server could not be started.
    Spawn(std::io::Error),
    /// Writing to the server failed.
    Io(std::io::Error),
    /// The server answered with an error (`codes::REQUEST_CANCELLED` after a cancellation,
    /// `codes::CONTENT_MODIFIED` if the document changed meanwhile...).
    Response { code: i64, message: String, data: Json },
    /// No answer in the time given (the request has been cancelled).
    Timeout,
    /// The server has exited (or closed its output) without answering.
    ServerExited,
    /// The client has been shut down.
    Closed,
    /// The server does not announce this feature (method name).
    Unsupported(&'static str),
    /// The server's answer to `initialize` is not valid.
    Protocol(String),
    /// The document has not been opened with `did_open`.
    NotOpen(PathBuf),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Spawn(e) => write!(f, "the language server could not be started: {e}"),
            Error::Io(e) => write!(f, "communication with the language server failed: {e}"),
            Error::Response { code, message, .. } => write!(f, "the language server answered error {code}: {message}"),
            Error::Timeout => f.write_str("the language server did not answer in time"),
            Error::ServerExited => f.write_str("the language server has exited"),
            Error::Closed => f.write_str("the language server client is shut down"),
            Error::Unsupported(m) => write!(f, "the language server does not support {m}"),
            Error::Protocol(m) => write!(f, "invalid answer of the language server: {m}"),
            Error::NotOpen(p) => write!(f, "document not open: {}", p.display()),
        }
    }
}

impl std::error::Error for Error {}

/// Something the server sent on its own, or that happened to it.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// `textDocument/publishDiagnostics`: the complete list for one document (empty when
    /// its problems are gone).
    Diagnostics { uri: String, path: Option<PathBuf>, version: Option<i64>, diagnostics: Vec<Diagnostic> },
    /// `window/logMessage` (level 1 error, 2 warning, 3 information, 4 log).
    Log { level: u32, message: String },
    /// `window/showMessage`, or `window/showMessageRequest` (answered without choice).
    Message { level: u32, message: String },
    /// Any other notification (`$/progress`...).
    Notification { method: String, params: Json },
    /// A line written by the server on its standard error.
    Stderr(String),
    /// A message that is not valid JSON (the stream goes on).
    Malformed(String),
    /// The server's output has closed: it has exited. `expected` after [`Client::shutdown`].
    Exited { code: Option<i32>, expected: bool },
}

/// How the server wants document changes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SyncKind {
    /// No change notifications.
    None,
    /// The whole text at each change.
    Full,
    /// Changed ranges.
    Incremental,
}

/// What the server announced in its answer to `initialize`.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerCapabilities {
    /// The whole answer (`capabilities` and `serverInfo`).
    pub raw: Json,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    /// Unit of positions chosen by the server (UTF-16 unless it chose UTF-8).
    pub position_encoding: PositionEncoding,
    pub sync: SyncKind,
    /// `didOpen` and `didClose` are wanted.
    pub open_close: bool,
    /// `didSave` is wanted; `Some(true)` with the text.
    pub save: Option<bool>,
    pub hover: bool,
    pub completion: bool,
    /// Characters that should trigger completion (`.`, `::`...).
    pub completion_triggers: Vec<String>,
    pub definition: bool,
    pub references: bool,
    pub rename: bool,
    pub formatting: bool,
}

impl ServerCapabilities {
    fn from_initialize(result: &Json) -> Result<ServerCapabilities, Error> {
        let caps = result.get("capabilities").filter(|c| c.as_object().is_some());
        let caps = caps.ok_or_else(|| Error::Protocol("no `capabilities` object".into()))?;
        // A provider is announced by `true` or by an object of options.
        let has = |k: &str| caps.get(k).is_some_and(|v| v.as_bool().unwrap_or(v.as_object().is_some()));
        let kind = |v: Option<&Json>| match v.and_then(Json::as_i64) {
            Some(1) => SyncKind::Full,
            Some(2) => SyncKind::Incremental,
            _ => SyncKind::None,
        };
        let (sync, open_close, save) = match caps.get("textDocumentSync") {
            Some(Json::Object(_)) => {
                let s = caps.get("textDocumentSync").unwrap_or(&Json::Null);
                let save = match s.get("save") {
                    Some(Json::Bool(true)) => Some(false),
                    Some(o @ Json::Object(_)) => Some(o.get("includeText").and_then(Json::as_bool).unwrap_or(false)),
                    _ => None,
                };
                (kind(s.get("change")), s.get("openClose").and_then(Json::as_bool).unwrap_or(false), save)
            }
            Some(v) => {
                let k = kind(Some(v));
                (k, k != SyncKind::None, Some(false))
            }
            None => (SyncKind::None, false, None),
        };
        let info = |k: &str| result.path(&["serverInfo", k]).and_then(Json::as_str).map(str::to_string);
        Ok(ServerCapabilities {
            raw: result.clone(),
            server_name: info("name"),
            server_version: info("version"),
            position_encoding: caps
                .get("positionEncoding")
                .and_then(Json::as_str)
                .and_then(PositionEncoding::from_name)
                .unwrap_or_default(),
            sync,
            open_close,
            save,
            hover: has("hoverProvider"),
            completion: caps.get("completionProvider").is_some_and(|v| !v.is_null() && v.as_bool() != Some(false)),
            completion_triggers: caps
                .path(&["completionProvider", "triggerCharacters"])
                .and_then(Json::as_array)
                .map(|a| a.iter().filter_map(Json::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
            definition: has("definitionProvider"),
            references: has("referencesProvider"),
            rename: has("renameProvider"),
            formatting: has("documentFormattingProvider"),
        })
    }
}

/// The answer to a request, as delivered by the reader thread.
enum Reply {
    Result(Json),
    Error { code: i64, message: String, data: Json },
    Exited,
}

struct Inner {
    stdin: Mutex<Option<ChildStdin>>,
    child: Mutex<Child>,
    pending: Mutex<HashMap<i64, Sender<Reply>>>,
    next_id: AtomicI64,
    /// The output of the server is still open.
    alive: AtomicBool,
    /// `shutdown` has been sent (or the client dropped).
    closing: AtomicBool,
    /// Open documents: URI → version.
    docs: Mutex<HashMap<String, i32>>,
    events: Sender<Event>,
    configuration: Json,
    root_uri: String,
    root_name: String,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic while holding a lock must not make the client unusable.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Inner {
    fn write(&self, message: &Json) -> Result<(), Error> {
        let mut stdin = lock(&self.stdin);
        match stdin.as_mut() {
            Some(w) => rpc::write_message(w, message).map_err(Error::Io),
            None => Err(Error::Closed),
        }
    }

    fn notify(&self, method: &str, params: Json) -> Result<(), Error> {
        let mut msg = Json::object([("jsonrpc", Json::from("2.0")), ("method", Json::from(method))]);
        if !params.is_null() {
            msg.insert("params", params);
        }
        self.write(&msg)
    }

    fn request<T>(self: &Arc<Self>, method: &str, params: Json, convert: fn(&Json) -> T) -> Result<Pending<T>, Error> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err(Error::ServerExited);
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        lock(&self.pending).insert(id, tx);
        // The reader thread clears `alive` before failing the pending requests: checking
        // again after the insertion means no request can be left without an answer.
        if !self.alive.load(Ordering::SeqCst) {
            lock(&self.pending).remove(&id);
            return Err(Error::ServerExited);
        }
        let mut msg =
            Json::object([("jsonrpc", Json::from("2.0")), ("id", Json::from(id)), ("method", Json::from(method))]);
        if !params.is_null() {
            msg.insert("params", params);
        }
        if let Err(e) = self.write(&msg) {
            lock(&self.pending).remove(&id);
            return Err(e);
        }
        Ok(Pending { id, rx, inner: Arc::clone(self), convert, done: false })
    }

    fn cancel(&self, id: i64) {
        if lock(&self.pending).remove(&id).is_some() {
            let _ = self.notify("$/cancelRequest", Json::object([("id", Json::from(id))]));
        }
    }

    fn reply(&self, id: Json, result: Result<Json, (i64, &str)>) {
        let mut msg = Json::object([("jsonrpc", Json::from("2.0")), ("id", id)]);
        match result {
            Ok(v) => msg.insert("result", v),
            Err((code, message)) => {
                msg.insert("error", Json::object([("code", Json::from(code)), ("message", Json::from(message))]))
            }
        }
        let _ = self.write(&msg);
    }

    fn event(&self, e: Event) {
        // The receiver may have been dropped: events are then discarded.
        let _ = self.events.send(e);
    }

    /// Handles one message read from the server.
    fn dispatch(&self, msg: Json) {
        let method = msg.get("method").and_then(Json::as_str).map(str::to_string);
        let id = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(Json::Null);
        match (method, id) {
            (None, Some(id)) => {
                let Some(id) = id.as_i64() else { return };
                let Some(tx) = lock(&self.pending).remove(&id) else { return };
                let reply = match msg.get("error") {
                    Some(e) if !e.is_null() => Reply::Error {
                        code: e.get("code").and_then(Json::as_i64).unwrap_or(0),
                        message: e.get("message").and_then(Json::as_str).unwrap_or("").to_string(),
                        data: e.get("data").cloned().unwrap_or(Json::Null),
                    },
                    _ => Reply::Result(msg.get("result").cloned().unwrap_or(Json::Null)),
                };
                let _ = tx.send(reply);
            }
            (Some(method), Some(id)) => self.server_request(&method, id, &params),
            (Some(method), None) => self.notification(method, params),
            (None, None) => self.event(Event::Malformed(format!("message without method or id: {msg}"))),
        }
    }

    /// Requests from the server: answered here, so that no server waits forever.
    fn server_request(&self, method: &str, id: Json, params: &Json) {
        let result = match method {
            "workspace/configuration" => {
                let items = params.get("items").and_then(Json::as_array).map(Vec::as_slice).unwrap_or(&[]);
                let values = items
                    .iter()
                    .map(|item| match item.get("section").and_then(Json::as_str) {
                        Some(section) => {
                            let keys: Vec<&str> = section.split('.').collect();
                            self.configuration.path(&keys).cloned().unwrap_or(Json::Null)
                        }
                        None => self.configuration.clone(),
                    })
                    .collect();
                Ok(Json::Array(values))
            }
            "workspace/workspaceFolders" => Ok(Json::from(vec![Json::object([
                ("uri", Json::from(self.root_uri.as_str())),
                ("name", Json::from(self.root_name.as_str())),
            ])])),
            "client/registerCapability"
            | "client/unregisterCapability"
            | "window/workDoneProgress/create"
            | "workspace/semanticTokens/refresh"
            | "workspace/inlayHint/refresh"
            | "workspace/inlineValue/refresh"
            | "workspace/codeLens/refresh"
            | "workspace/diagnostic/refresh"
            | "workspace/foldingRange/refresh" => Ok(Json::Null),
            "window/showMessageRequest" => {
                self.event(Event::Message {
                    level: params.get("type").and_then(Json::as_u32).unwrap_or(3),
                    message: params.get("message").and_then(Json::as_str).unwrap_or("").to_string(),
                });
                Ok(Json::Null)
            }
            "workspace/applyEdit" => Ok(Json::object([
                ("applied", Json::from(false)),
                ("failureReason", Json::from("not supported by NEXUL Studio")),
            ])),
            "window/showDocument" => Ok(Json::object([("success", Json::from(false))])),
            _ => Err((codes::METHOD_NOT_FOUND, "method not supported by the client")),
        };
        self.reply(id, result);
    }

    fn notification(&self, method: String, params: Json) {
        let text = |k: &str| params.get(k).and_then(Json::as_str).unwrap_or("").to_string();
        let level = params.get("type").and_then(Json::as_u32).unwrap_or(4);
        let e = match method.as_str() {
            "textDocument/publishDiagnostics" => {
                let uri = text("uri");
                Event::Diagnostics {
                    path: uri_to_path(&uri),
                    version: params.get("version").and_then(Json::as_i64),
                    diagnostics: params
                        .get("diagnostics")
                        .and_then(Json::as_array)
                        .map(|a| a.iter().filter_map(Diagnostic::from_json).collect())
                        .unwrap_or_default(),
                    uri,
                }
            }
            "window/logMessage" => Event::Log { level, message: text("message") },
            "window/showMessage" => Event::Message { level, message: text("message") },
            _ => Event::Notification { method, params },
        };
        self.event(e);
    }

    /// Waits up to `timeout` for the process to end; its exit code if it did.
    fn wait_exit(&self, timeout: Duration) -> Option<Option<i32>> {
        let start = Instant::now();
        loop {
            if let Ok(Some(status)) = lock(&self.child).try_wait() {
                return Some(status.code());
            }
            if start.elapsed() >= timeout {
                return None;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn kill(&self) {
        kill_tree(&mut lock(&self.child));
    }
}

/// Kills a process and, on Windows, the processes it started: a server launched through
/// a `.cmd` script (npm) runs in a child of `cmd.exe`, which `Child::kill` alone would
/// leave running.
pub(crate) fn kill_tree(child: &mut Child) {
    if let Ok(None) = child.try_wait() {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let system =
                std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
            let _ = Command::new(system.join("System32").join("taskkill.exe"))
                .args(["/T", "/F", "/PID", &child.id().to_string()])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
        let _ = child.kill();
    }
    let _ = child.wait();
}

/// The reader thread: dispatches every message of the server until its output closes.
fn read_loop(inner: Arc<Inner>, stdout: impl Read) {
    let mut reader = BufReader::with_capacity(64 * 1024, stdout);
    loop {
        match rpc::read_message(&mut reader) {
            Ok(Some(Ok(msg))) => inner.dispatch(msg),
            Ok(Some(Err(e))) => inner.event(Event::Malformed(e.to_string())),
            Ok(None) => break,
            Err(e) => {
                inner.event(Event::Malformed(format!("broken message stream: {e}")));
                break;
            }
        }
    }
    inner.alive.store(false, Ordering::SeqCst);
    for (_, tx) in lock(&inner.pending).drain() {
        let _ = tx.send(Reply::Exited);
    }
    *lock(&inner.stdin) = None;
    let expected = inner.closing.load(Ordering::SeqCst);
    // The output may close slightly before the process ends.
    let code = inner.wait_exit(Duration::from_secs(2)).flatten();
    inner.event(Event::Exited { code, expected });
}

/// The answer to a request, to wait for or poll. Dropping it before the answer cancels
/// the request (`$/cancelRequest`): an editor drops the hover it no longer needs.
pub struct Pending<T> {
    id: i64,
    rx: Receiver<Reply>,
    inner: Arc<Inner>,
    convert: fn(&Json) -> T,
    done: bool,
}

impl<T> Pending<T> {
    /// The JSON-RPC id of the request.
    pub fn id(&self) -> i64 {
        self.id
    }

    fn finish(&mut self, reply: Reply) -> Result<T, Error> {
        self.done = true;
        match reply {
            Reply::Result(v) => Ok((self.convert)(&v)),
            Reply::Error { code, message, data } => Err(Error::Response { code, message, data }),
            Reply::Exited => Err(Error::ServerExited),
        }
    }

    /// Waits for the answer at most `timeout`; after that the request is cancelled and
    /// [`Error::Timeout`] returned.
    pub fn wait(mut self, timeout: Duration) -> Result<T, Error> {
        match self.rx.recv_timeout(timeout) {
            Ok(reply) => self.finish(reply),
            Err(RecvTimeoutError::Timeout) => {
                self.done = true;
                self.inner.cancel(self.id);
                Err(Error::Timeout)
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.done = true;
                Err(Error::ServerExited)
            }
        }
    }

    /// The answer if it has arrived, without waiting; `None` before (and after it has
    /// been taken once).
    pub fn try_take(&mut self) -> Option<Result<T, Error>> {
        if self.done {
            return None;
        }
        match self.rx.try_recv() {
            Ok(reply) => Some(self.finish(reply)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.done = true;
                Some(Err(Error::ServerExited))
            }
        }
    }

    /// Cancels the request.
    pub fn cancel(mut self) {
        self.done = true;
        self.inner.cancel(self.id);
    }
}

impl<T> Drop for Pending<T> {
    fn drop(&mut self) {
        if !self.done {
            self.inner.cancel(self.id);
        }
    }
}

impl<T> fmt::Debug for Pending<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pending").field("id", &self.id).field("done", &self.done).finish()
    }
}

/// A running language server.
pub struct Client {
    inner: Arc<Inner>,
    capabilities: ServerCapabilities,
    pid: u32,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client").field("pid", &self.pid).field("server", &self.capabilities.server_name).finish()
    }
}

fn client_capabilities() -> Json {
    let j = |s: &str| Json::parse(s).unwrap_or(Json::Null);
    Json::object([
        ("general", j(r#"{"positionEncodings":["utf-8","utf-16"],"markdown":{"parser":"NEXUL Studio"}}"#)),
        (
            "textDocument",
            j(r#"{
                "synchronization":{"dynamicRegistration":false,"didSave":true,"willSave":false,"willSaveWaitUntil":false},
                "publishDiagnostics":{"relatedInformation":false,"versionSupport":true},
                "hover":{"dynamicRegistration":false,"contentFormat":["markdown","plaintext"]},
                "completion":{"dynamicRegistration":false,"contextSupport":false,
                    "completionItem":{"snippetSupport":false,"documentationFormat":["markdown","plaintext"],
                        "insertReplaceSupport":true,"deprecatedSupport":true,"labelDetailsSupport":false}},
                "definition":{"dynamicRegistration":false,"linkSupport":true},
                "references":{"dynamicRegistration":false},
                "rename":{"dynamicRegistration":false,"prepareSupport":false},
                "formatting":{"dynamicRegistration":false}
            }"#),
        ),
        (
            "workspace",
            j(r#"{"configuration":true,"workspaceFolders":true,"applyEdit":false,
                "workspaceEdit":{"documentChanges":true,"resourceOperations":[]},
                "didChangeConfiguration":{"dynamicRegistration":false}}"#),
        ),
        ("window", j(r#"{"workDoneProgress":false,"showMessage":{},"showDocument":{"support":false}}"#)),
    ])
}

impl Client {
    /// Starts the server in `root` (the folder open in Studio), performs `initialize`
    /// and `initialized`, and returns the client with the channel of its events.
    pub fn start(
        command: &ServerCommand,
        root: &Path,
        options: StartOptions,
    ) -> Result<(Client, Receiver<Event>), Error> {
        let root = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
        let mut cmd = Command::new(&command.program);
        cmd.args(&command.args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        for (k, v) in &command.env {
            cmd.env(k, v);
        }
        if root.is_dir() {
            cmd.current_dir(&root);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn().map_err(Error::Spawn)?;
        let pid = child.id();
        let (stdin, stdout, stderr) = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (Some(stdin), Some(stdout), Some(stderr)) = (stdin, stdout, stderr) else {
            let _ = child.kill();
            return Err(Error::Spawn(std::io::Error::other("no standard streams")));
        };
        let (events, events_rx) = mpsc::channel();
        let root_uri = path_to_uri(&root);
        let root_name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| root_uri.clone());
        let inner = Arc::new(Inner {
            stdin: Mutex::new(Some(stdin)),
            child: Mutex::new(child),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicI64::new(1),
            alive: AtomicBool::new(true),
            closing: AtomicBool::new(false),
            docs: Mutex::new(HashMap::new()),
            events,
            configuration: options.configuration.clone(),
            root_uri: root_uri.clone(),
            root_name: root_name.clone(),
        });
        let reader = Arc::clone(&inner);
        thread::Builder::new()
            .name("lsp-reader".into())
            .spawn(move || read_loop(reader, stdout))
            .map_err(Error::Spawn)?;
        let err_events = inner.events.clone();
        thread::Builder::new()
            .name("lsp-stderr".into())
            .spawn(move || {
                let mut r = BufReader::new(stderr);
                let mut line = Vec::new();
                while let Ok(n) = r.read_until(b'\n', &mut line) {
                    if n == 0 {
                        break;
                    }
                    let text = String::from_utf8_lossy(&line).trim_end_matches(['\r', '\n']).to_string();
                    let _ = err_events.send(Event::Stderr(text));
                    line.clear();
                }
            })
            .map_err(Error::Spawn)?;

        let params = Json::object([
            ("processId", Json::from(std::process::id())),
            (
                "clientInfo",
                Json::object([
                    ("name", Json::from(options.client_name.as_str())),
                    ("version", Json::from(options.client_version.as_str())),
                ]),
            ),
            ("locale", Json::from("fr")),
            ("rootUri", Json::from(root_uri.as_str())),
            ("rootPath", Json::from(root.to_string_lossy().as_ref())),
            ("capabilities", client_capabilities()),
            ("initializationOptions", options.initialization_options.clone()),
            ("trace", Json::from("off")),
            (
                "workspaceFolders",
                Json::from(vec![Json::object([
                    ("uri", Json::from(root_uri.as_str())),
                    ("name", Json::from(root_name.as_str())),
                ])]),
            ),
        ]);
        let started = inner
            .request("initialize", params, |v| v.clone())
            .and_then(|p| p.wait(options.init_timeout))
            .and_then(|result| ServerCapabilities::from_initialize(&result))
            .and_then(|caps| inner.notify("initialized", Json::Object(Vec::new())).map(|_| caps));
        match started {
            Ok(capabilities) => Ok((Client { inner, capabilities, pid }, events_rx)),
            Err(e) => {
                inner.closing.store(true, Ordering::SeqCst);
                inner.kill();
                Err(e)
            }
        }
    }

    /// What the server announced.
    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    /// Unit of the `character` of positions exchanged with this server.
    pub fn position_encoding(&self) -> PositionEncoding {
        self.capabilities.position_encoding
    }

    /// Process id of the server.
    pub fn process_id(&self) -> u32 {
        self.pid
    }

    /// The server is still running (its output is open).
    pub fn is_running(&self) -> bool {
        self.inner.alive.load(Ordering::SeqCst)
    }

    /// Any request; the answer is the raw `result`.
    pub fn request(&self, method: &str, params: Json) -> Result<Pending<Json>, Error> {
        self.check_open()?;
        self.inner.request(method, params, |v| v.clone())
    }

    /// Any notification.
    pub fn notify(&self, method: &str, params: Json) -> Result<(), Error> {
        self.check_open()?;
        self.inner.notify(method, params)
    }

    fn check_open(&self) -> Result<(), Error> {
        if self.inner.closing.load(Ordering::SeqCst) {
            Err(Error::Closed)
        } else if !self.is_running() {
            Err(Error::ServerExited)
        } else {
            Ok(())
        }
    }

    fn document(path: &Path) -> Json {
        Json::object([("uri", Json::from(path_to_uri(path)))])
    }

    fn at(path: &Path, pos: Position) -> Json {
        Json::object([("textDocument", Self::document(path)), ("position", pos.to_json())])
    }

    /// The version of an open document (1 when opened, +1 at each change).
    pub fn version(&self, path: &Path) -> Option<i32> {
        lock(&self.inner.docs).get(&path_to_uri(path)).copied()
    }

    /// `textDocument/didOpen` (version 1). `language_id` is the LSP identifier of the
    /// language (`"rust"`, `"python"`...; see `registry`). Opening an open document sends
    /// its whole text as a change instead.
    pub fn did_open(&self, path: &Path, language_id: &str, text: &str) -> Result<(), Error> {
        self.check_open()?;
        let uri = path_to_uri(path);
        if lock(&self.inner.docs).contains_key(&uri) {
            return self.did_change(path, text);
        }
        lock(&self.inner.docs).insert(uri.clone(), 1);
        if !self.capabilities.open_close {
            return Ok(());
        }
        let doc = Json::object([
            ("uri", Json::from(uri)),
            ("languageId", Json::from(language_id)),
            ("version", Json::from(1)),
            ("text", Json::from(text)),
        ]);
        self.inner.notify("textDocument/didOpen", Json::object([("textDocument", doc)]))
    }

    fn next_version(&self, path: &Path) -> Result<(String, i32), Error> {
        let uri = path_to_uri(path);
        let mut docs = lock(&self.inner.docs);
        let v = docs.get_mut(&uri).ok_or_else(|| Error::NotOpen(path.to_path_buf()))?;
        *v += 1;
        Ok((uri, *v))
    }

    fn send_changes(&self, uri: String, version: i32, changes: Vec<Json>) -> Result<(), Error> {
        if self.capabilities.sync == SyncKind::None {
            return Ok(());
        }
        let doc = Json::object([("uri", Json::from(uri)), ("version", Json::from(version))]);
        self.inner.notify(
            "textDocument/didChange",
            Json::object([("textDocument", doc), ("contentChanges", Json::from(changes))]),
        )
    }

    /// `textDocument/didChange` with the whole new text: valid whatever the server's
    /// [`SyncKind`].
    pub fn did_change(&self, path: &Path, text: &str) -> Result<(), Error> {
        self.check_open()?;
        let (uri, version) = self.next_version(path)?;
        self.send_changes(uri, version, vec![Json::object([("text", Json::from(text))])])
    }

    /// `textDocument/didChange` with ranges: each edit applies to the text left by the
    /// previous ones (protocol rule). Only for a server in [`SyncKind::Incremental`]
    /// ([`Error::Unsupported`] otherwise: send the whole text with
    /// [`Client::did_change`]).
    pub fn did_change_incremental(&self, path: &Path, edits: &[TextEdit]) -> Result<(), Error> {
        self.check_open()?;
        if self.capabilities.sync != SyncKind::Incremental {
            return Err(Error::Unsupported("incremental document changes"));
        }
        let (uri, version) = self.next_version(path)?;
        let changes = edits
            .iter()
            .map(|e| Json::object([("range", e.range.to_json()), ("text", Json::from(e.new_text.as_str()))]));
        self.send_changes(uri, version, changes.collect())
    }

    /// `textDocument/didSave`; the text is sent if the server asks for it and it is given.
    pub fn did_save(&self, path: &Path, text: Option<&str>) -> Result<(), Error> {
        self.check_open()?;
        if !lock(&self.inner.docs).contains_key(&path_to_uri(path)) {
            return Err(Error::NotOpen(path.to_path_buf()));
        }
        let Some(include_text) = self.capabilities.save else {
            return Ok(());
        };
        let mut params = Json::object([("textDocument", Self::document(path))]);
        if let (true, Some(text)) = (include_text, text) {
            params.insert("text", Json::from(text));
        }
        self.inner.notify("textDocument/didSave", params)
    }

    /// `textDocument/didClose`.
    pub fn did_close(&self, path: &Path) -> Result<(), Error> {
        self.check_open()?;
        if lock(&self.inner.docs).remove(&path_to_uri(path)).is_none() {
            return Err(Error::NotOpen(path.to_path_buf()));
        }
        if !self.capabilities.open_close {
            return Ok(());
        }
        self.inner.notify("textDocument/didClose", Json::object([("textDocument", Self::document(path))]))
    }

    fn feature<T>(
        &self,
        supported: bool,
        method: &'static str,
        params: Json,
        convert: fn(&Json) -> T,
    ) -> Result<Pending<T>, Error> {
        self.check_open()?;
        if !supported {
            return Err(Error::Unsupported(method));
        }
        self.inner.request(method, params, convert)
    }

    /// `textDocument/hover`: `None` when there is nothing to show.
    pub fn hover(&self, path: &Path, pos: Position) -> Result<Pending<Option<Hover>>, Error> {
        self.feature(self.capabilities.hover, "textDocument/hover", Self::at(path, pos), Hover::from_json)
    }

    /// `textDocument/completion`.
    pub fn completion(&self, path: &Path, pos: Position) -> Result<Pending<Completions>, Error> {
        self.feature(
            self.capabilities.completion,
            "textDocument/completion",
            Self::at(path, pos),
            Completions::from_json,
        )
    }

    /// `textDocument/definition`.
    pub fn definition(&self, path: &Path, pos: Position) -> Result<Pending<Vec<Location>>, Error> {
        self.feature(
            self.capabilities.definition,
            "textDocument/definition",
            Self::at(path, pos),
            Location::list_from_json,
        )
    }

    /// `textDocument/references`.
    pub fn references(
        &self,
        path: &Path,
        pos: Position,
        include_declaration: bool,
    ) -> Result<Pending<Vec<Location>>, Error> {
        let mut params = Self::at(path, pos);
        params.insert("context", Json::object([("includeDeclaration", Json::from(include_declaration))]));
        self.feature(self.capabilities.references, "textDocument/references", params, Location::list_from_json)
    }

    /// `textDocument/rename`: the edits to apply, in every affected document.
    pub fn rename(&self, path: &Path, pos: Position, new_name: &str) -> Result<Pending<WorkspaceEdit>, Error> {
        let mut params = Self::at(path, pos);
        params.insert("newName", Json::from(new_name));
        self.feature(self.capabilities.rename, "textDocument/rename", params, WorkspaceEdit::from_json)
    }

    /// `textDocument/formatting`: edits that format the whole document.
    pub fn formatting(&self, path: &Path, options: FormattingOptions) -> Result<Pending<Vec<TextEdit>>, Error> {
        let params = Json::object([("textDocument", Self::document(path)), ("options", options.to_json())]);
        self.feature(self.capabilities.formatting, "textDocument/formatting", params, TextEdit::list_from_json)
    }

    /// Stops the server properly: `shutdown`, then `exit`, each waited for at most
    /// `timeout`; the process is killed if it is still running. Returns its exit code when
    /// known. Further calls on the client give [`Error::Closed`].
    pub fn shutdown(&self, timeout: Duration) -> Option<i32> {
        if !self.inner.closing.swap(true, Ordering::SeqCst) && self.is_running() {
            if let Ok(p) = self.inner.request("shutdown", Json::Null, |_| ()) {
                let _ = p.wait(timeout);
            }
            let _ = self.inner.notify("exit", Json::Null);
        }
        *lock(&self.inner.stdin) = None;
        match self.inner.wait_exit(timeout) {
            Some(code) => code,
            None => {
                self.inner.kill();
                None
            }
        }
    }
}

impl Drop for Client {
    /// A client dropped without [`Client::shutdown`] asks the server to exit and kills it
    /// after one second, without blocking the caller.
    fn drop(&mut self) {
        if self.inner.closing.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ = self.inner.notify("exit", Json::Null);
        *lock(&self.inner.stdin) = None;
        let inner = Arc::clone(&self.inner);
        let _ = thread::Builder::new().name("lsp-stop".into()).spawn(move || {
            if inner.wait_exit(Duration::from_secs(1)).is_none() {
                inner.kill();
            }
        });
    }
}

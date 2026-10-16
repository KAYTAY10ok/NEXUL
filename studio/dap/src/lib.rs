//! Debug Adapter Protocol (DAP) client and adapter discovery for NEXUL Studio.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug)]
pub enum DapError {
    Io(io::Error),
    Json(serde_json::Error),
    Timeout(String),
    AdapterExited,
    InvalidMessage(String),
}

impl std::fmt::Display for DapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "DAP I/O error: {error}"),
            Self::Json(error) => write!(f, "invalid DAP JSON: {error}"),
            Self::Timeout(command) => write!(f, "DAP request `{command}` timed out"),
            Self::AdapterExited => f.write_str("debug adapter exited before replying"),
            Self::InvalidMessage(message) => write!(f, "invalid DAP message: {message}"),
        }
    }
}

impl std::error::Error for DapError {}
impl From<io::Error> for DapError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<serde_json::Error> for DapError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterCommand {
    pub adapter_id: String,
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(OsString, OsString)>,
}

impl AdapterCommand {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        let program = program.into();
        Self {
            adapter_id: "nexul".into(),
            program,
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
        }
    }

    pub fn adapter_id(mut self, adapter_id: impl Into<String>) -> Self {
        self.adapter_id = adapter_id.into();
        self
    }

    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }
    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum AdapterKind {
    LldbDap,
    Debugpy,
    Netcoredbg,
    Node,
}

#[derive(Clone, Debug, Default)]
pub struct AdapterRegistry {
    adapters: HashMap<AdapterKind, AdapterCommand>,
}

impl AdapterRegistry {
    /// Detect installed adapters only. This function never downloads or installs tools.
    pub fn discover(nxc: Option<&Path>) -> Self {
        let mut registry = Self::default();
        if let Some(path) = find_lldb_dap(nxc) {
            registry.insert(
                AdapterKind::LldbDap,
                AdapterCommand::new(path).adapter_id("lldb"),
            );
        }
        if let Some(command) = find_debugpy() {
            registry.insert(AdapterKind::Debugpy, command);
        }
        if let Some(path) = find_executable(&["netcoredbg.exe", "netcoredbg"]) {
            registry.insert(
                AdapterKind::Netcoredbg,
                AdapterCommand::new(path).adapter_id("coreclr"),
            );
        }
        if let Some(command) = find_node_debug_adapter() {
            registry.insert(AdapterKind::Node, command);
        }
        registry
    }

    pub fn insert(&mut self, kind: AdapterKind, command: AdapterCommand) {
        self.adapters.insert(kind, command);
    }
    pub fn get(&self, kind: AdapterKind) -> Option<&AdapterCommand> {
        self.adapters.get(&kind)
    }
    pub fn available(&self, kind: AdapterKind) -> bool {
        self.adapters.contains_key(&kind)
    }
    pub fn iter(&self) -> impl Iterator<Item = (AdapterKind, &AdapterCommand)> {
        self.adapters.iter().map(|(kind, command)| (*kind, command))
    }
}

fn find_lldb_dap(nxc: Option<&Path>) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "lldb-dap.exe"
    } else {
        "lldb-dap"
    };
    let mut dirs = Vec::new();
    if let Some(root) = std::env::var_os("NEXUL_LLVM") {
        dirs.push(PathBuf::from(&root).join("bin"));
        dirs.push(PathBuf::from(root));
    }
    if let Some(nxc_dir) = nxc.and_then(Path::parent) {
        dirs.push(nxc_dir.join("llvm").join("bin"));
        dirs.push(nxc_dir.join("..").join("llvm").join("bin"));
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(dir) = executable.parent() {
            dirs.push(dir.join("llvm").join("bin"));
            dirs.push(dir.join("..").join("llvm").join("bin"));
        }
    }
    if cfg!(windows) {
        dirs.push(PathBuf::from(r"C:\Program Files\LLVM\bin"));
    }
    dirs.into_iter()
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
        .or_else(|| find_executable(&[name]))
}

fn find_debugpy() -> Option<AdapterCommand> {
    let (python, prefix): (PathBuf, Vec<OsString>) = if let Some(python) =
        find_executable(&["python.exe", "python3.exe", "python", "python3"])
    {
        (python, Vec::new())
    } else if cfg!(windows) {
        (
            find_executable(&["py.exe", "py"])?,
            vec![OsString::from("-3")],
        )
    } else {
        return None;
    };
    // Import probing checks whether the already-installed environment has debugpy.
    let probe = Command::new(&python)
        .args(&prefix)
        .args([
            "-c",
            "import os, debugpy; print(os.path.join(os.path.dirname(debugpy.__file__), 'adapter'))",
        ])
        .output()
        .ok()?;
    if !probe.status.success() {
        return None;
    }
    let adapter_path = String::from_utf8(probe.stdout).ok()?.trim().to_owned();
    if adapter_path.is_empty() || !Path::new(&adapter_path).exists() {
        return None;
    }
    Some(
        AdapterCommand::new(python)
            .adapter_id("python")
            .args(prefix)
            .arg(adapter_path),
    )
}

fn find_node_debug_adapter() -> Option<AdapterCommand> {
    if let Some(adapter) = find_executable(&[
        "js-debug-adapter.exe",
        "js-debug-adapter",
        "node-debug2.exe",
        "node-debug2",
    ]) {
        return Some(AdapterCommand::new(adapter).adapter_id("pwa-node"));
    }
    let node = find_executable(&["node.exe", "node"])?;
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        roots.push(PathBuf::from(&home).join(".vscode").join("extensions"));
        roots.push(
            PathBuf::from(&home)
                .join(".vscode-insiders")
                .join("extensions"),
        );
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(
            PathBuf::from(&local)
                .join("Programs")
                .join("Microsoft VS Code")
                .join("resources")
                .join("app")
                .join("extensions"),
        );
        roots.push(
            PathBuf::from(local)
                .join("Programs")
                .join("Microsoft VS Code Insiders")
                .join("resources")
                .join("app")
                .join("extensions"),
        );
    }
    let mut candidates = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
                if !name.starts_with("ms-vscode.js-debug") {
                    continue;
                }
                for relative in [
                    "out/src/dapDebugServer.js",
                    "dist/src/dapDebugServer.js",
                    "src/dapDebugServer.js",
                ] {
                    let adapter = path.join(relative);
                    if adapter.is_file() {
                        candidates.push(adapter);
                    }
                }
            }
        }
    }
    candidates.sort();
    candidates.pop().map(|adapter| {
        AdapterCommand::new(node)
            .adapter_id("pwa-node")
            .arg(adapter)
    })
}

fn find_executable(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let extensions: Vec<OsString> = if cfg!(windows) {
        std::env::var_os("PATHEXT")
            .unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"))
            .to_string_lossy()
            .split(';')
            .filter(|s| !s.is_empty())
            .map(OsString::from)
            .collect()
    } else {
        vec![OsString::new()]
    };
    for dir in std::env::split_paths(&path) {
        for name in names {
            let path = dir.join(name);
            if path.is_file() {
                return Some(path);
            }
            if Path::new(name).extension().is_none() {
                for extension in &extensions {
                    let candidate = dir.join(format!("{}{}", name, extension.to_string_lossy()));
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }
    None
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DapEvent {
    pub event: String,
    pub body: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DapResponse {
    pub request_seq: i64,
    pub command: String,
    pub success: bool,
    pub message: Option<String>,
    pub body: Value,
}

#[derive(Clone, Debug)]
enum PendingResult {
    Response(Value),
    AdapterExited,
    ReaderFailed(String),
}

#[derive(Default)]
struct PendingRequests {
    adapter_exited: bool,
    requests: HashMap<i64, Sender<PendingResult>>,
}

/// An already-sent request whose reply can be awaited after other DAP messages.
/// Dropping this handle removes its local response subscription.
#[must_use = "wait for the response, or drop the handle to abandon it"]
pub struct PendingRequest {
    sequence: i64,
    command: String,
    receiver: Receiver<PendingResult>,
    pending: Arc<Mutex<PendingRequests>>,
    completed: Option<PendingResult>,
}

impl PendingRequest {
    /// Wait at most `timeout` from this call, while the client can still route events.
    pub fn wait(mut self, timeout: Duration) -> Result<DapResponse, DapError> {
        let result = match self.completed.take() {
            Some(result) => result,
            None => match self.receiver.recv_timeout(timeout) {
                Ok(result) => result,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(DapError::Timeout(self.command.clone()))
                }
                Err(RecvTimeoutError::Disconnected) => return Err(DapError::AdapterExited),
            },
        };
        self.decode_result(result)
    }

    /// Return immediately with `None` while the adapter has not replied yet.
    /// The response subscription stays active after `None` and can be polled again.
    pub fn try_wait(&mut self) -> Result<Option<DapResponse>, DapError> {
        if self.completed.is_some() {
            return Ok(None);
        }
        match self.receiver.try_recv() {
            Ok(result) => {
                self.completed = Some(result.clone());
                self.decode_result(result).map(Some)
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(DapError::AdapterExited),
        }
    }

    fn decode_result(&self, result: PendingResult) -> Result<DapResponse, DapError> {
        let value = match result {
            PendingResult::Response(value) => value,
            PendingResult::AdapterExited => return Err(DapError::AdapterExited),
            PendingResult::ReaderFailed(message) => return Err(DapError::InvalidMessage(message)),
        };
        let response = value
            .as_object()
            .ok_or_else(|| DapError::InvalidMessage("response is not an object".into()))?;
        Ok(DapResponse {
            request_seq: response
                .get("request_seq")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            command: response
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or(&self.command)
                .to_owned(),
            success: response
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            message: response
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned),
            body: response.get("body").cloned().unwrap_or(Value::Null),
        })
    }
}

impl Drop for PendingRequest {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.requests.remove(&self.sequence);
        }
    }
}

#[derive(Serialize)]
struct BreakpointRequest<'a> {
    line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    condition: Option<&'a str>,
    #[serde(rename = "hitCondition", skip_serializing_if = "Option::is_none")]
    hit_condition: Option<&'a str>,
    #[serde(rename = "logMessage", skip_serializing_if = "Option::is_none")]
    log_message: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Breakpoint {
    pub line: u32,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
    pub log_message: Option<String>,
}

impl Breakpoint {
    pub fn new(line: u32) -> Self {
        Self {
            line,
            condition: None,
            hit_condition: None,
            log_message: None,
        }
    }
    pub fn condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }
    pub fn hit_condition(mut self, condition: impl Into<String>) -> Self {
        self.hit_condition = Some(condition.into());
        self
    }
    pub fn log_message(mut self, message: impl Into<String>) -> Self {
        self.log_message = Some(message.into());
        self
    }
}

/// A DAP session over an adapter's stdin/stdout streams.
pub struct DapClient {
    adapter_id: String,
    writer: Arc<Mutex<std::process::ChildStdin>>,
    pending: Arc<Mutex<PendingRequests>>,
    events: Receiver<DapEvent>,
    sequence: AtomicI64,
    child: Child,
    reader_thread: Option<JoinHandle<()>>,
    stderr_thread: Option<JoinHandle<()>>,
    closed: bool,
}

impl DapClient {
    pub fn spawn(command: &AdapterCommand) -> Result<Self, DapError> {
        let mut child_command = Command::new(&command.program);
        child_command
            .args(&command.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = &command.cwd {
            child_command.current_dir(cwd);
        }
        child_command.envs(command.env.iter().cloned());
        let mut child = child_command.spawn()?;
        let writer = Arc::new(Mutex::new(child.stdin.take().ok_or_else(|| {
            DapError::InvalidMessage("adapter stdin was not piped".into())
        })?));
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DapError::InvalidMessage("adapter stdout was not piped".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| DapError::InvalidMessage("adapter stderr was not piped".into()))?;
        let pending = Arc::new(Mutex::new(PendingRequests::default()));
        let (event_tx, events) = mpsc::channel();
        let reader_thread = Some(spawn_reader_thread(
            stdout,
            pending.clone(),
            event_tx.clone(),
        ));
        let stderr_thread = Some(spawn_stderr_thread(stderr, event_tx));
        Ok(Self {
            adapter_id: command.adapter_id.clone(),
            writer,
            pending,
            events,
            sequence: AtomicI64::new(1),
            child,
            reader_thread,
            stderr_thread,
            closed: false,
        })
    }

    pub fn request(&self, command: &str, arguments: Value) -> Result<DapResponse, DapError> {
        self.request_timeout(command, arguments, DEFAULT_REQUEST_TIMEOUT)
    }

    pub fn request_timeout(
        &self,
        command: &str,
        arguments: Value,
        timeout: Duration,
    ) -> Result<DapResponse, DapError> {
        self.request_async(command, arguments)?.wait(timeout)
    }

    /// Send a request without waiting for its reply. Useful when an adapter needs
    /// configuration requests before it can complete `launch` or `attach`.
    pub fn request_async(
        &self,
        command: &str,
        arguments: Value,
    ) -> Result<PendingRequest, DapError> {
        if self.closed {
            return Err(DapError::AdapterExited);
        }
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| DapError::InvalidMessage("pending request lock poisoned".into()))?;
            if pending.adapter_exited {
                return Err(DapError::AdapterExited);
            }
            pending.requests.insert(seq, tx);
        }
        let request = PendingRequest {
            sequence: seq,
            command: command.to_owned(),
            receiver: rx,
            pending: self.pending.clone(),
            completed: None,
        };
        let message = json!({"seq":seq,"type":"request","command":command,"arguments":arguments});
        let write_result = self
            .writer
            .lock()
            .map_err(|_| DapError::InvalidMessage("adapter input lock poisoned".into()))
            .and_then(|mut writer| write_frame(&mut *writer, &message));
        if let Err(error) = write_result {
            drop(request);
            return Err(error);
        }
        Ok(request)
    }

    pub fn initialize(&self, client_id: &str, client_name: &str) -> Result<DapResponse, DapError> {
        self.request(
            "initialize",
            json!({
                "clientID": client_id, "clientName": client_name,
                "adapterID": self.adapter_id, "locale": "en-US",
                "linesStartAt1": true, "columnsStartAt1": true,
                "pathFormat": "path", "supportsVariableType": true,
                "supportsRunInTerminalRequest": false,
                "supportsProgressReporting": true
            }),
        )
    }

    /// Start launch, then configure on `initialized` before awaiting the reply.
    pub fn begin_launch(&self, arguments: Value) -> Result<PendingRequest, DapError> {
        self.request_async("launch", arguments)
    }
    pub fn begin_attach(&self, arguments: Value) -> Result<PendingRequest, DapError> {
        self.request_async("attach", arguments)
    }
    /// Blocking convenience for adapters which do not defer the launch response.
    /// For LLDB and normal session setup, use `begin_launch` instead.
    #[deprecated(note = "use begin_launch and handle the initialized event before waiting")]
    pub fn launch(&self, arguments: Value) -> Result<DapResponse, DapError> {
        self.request("launch", arguments)
    }
    /// Blocking convenience; use `begin_attach` if configuration is still needed.
    #[deprecated(note = "use begin_attach and handle the initialized event before waiting")]
    pub fn attach(&self, arguments: Value) -> Result<DapResponse, DapError> {
        self.request("attach", arguments)
    }
    pub fn configuration_done(&self) -> Result<DapResponse, DapError> {
        self.request("configurationDone", json!({}))
    }

    pub fn set_breakpoints(
        &self,
        source: &Path,
        breakpoints: &[Breakpoint],
    ) -> Result<DapResponse, DapError> {
        let requests: Vec<_> = breakpoints
            .iter()
            .map(|breakpoint| BreakpointRequest {
                line: breakpoint.line,
                condition: breakpoint.condition.as_deref(),
                hit_condition: breakpoint.hit_condition.as_deref(),
                log_message: breakpoint.log_message.as_deref(),
            })
            .collect();
        self.request("setBreakpoints", json!({
            "source": { "name": source.file_name().unwrap_or_else(|| source.as_os_str()).to_string_lossy(), "path": source.to_string_lossy() },
            "breakpoints": requests,
            "sourceModified": false
        }))
    }

    pub fn threads(&self) -> Result<DapResponse, DapError> {
        self.request("threads", json!({}))
    }
    pub fn stack_trace(
        &self,
        thread_id: i64,
        start: Option<u32>,
        levels: Option<u32>,
    ) -> Result<DapResponse, DapError> {
        self.request(
            "stackTrace",
            json!({ "threadId": thread_id, "startFrame": start, "levels": levels }),
        )
    }
    pub fn scopes(&self, frame_id: i64) -> Result<DapResponse, DapError> {
        self.request("scopes", json!({ "frameId": frame_id }))
    }
    pub fn variables(&self, variables_reference: i64) -> Result<DapResponse, DapError> {
        self.request(
            "variables",
            json!({ "variablesReference": variables_reference }),
        )
    }
    pub fn continue_execution(&self, thread_id: i64) -> Result<DapResponse, DapError> {
        self.request("continue", json!({ "threadId": thread_id }))
    }
    pub fn next(&self, thread_id: i64) -> Result<DapResponse, DapError> {
        self.request("next", json!({ "threadId": thread_id }))
    }
    pub fn step_in(&self, thread_id: i64) -> Result<DapResponse, DapError> {
        self.request("stepIn", json!({ "threadId": thread_id }))
    }
    pub fn step_out(&self, thread_id: i64) -> Result<DapResponse, DapError> {
        self.request("stepOut", json!({ "threadId": thread_id }))
    }
    pub fn pause(&self, thread_id: i64) -> Result<DapResponse, DapError> {
        self.request("pause", json!({ "threadId": thread_id }))
    }
    pub fn evaluate(
        &self,
        expression: &str,
        frame_id: Option<i64>,
        context: &str,
    ) -> Result<DapResponse, DapError> {
        self.request(
            "evaluate",
            json!({ "expression": expression, "frameId": frame_id, "context": context }),
        )
    }

    pub fn try_recv_event(&self) -> Result<DapEvent, mpsc::TryRecvError> {
        self.events.try_recv()
    }
    pub fn recv_event_timeout(&self, timeout: Duration) -> Result<DapEvent, RecvTimeoutError> {
        self.events.recv_timeout(timeout)
    }

    pub fn disconnect(&self, terminate_debuggee: bool) -> Result<DapResponse, DapError> {
        self.request_timeout(
            "disconnect",
            json!({ "restart": false, "terminateDebuggee": terminate_debuggee }),
            Duration::from_secs(2),
        )
    }

    /// Ask the adapter to disconnect, then terminate it if it does not exit promptly.
    pub fn shutdown(&mut self, timeout: Duration) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        if self.child.try_wait()?.is_none() {
            let _ = self.disconnect(true);
            let start = Instant::now();
            while start.elapsed() < timeout {
                if self.child.try_wait()?.is_some() {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
            if self.child.try_wait()?.is_none() {
                let _ = self.child.kill();
            }
        }
        let _ = self.child.wait();
        self.closed = true;
        if let Some(thread) = self.reader_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.stderr_thread.take() {
            let _ = thread.join();
        }
        Ok(())
    }
}

impl Drop for DapClient {
    fn drop(&mut self) {
        let _ = self.shutdown(Duration::from_millis(500));
    }
}

fn spawn_reader_thread(
    stdout: std::process::ChildStdout,
    pending: Arc<Mutex<PendingRequests>>,
    events: Sender<DapEvent>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("nexul-dap-reader".into())
        .spawn(move || {
            let mut reader = BufReader::new(stdout);
            let terminal_result = loop {
                match read_frame(&mut reader) {
                    Ok(Some(value)) => route_message(value, &pending, &events),
                    Ok(None) => break PendingResult::AdapterExited,
                    Err(error) => {
                        let message = error.to_string();
                        let _ = events.send(DapEvent {
                            event: "protocolError".into(),
                            body: json!({ "message": message }),
                        });
                        break PendingResult::ReaderFailed(message);
                    }
                }
            };
            let requests = {
                let mut state = pending
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                state.adapter_exited = true;
                std::mem::take(&mut state.requests)
            };
            for (_, sender) in requests {
                let _ = sender.send(terminal_result.clone());
            }
            let _ = events.send(DapEvent {
                event: "adapterExited".into(),
                body: Value::Null,
            });
        })
        .expect("failed to start DAP reader thread")
}

fn spawn_stderr_thread(
    stderr: std::process::ChildStderr,
    events: Sender<DapEvent>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("nexul-dap-stderr".into())
        .spawn(move || {
            for line in BufReader::new(stderr).lines().flatten() {
                let _ = events.send(DapEvent {
                    event: "adapterOutput".into(),
                    body: json!({ "category": "stderr", "output": line }),
                });
            }
        })
        .expect("failed to start DAP stderr thread")
}

fn route_message(value: Value, pending: &Arc<Mutex<PendingRequests>>, events: &Sender<DapEvent>) {
    match value.get("type").and_then(Value::as_str) {
        Some("response") => {
            let request_seq = value.get("request_seq").and_then(Value::as_i64);
            if let Some(sender) =
                request_seq.and_then(|seq| pending.lock().ok()?.requests.remove(&seq))
            {
                let _ = sender.send(PendingResult::Response(value));
            }
        }
        Some("event") => {
            let event = value
                .get("event")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned();
            let body = value.get("body").cloned().unwrap_or(Value::Null);
            let _ = events.send(DapEvent { event, body });
        }
        _ => {
            let _ = events.send(DapEvent { event: "protocolError".into(), body: json!({ "message": "adapter sent a message that is not a response or event", "raw": value }) });
        }
    }
}

fn read_frame(reader: &mut impl BufRead) -> Result<Option<Value>, DapError> {
    let mut content_length = None;
    let mut saw_header = false;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            if saw_header {
                return Err(DapError::InvalidMessage(
                    "adapter closed in the middle of a DAP header".into(),
                ));
            }
            return Ok(None);
        }
        saw_header = true;
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.trim_end_matches(['\r', '\n']).split_once(':') {
            if name.eq_ignore_ascii_case("Content-Length") {
                content_length = Some(value.trim().parse::<usize>().map_err(|_| {
                    DapError::InvalidMessage("invalid Content-Length header".into())
                })?);
            }
        }
    }
    let length = content_length
        .ok_or_else(|| DapError::InvalidMessage("DAP frame has no Content-Length header".into()))?;
    if length > MAX_FRAME_BYTES {
        return Err(DapError::InvalidMessage(format!(
            "DAP frame exceeds {MAX_FRAME_BYTES} bytes"
        )));
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    Ok(Some(serde_json::from_slice(&bytes)?))
}

fn write_frame(writer: &mut impl Write, message: &Value) -> Result<(), DapError> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

/// Build a host executable with NEXUL's default debug information enabled. On
/// Windows, this also checks the adjacent PDB generated by `nxc build`.
pub fn build_nexul_program(nxc: &Path, source: &Path, output: &Path) -> io::Result<NexulProgram> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let result = Command::new(nxc)
        .arg("build")
        .arg(source)
        .arg("-o")
        .arg(output)
        .output()?;
    if !result.status.success() {
        let mut detail = String::from_utf8_lossy(&result.stderr).into_owned();
        if detail.trim().is_empty() {
            detail = String::from_utf8_lossy(&result.stdout).into_owned();
        }
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("nxc build failed: {}", detail.trim()),
        ));
    }
    if !output.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "nxc reported success but did not create the executable",
        ));
    }
    let pdb = if cfg!(windows) {
        let pdb = output.with_extension("pdb");
        if !pdb.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("nxc build did not create expected PDB {}", pdb.display()),
            ));
        }
        Some(pdb)
    } else {
        None
    };
    Ok(NexulProgram {
        executable: output.to_path_buf(),
        pdb,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NexulProgram {
    pub executable: PathBuf,
    pub pdb: Option<PathBuf>,
}

/// Arguments accepted by `lldb-dap` for a NEXUL executable.
pub fn lldb_launch_arguments(
    program: &Path,
    cwd: &Path,
    args: &[OsString],
    stop_on_entry: bool,
) -> Value {
    json!({
        "program": program.to_string_lossy(),
        "cwd": cwd.to_string_lossy(),
        "args": args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        "stopOnEntry": stop_on_entry,
        "disableASLR": true
    })
}

/// Find `nxc` from an explicit environment path or from `PATH`.
pub fn find_nxc() -> Option<PathBuf> {
    let explicit = std::env::var_os("NEXUL_NXC").map(PathBuf::from);
    let path_candidate = if explicit.is_none() {
        find_executable(&["nxc.exe", "nxc"])
    } else {
        None
    };
    select_nxc(explicit, path_candidate)
}

fn select_nxc(explicit: Option<PathBuf>, path_candidate: Option<PathBuf>) -> Option<PathBuf> {
    match explicit {
        Some(path) => path.is_file().then_some(path),
        None => path_candidate.filter(|path| path.is_file()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoint_conditions_are_serializable() {
        let bp = Breakpoint::new(12)
            .condition("value == 2")
            .hit_condition("3")
            .log_message("hit {value}");
        assert_eq!(bp.line, 12);
        assert_eq!(bp.condition.as_deref(), Some("value == 2"));
        assert_eq!(bp.hit_condition.as_deref(), Some("3"));
    }

    #[test]
    fn launch_arguments_are_platform_paths() {
        let args = lldb_launch_arguments(
            Path::new("a.nx.exe"),
            Path::new("."),
            &[OsString::from("one")],
            true,
        );
        assert_eq!(args["program"], "a.nx.exe");
        assert_eq!(args["stopOnEntry"], true);
        assert_eq!(args["args"][0], "one");
    }

    #[test]
    fn frame_reader_accepts_content_length_with_extra_headers() {
        let body = br#"{"seq":1,"type":"event","event":"stopped","body":{"threadId":3}}"#;
        let frame = format!("Content-Type: application/vscode-jsonrpc; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}", body.len(), std::str::from_utf8(body).unwrap());
        let value = read_frame(&mut io::Cursor::new(frame)).unwrap().unwrap();
        assert_eq!(value["event"], "stopped");
        assert_eq!(value["body"]["threadId"], 3);
    }

    #[test]
    fn invalid_explicit_nxc_path_does_not_fall_back_to_path() {
        assert!(select_nxc(
            Some(std::env::temp_dir()),
            Some(std::env::current_exe().unwrap())
        )
        .is_none());
    }

    #[test]
    fn explicit_nxc_path_takes_precedence_over_path_candidate() {
        let explicit = std::env::current_exe().unwrap();
        let path_candidate = std::env::temp_dir().join("another-nxc-candidate.exe");
        assert_eq!(
            select_nxc(Some(explicit.clone()), Some(path_candidate)),
            Some(explicit)
        );
    }
}

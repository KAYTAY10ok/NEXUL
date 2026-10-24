//! Language Server Protocol 3.17 transport for `nxc`.

use crate::{CompileOptions, Mode, Target};
use std::collections::{BTreeMap, HashMap};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
const DIAGNOSTIC_DELAY: Duration = Duration::from_millis(200);

#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl Json {
    fn object(entries: impl IntoIterator<Item = (impl Into<String>, Json)>) -> Json {
        Json::Object(entries.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(map) => map.get(key),
            _ => None,
        }
    }

    fn string(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    fn integer(&self) -> Option<i64> {
        match self {
            Json::Number(n) => n.parse().ok(),
            _ => None,
        }
    }

    fn boolean(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    fn encode(&self) -> String {
        match self {
            Json::Null => "null".into(),
            Json::Bool(v) => v.to_string(),
            Json::Number(n) => n.clone(),
            Json::String(s) => crate::json_str(s),
            Json::Array(items) => format!(
                "[{}]",
                items.iter().map(Json::encode).collect::<Vec<_>>().join(",")
            ),
            Json::Object(map) => format!(
                "{{{}}}",
                map.iter()
                    .map(|(k, v)| format!("{}:{}", crate::json_str(k), v.encode()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

fn jstr(value: impl Into<String>) -> Json {
    Json::String(value.into())
}
fn jnum(value: impl ToString) -> Json {
    Json::Number(value.to_string())
}
fn obj(entries: impl IntoIterator<Item = (impl Into<String>, Json)>) -> Json {
    Json::object(entries)
}
fn arr(items: impl IntoIterator<Item = Json>) -> Json {
    Json::Array(items.into_iter().collect())
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

fn parse_json(text: &str) -> Result<Json, String> {
    let mut parser = JsonParser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    let value = parser.value()?;
    parser.whitespace();
    if parser.pos != parser.bytes.len() {
        return Err("trailing data after JSON value".into());
    }
    Ok(value)
}

impl JsonParser<'_> {
    fn whitespace(&mut self) {
        while self.pos < self.bytes.len()
            && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\r' | b'\n')
        {
            self.pos += 1;
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.whitespace();
        let Some(&b) = self.bytes.get(self.pos) else {
            return Err("expected a JSON value".into());
        };
        match b {
            b'n' => {
                self.literal(b"null")?;
                Ok(Json::Null)
            }
            b't' => {
                self.literal(b"true")?;
                Ok(Json::Bool(true))
            }
            b'f' => {
                self.literal(b"false")?;
                Ok(Json::Bool(false))
            }
            b'"' => self.string().map(Json::String),
            b'[' => self.array(),
            b'{' => self.object(),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err("unexpected character in JSON value".into()),
        }
    }

    fn literal(&mut self, expected: &[u8]) -> Result<(), String> {
        if self.bytes.get(self.pos..self.pos + expected.len()) == Some(expected) {
            self.pos += expected.len();
            Ok(())
        } else {
            Err("invalid JSON literal".into())
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.pos += 1;
        let mut out = String::new();
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            self.pos += 1;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let Some(&escaped) = self.bytes.get(self.pos) else {
                        return Err("unfinished JSON escape".into());
                    };
                    self.pos += 1;
                    match escaped {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.unicode_escape()?),
                        _ => return Err("invalid JSON escape".into()),
                    }
                }
                0..=0x1f => return Err("control character in JSON string".into()),
                0x20..=0x7f => out.push(b as char),
                _ => {
                    self.pos -= 1;
                    let rest = std::str::from_utf8(&self.bytes[self.pos..])
                        .map_err(|_| "invalid UTF-8 in JSON string")?;
                    let ch = rest
                        .chars()
                        .next()
                        .ok_or_else(|| "invalid UTF-8 in JSON string".to_string())?;
                    self.pos += ch.len_utf8();
                    out.push(ch);
                }
            }
        }
        Err("unterminated JSON string".into())
    }

    fn unicode_escape(&mut self) -> Result<char, String> {
        let high = self.hex_quad()?;
        let scalar = if (0xd800..=0xdbff).contains(&high) {
            if self.bytes.get(self.pos..self.pos + 2) != Some(b"\\u") {
                return Err("missing low surrogate in JSON escape".into());
            }
            self.pos += 2;
            let low = self.hex_quad()?;
            if !(0xdc00..=0xdfff).contains(&low) {
                return Err("invalid low surrogate in JSON escape".into());
            }
            0x10000 + (((high - 0xd800) as u32) << 10) + (low - 0xdc00) as u32
        } else if (0xdc00..=0xdfff).contains(&high) {
            return Err("unpaired low surrogate in JSON escape".into());
        } else {
            high as u32
        };
        char::from_u32(scalar).ok_or_else(|| "invalid Unicode scalar in JSON escape".into())
    }

    fn hex_quad(&mut self) -> Result<u16, String> {
        if self.pos + 4 > self.bytes.len() {
            return Err("short Unicode escape".into());
        }
        let mut value = 0u16;
        for _ in 0..4 {
            let digit = match self.bytes[self.pos] {
                b'0'..=b'9' => self.bytes[self.pos] - b'0',
                b'a'..=b'f' => self.bytes[self.pos] - b'a' + 10,
                b'A'..=b'F' => self.bytes[self.pos] - b'A' + 10,
                _ => return Err("invalid hexadecimal digit in Unicode escape".into()),
            };
            self.pos += 1;
            value = (value << 4) | digit as u16;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.bytes[self.pos] == b'-' {
            self.pos += 1;
        }
        if self.bytes.get(self.pos) == Some(&b'0') {
            self.pos += 1;
        } else {
            let before = self.pos;
            while self.bytes.get(self.pos).map_or(false, u8::is_ascii_digit) {
                self.pos += 1;
            }
            if self.pos == before {
                return Err("invalid JSON number".into());
            }
        }
        if self.bytes.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            let before = self.pos;
            while self.bytes.get(self.pos).map_or(false, u8::is_ascii_digit) {
                self.pos += 1;
            }
            if self.pos == before {
                return Err("invalid JSON fraction".into());
            }
        }
        if matches!(self.bytes.get(self.pos), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.bytes.get(self.pos), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            let before = self.pos;
            while self.bytes.get(self.pos).map_or(false, u8::is_ascii_digit) {
                self.pos += 1;
            }
            if self.pos == before {
                return Err("invalid JSON exponent".into());
            }
        }
        Ok(Json::Number(
            std::str::from_utf8(&self.bytes[start..self.pos])
                .unwrap()
                .to_string(),
        ))
    }

    fn array(&mut self) -> Result<Json, String> {
        self.pos += 1;
        self.whitespace();
        let mut items = Vec::new();
        if self.bytes.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.whitespace();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err("expected `,` or `]` in JSON array".into()),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.pos += 1;
        self.whitespace();
        let mut map = BTreeMap::new();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(Json::Object(map));
        }
        loop {
            self.whitespace();
            if self.bytes.get(self.pos) != Some(&b'"') {
                return Err("expected a string key in JSON object".into());
            }
            let key = self.string()?;
            self.whitespace();
            if self.bytes.get(self.pos) != Some(&b':') {
                return Err("expected `:` after JSON object key".into());
            }
            self.pos += 1;
            let value = self.value()?;
            map.insert(key, value);
            self.whitespace();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Object(map));
                }
                _ => return Err("expected `,` or `}` in JSON object".into()),
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PositionEncoding {
    Utf8,
    Utf16,
}

#[derive(Clone)]
struct Document {
    uri: String,
    path: PathBuf,
    text: String,
    version: i64,
    generation: u64,
}

#[derive(Clone)]
struct DocumentSnapshot(Document);

#[derive(Clone)]
struct RpcError {
    code: i64,
    message: String,
}

struct PendingRequest {
    id: Json,
    uri: String,
    version: Option<i64>,
    generation: u64,
    document_generations: Vec<(String, u64)>,
}

enum Job {
    Request {
        key: String,
        method: String,
        params: Json,
        document: DocumentSnapshot,
        documents: Vec<DocumentSnapshot>,
        cfg: nxc_sema::target::Cfg,
        encoding: PositionEncoding,
        cacheable: bool,
    },
    Publish {
        documents: Vec<DocumentSnapshot>,
        cfg: nxc_sema::target::Cfg,
        encoding: PositionEncoding,
    },
    InvalidateCache,
    Stop,
}

struct CachedAnalysis {
    fingerprint: String,
    analysis: crate::ide::Analysis,
}

#[derive(Default)]
struct AnalysisCache {
    cached: Option<CachedAnalysis>,
}

struct Publication {
    uri: String,
    generation: Option<u64>,
    version: Option<i64>,
    document_generations: Vec<(String, u64)>,
    diagnostics: Json,
}

enum WorkerEvent {
    Response {
        key: String,
        result: Result<Json, RpcError>,
    },
    Published(Vec<Publication>),
}

enum Event {
    Input(Result<Json, String>),
    Eof,
    Worker(WorkerEvent),
}

struct Logger(Mutex<Option<File>>);

impl Logger {
    fn log(&self, message: impl AsRef<str>) {
        if let Ok(mut file) = self.0.lock() {
            if let Some(file) = file.as_mut() {
                let _ = writeln!(file, "{}", message.as_ref());
                let _ = file.flush();
            }
        }
    }
}

struct Runtime {
    initialized: bool,
    shutdown: bool,
    exited: bool,
    exit_code: i32,
    cfg: nxc_sema::target::Cfg,
    encoding: PositionEncoding,
    documents: HashMap<String, Document>,
    generations: HashMap<String, u64>,
    pending: HashMap<String, PendingRequest>,
    diagnostic_due: HashMap<String, Instant>,
    next_server_id: u64,
}

impl Runtime {
    fn new() -> Self {
        Self {
            initialized: false,
            shutdown: false,
            exited: false,
            exit_code: 0,
            cfg: nxc_sema::target::Cfg::default(),
            encoding: PositionEncoding::Utf16,
            documents: HashMap::new(),
            generations: HashMap::new(),
            pending: HashMap::new(),
            diagnostic_due: HashMap::new(),
            next_server_id: 1,
        }
    }

    fn snapshots(&self) -> Vec<DocumentSnapshot> {
        self.documents
            .values()
            .cloned()
            .map(DocumentSnapshot)
            .collect()
    }
    fn schedule_all(&mut self) {
        let due = Instant::now() + DIAGNOSTIC_DELAY;
        for uri in self.documents.keys() {
            self.diagnostic_due.insert(uri.clone(), due);
        }
    }

    fn bump_generation(&mut self, uri: &str) -> u64 {
        let generation = self.generations.entry(uri.to_string()).or_insert(0);
        *generation += 1;
        *generation
    }
}

fn read_frame<R: BufRead>(reader: &mut R) -> io::Result<Option<Result<Json, String>>> {
    let mut content_length = None;
    let mut saw_header = false;
    loop {
        let mut line = String::new();
        let count = reader.read_line(&mut line)?;
        if count == 0 {
            return if saw_header {
                Ok(Some(Err("stdin ended inside a JSON-RPC header".into())))
            } else {
                Ok(None)
            };
        }
        saw_header = true;
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                match value.trim().parse::<usize>() {
                    Ok(n) => content_length = Some(n),
                    Err(_) => return Ok(Some(Err("invalid Content-Length header".into()))),
                }
            }
        }
    }
    let Some(length) = content_length else {
        return Ok(Some(Err("missing Content-Length header".into())));
    };
    if length > MAX_MESSAGE_BYTES {
        let mut remaining = length;
        let mut buffer = [0u8; 8192];
        while remaining > 0 {
            let chunk = remaining.min(buffer.len());
            reader.read_exact(&mut buffer[..chunk])?;
            remaining -= chunk;
        }
        return Ok(Some(
            Err("JSON-RPC message exceeds the 16 MiB limit".into()),
        ));
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(_) => return Ok(Some(Err("JSON-RPC message is not UTF-8".into()))),
    };
    Ok(Some(parse_json(text)))
}

fn input_loop(events: Sender<Event>, logger: Arc<Logger>) {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    loop {
        match read_frame(&mut reader) {
            Ok(Some(Ok(message))) => {
                if events.send(Event::Input(Ok(message))).is_err() {
                    break;
                }
            }
            Ok(Some(Err(error))) => {
                logger.log(format!("parse error: {}", error));
                if events.send(Event::Input(Err(error))).is_err() {
                    break;
                }
            }
            Ok(None) => {
                let _ = events.send(Event::Eof);
                break;
            }
            Err(error) => {
                logger.log(format!("stdin read error: {}", error));
                let _ = events.send(Event::Input(Err(error.to_string())));
                let _ = events.send(Event::Eof);
                break;
            }
        }
    }
}

fn write_frame<W: Write>(writer: &mut W, message: &Json) -> io::Result<()> {
    let body = message.encode();
    write!(writer, "Content-Length: {}\r\n\r\n", body.as_bytes().len())?;
    writer.write_all(body.as_bytes())?;
    writer.flush()
}

fn rpc_response(id: Json, result: Json) -> Json {
    obj([("jsonrpc", jstr("2.0")), ("id", id), ("result", result)])
}

fn rpc_error(id: Json, code: i64, message: impl Into<String>) -> Json {
    obj([
        ("jsonrpc", jstr("2.0")),
        ("id", id),
        (
            "error",
            obj([("code", jnum(code)), ("message", jstr(message))]),
        ),
    ])
}

fn notification(method: &str, params: Json) -> Json {
    obj([
        ("jsonrpc", jstr("2.0")),
        ("method", jstr(method)),
        ("params", params),
    ])
}

fn request_key(id: &Json) -> String {
    id.encode()
}

fn response_for_pending(
    runtime: &Runtime,
    pending: PendingRequest,
    result: Result<Json, RpcError>,
) -> Json {
    let generation = runtime.generations.get(&pending.uri).copied().unwrap_or(0);
    let current_version = runtime.documents.get(&pending.uri).map(|doc| doc.version);
    let modified = generation != pending.generation
        || pending
            .version
            .map_or(false, |version| current_version != Some(version))
        || pending
            .document_generations
            .iter()
            .any(|(uri, generation)| {
                runtime.documents.get(uri).map(|doc| doc.generation) != Some(*generation)
            });
    if modified {
        rpc_error(
            pending.id,
            -32801,
            "document changed while the request was running",
        )
    } else {
        match result {
            Ok(value) => rpc_response(pending.id, value),
            Err(error) => rpc_error(pending.id, error.code, error.message),
        }
    }
}

fn publication_is_stale(runtime: &Runtime, publication: &Publication) -> bool {
    publication
        .document_generations
        .iter()
        .any(|(uri, generation)| {
            runtime.documents.get(uri).map(|doc| doc.generation) != Some(*generation)
        })
        || publication.generation.map_or(false, |generation| {
            runtime
                .documents
                .get(&publication.uri)
                .map(|doc| doc.generation)
                != Some(generation)
        })
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let high = hex_value(*bytes.get(i + 1)?)?;
            let low = hex_value(*bytes.get(i + 2)?)?;
            out.push((high << 4) | low);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/' | b':') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

fn path_from_file_uri(uri: &str) -> Option<PathBuf> {
    let raw = uri.strip_prefix("file://")?;
    let decoded = percent_decode(raw)?;
    if cfg!(windows) {
        let drive_path = decoded.strip_prefix('/').unwrap_or(&decoded);
        if drive_path.as_bytes().get(1) == Some(&b':') {
            Some(PathBuf::from(drive_path.replace('/', "\\")))
        } else if decoded.starts_with("//") || !decoded.starts_with('/') {
            let unc = decoded.trim_start_matches('/').replace('/', "\\");
            Some(PathBuf::from(format!("\\\\{}", unc)))
        } else {
            Some(PathBuf::from(decoded.replace('/', "\\")))
        }
    } else {
        let path = if decoded.starts_with('/') {
            decoded
        } else {
            format!("/{}", decoded)
        };
        Some(PathBuf::from(path))
    }
}

fn file_uri_from_path(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut raw = absolute.to_string_lossy().replace('\\', "/");
    if let Some(rest) = raw.strip_prefix("//?/UNC/") {
        raw = format!("//{}", rest);
    } else if let Some(rest) = raw.strip_prefix("//?/") {
        raw = rest.to_string();
    }
    if cfg!(windows) {
        if raw.starts_with("//") {
            format!("file:{}", percent_encode(&raw))
        } else {
            format!("file:///{}", percent_encode(raw.trim_start_matches('/')))
        }
    } else {
        format!("file://{}", percent_encode(&raw))
    }
}

fn standard_uri(source_name: &str, cfg: &nxc_sema::target::Cfg) -> Option<String> {
    let module = source_name.strip_prefix("<std.")?.strip_suffix('>')?;
    let module = if module == "sys" {
        match cfg.target.os {
            nxc_sema::target::Os::Windows => "sys/windows",
            nxc_sema::target::Os::Linux | nxc_sema::target::Os::MacOs => "sys/linux",
            nxc_sema::target::Os::Wasi => "sys/wasi",
            nxc_sema::target::Os::Web => "sys/web",
            nxc_sema::target::Os::None => "sys/bare",
        }
    } else {
        module
    };
    Some(format!("nexul-std:///{}.nx", module.replace('.', "/")))
}

fn std_module_from_uri(uri: &str) -> Option<String> {
    let path = uri.strip_prefix("nexul-std:///")?.strip_suffix(".nx")?;
    if path.starts_with("sys/") {
        Some("std.sys".into())
    } else {
        Some(format!("std.{}", path.replace('/', ".")))
    }
}

fn source_path(analysis: &crate::ide::Analysis, file: nxc_syntax::FileId) -> Option<PathBuf> {
    let source = analysis.sm.try_get(file)?;
    if source.name.starts_with('<') {
        return None;
    }
    let path = Path::new(&source.name);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        analysis
            .dir
            .as_deref()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    })
}

fn uri_for_span(
    analysis: &crate::ide::Analysis,
    span: nxc_syntax::Span,
    cfg: &nxc_sema::target::Cfg,
) -> Option<String> {
    let source = analysis.sm.try_get(span.file)?;
    if source.name.starts_with('<') {
        standard_uri(&source.name, cfg)
    } else {
        source_path(analysis, span.file).map(|path| file_uri_from_path(&path))
    }
}

fn line_bounds(text: &str, line: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut start = 0usize;
    for _ in 0..line {
        let newline = bytes[start..].iter().position(|b| *b == b'\n')?;
        start += newline + 1;
    }
    let end = bytes[start..]
        .iter()
        .position(|b| *b == b'\n')
        .map(|i| start + i)
        .unwrap_or(bytes.len());
    let end = if end > start && bytes[end - 1] == b'\r' {
        end - 1
    } else {
        end
    };
    Some((start, end))
}

fn lsp_to_ide_position(
    text: &str,
    line: usize,
    character: usize,
    encoding: PositionEncoding,
) -> Option<(usize, usize)> {
    let (start, end) = line_bounds(text, line)?;
    let mut units = 0usize;
    let mut columns = 0usize;
    for ch in text.get(start..end)?.chars() {
        if units == character {
            return Some((line + 1, columns + 1));
        }
        let width = match encoding {
            PositionEncoding::Utf8 => ch.len_utf8(),
            PositionEncoding::Utf16 => ch.len_utf16(),
        };
        if units + width > character {
            return None;
        }
        units += width;
        columns += 1;
    }
    (units == character).then_some((line + 1, columns + 1))
}

fn lsp_position_to_byte(
    text: &str,
    line: usize,
    character: usize,
    encoding: PositionEncoding,
) -> Option<usize> {
    let (start, end) = line_bounds(text, line)?;
    let mut units = 0usize;
    for (byte, ch) in text.get(start..end)?.char_indices() {
        if units == character {
            return Some(start + byte);
        }
        let width = match encoding {
            PositionEncoding::Utf8 => ch.len_utf8(),
            PositionEncoding::Utf16 => ch.len_utf16(),
        };
        if units + width > character {
            return None;
        }
        units += width;
    }
    (units == character).then_some(end)
}

fn ide_column_to_lsp(
    text: &str,
    line: usize,
    column: usize,
    encoding: PositionEncoding,
) -> Option<usize> {
    let (start, end) = line_bounds(text, line.checked_sub(1)?)?;
    let prefix: String = text
        .get(start..end)?
        .chars()
        .take(column.saturating_sub(1))
        .collect();
    Some(match encoding {
        PositionEncoding::Utf8 => prefix.len(),
        PositionEncoding::Utf16 => prefix.encode_utf16().count(),
    })
}

fn lsp_position(line: usize, character: usize) -> Json {
    obj([("line", jnum(line)), ("character", jnum(character))])
}

fn lsp_range(start_line: usize, start_char: usize, end_line: usize, end_char: usize) -> Json {
    obj([
        ("start", lsp_position(start_line, start_char)),
        ("end", lsp_position(end_line, end_char)),
    ])
}

fn span_range(
    analysis: &crate::ide::Analysis,
    span: nxc_syntax::Span,
    encoding: PositionEncoding,
) -> Option<Json> {
    let file = analysis.sm.try_get(span.file)?;
    if span.is_dummy() {
        return None;
    }
    let (start_line, start_col) = file.line_col(span.lo);
    let (end_line, end_col) = file.line_col(span.hi.max(span.lo));
    Some(lsp_range(
        start_line - 1,
        ide_column_to_lsp(&file.text, start_line, start_col, encoding)?,
        end_line - 1,
        ide_column_to_lsp(&file.text, end_line, end_col, encoding)?,
    ))
}

fn whole_document_range(text: &str, encoding: PositionEncoding) -> Json {
    let lines: Vec<&str> = text.split('\n').collect();
    let line = lines.len().saturating_sub(1);
    let last = lines.last().copied().unwrap_or("").trim_end_matches('\r');
    let col = match encoding {
        PositionEncoding::Utf8 => last.len(),
        PositionEncoding::Utf16 => last.encode_utf16().count(),
    };
    lsp_range(0, 0, line, col)
}

fn rpc_invalid(message: impl Into<String>) -> RpcError {
    RpcError {
        code: -32602,
        message: message.into(),
    }
}

fn text_document_uri(params: &Json) -> Option<&str> {
    params.get("textDocument")?.get("uri")?.string()
}

fn position_params(
    params: &Json,
    text: &str,
    encoding: PositionEncoding,
) -> Option<(usize, usize)> {
    let position = params.get("position")?;
    let line = usize::try_from(position.get("line")?.integer()?).ok()?;
    let character = usize::try_from(position.get("character")?.integer()?).ok()?;
    lsp_to_ide_position(text, line, character, encoding)
}

fn lsp_location(
    analysis: &crate::ide::Analysis,
    span: nxc_syntax::Span,
    cfg: &nxc_sema::target::Cfg,
    encoding: PositionEncoding,
) -> Option<Json> {
    Some(obj([
        ("uri", jstr(uri_for_span(analysis, span, cfg)?)),
        ("range", span_range(analysis, span, encoding)?),
    ]))
}

fn completion_kind(name: &str) -> i64 {
    match name {
        "method" => 2,
        "function" => 3,
        "field" => 5,
        "variable" | "parameter" => 6,
        "type" => 7,
        "trait" => 8,
        "module" => 9,
        "enum" => 13,
        "keyword" => 14,
        "variant" => 20,
        "constant" | "static" => 21,
        "struct" => 22,
        _ => 1,
    }
}

fn completion_items(legacy: Json) -> Json {
    let Some(items) = legacy.array() else {
        return Json::Array(Vec::new());
    };
    arr(items.iter().map(|item| {
        let mut fields = vec![
            (
                "label",
                jstr(item.get("label").and_then(Json::string).unwrap_or("")),
            ),
            (
                "kind",
                jnum(completion_kind(
                    item.get("kind").and_then(Json::string).unwrap_or(""),
                )),
            ),
        ];
        if let Some(detail) = item
            .get("detail")
            .and_then(Json::string)
            .filter(|s| !s.is_empty())
        {
            fields.push(("detail", jstr(detail)));
        }
        if let Some(doc) = item
            .get("doc")
            .and_then(Json::string)
            .filter(|s| !s.is_empty())
        {
            fields.push((
                "documentation",
                obj([("kind", jstr("markdown")), ("value", jstr(doc))]),
            ));
        }
        obj(fields)
    }))
}

fn symbol_kind(name: &str) -> i64 {
    match name {
        "module" => 2,
        "function" | "test" => 12,
        "struct" => 23,
        "enum" => 10,
        "trait" => 11,
        "constant" => 14,
        "field" => 8,
        _ => 13,
    }
}

fn symbol_to_lsp(symbol: &Json, text: &str, encoding: PositionEncoding) -> Json {
    let name = symbol.get("name").and_then(Json::string).unwrap_or("");
    let line = symbol
        .get("line")
        .and_then(Json::integer)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(1);
    let col = symbol
        .get("col")
        .and_then(Json::integer)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(1);
    let start = ide_column_to_lsp(text, line, col, encoding).unwrap_or(col.saturating_sub(1));
    let end = start
        + match encoding {
            PositionEncoding::Utf8 => name.len(),
            PositionEncoding::Utf16 => name.encode_utf16().count(),
        };
    let range = lsp_range(line.saturating_sub(1), start, line.saturating_sub(1), end);
    let mut fields = vec![
        ("name", jstr(name)),
        (
            "kind",
            jnum(symbol_kind(
                symbol.get("kind").and_then(Json::string).unwrap_or(""),
            )),
        ),
        ("range", range.clone()),
        ("selectionRange", range),
    ];
    if let Some(detail) = symbol
        .get("detail")
        .and_then(Json::string)
        .filter(|s| !s.is_empty())
    {
        fields.push(("detail", jstr(detail)));
    }
    if let Some(children) = symbol.get("children").and_then(Json::array) {
        if !children.is_empty() {
            fields.push((
                "children",
                arr(children
                    .iter()
                    .map(|child| symbol_to_lsp(child, text, encoding))),
            ));
        }
    }
    obj(fields)
}

fn document_symbols(legacy: Json, text: &str, encoding: PositionEncoding) -> Json {
    let Some(items) = legacy.array() else {
        return Json::Array(Vec::new());
    };
    arr(items.iter().map(|item| symbol_to_lsp(item, text, encoding)))
}

fn test_items(legacy: Json, text: &str, encoding: PositionEncoding) -> Json {
    let Some(items) = legacy.array() else {
        return Json::Array(Vec::new());
    };
    arr(items.iter().map(|item| {
        let name = item.get("name").and_then(Json::string).unwrap_or("");
        let line = item
            .get("line")
            .and_then(Json::integer)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(1);
        let (start, end) = line_bounds(text, line.saturating_sub(1)).unwrap_or((0, 0));
        let line_text = text.get(start..end).unwrap_or("");
        let col = line_text
            .find(name)
            .map(|i| match encoding {
                PositionEncoding::Utf8 => i,
                PositionEncoding::Utf16 => line_text[..i].encode_utf16().count(),
            })
            .unwrap_or(0);
        let length = match encoding {
            PositionEncoding::Utf8 => name.len(),
            PositionEncoding::Utf16 => name.encode_utf16().count(),
        };
        obj([
            ("name", jstr(name)),
            (
                "range",
                lsp_range(
                    line.saturating_sub(1),
                    col,
                    line.saturating_sub(1),
                    col + length,
                ),
            ),
        ])
    }))
}

fn query_cached(job: &Job, cache: &mut AnalysisCache) -> Result<Json, RpcError> {
    let Job::Request {
        document,
        documents,
        cfg,
        cacheable,
        ..
    } = job
    else {
        unreachable!()
    };
    let DocumentSnapshot(doc) = document;

    if *cacheable {
        let mut document_generations = documents
            .iter()
            .map(|DocumentSnapshot(document)| format!("{}#{}", document.uri, document.generation))
            .collect::<Vec<_>>();
        document_generations.sort();
        let fingerprint = format!(
            "{}|release:{}|test:{}|{}",
            cfg.target.name,
            cfg.release,
            cfg.test,
            document_generations.join("|")
        );
        if let Some(cached) = cache.cached.as_mut() {
            if cached.fingerprint == fingerprint {
                if let Some(file) = analysis_file_for_path(&cached.analysis, &doc.path) {
                    cached.analysis.file = file;
                    cached.analysis.document_path = doc.path.clone();
                    return query_with_analysis(job, &cached.analysis);
                }
            }
        }
        cache.cached = None;
        let overlays = documents
            .iter()
            .map(|DocumentSnapshot(document)| (document.path.clone(), document.text.clone()))
            .collect();
        let analysis = crate::ide::analyze_document_with_cfg(
            &doc.path,
            Some(doc.text.clone()),
            overlays,
            cfg.clone(),
        )
        .ok_or_else(|| rpc_invalid(format!("cannot analyze `{}`", doc.path.display())))?;
        cache.cached = Some(CachedAnalysis {
            fingerprint,
            analysis,
        });
        let cached = cache.cached.as_mut().unwrap();
        return query_with_analysis(job, &mut cached.analysis);
    }

    let overlays = documents
        .iter()
        .map(|DocumentSnapshot(d)| (d.path.clone(), d.text.clone()))
        .collect();
    let analysis = crate::ide::analyze_document_with_cfg(
        &doc.path,
        Some(doc.text.clone()),
        overlays,
        cfg.clone(),
    )
    .ok_or_else(|| rpc_invalid(format!("cannot analyze `{}`", doc.path.display())))?;
    query_with_analysis(job, &analysis)
}

fn query_with_analysis(job: &Job, analysis: &crate::ide::Analysis) -> Result<Json, RpcError> {
    let Job::Request {
        method,
        params,
        document,
        cfg,
        encoding,
        ..
    } = job
    else {
        unreachable!()
    };
    let DocumentSnapshot(doc) = document;
    let needs_position = matches!(
        method.as_str(),
        "textDocument/hover"
            | "textDocument/definition"
            | "textDocument/references"
            | "textDocument/prepareRename"
            | "textDocument/rename"
            | "textDocument/completion"
    );
    let position = if needs_position {
        Some(
            position_params(params, &doc.text, *encoding).ok_or_else(|| {
                rpc_invalid("position is outside the document or uses an invalid encoding boundary")
            })?,
        )
    } else {
        None
    };
    let (line, col) = position.unwrap_or((1, 1));
    match method.as_str() {
        "textDocument/hover" => {
            let legacy =
                parse_json(&crate::ide::hover(&analysis, line, col)).map_err(rpc_invalid)?;
            if legacy == Json::Null {
                return Ok(Json::Null);
            }
            let code = legacy.get("code").and_then(Json::string).unwrap_or("");
            let doc = legacy.get("doc").and_then(Json::string).unwrap_or("");
            let value = if doc.is_empty() {
                format!("```nexul\n{}\n```", code)
            } else {
                format!("```nexul\n{}\n```\n\n{}", code, doc)
            };
            Ok(obj([(
                "contents",
                obj([("kind", jstr("markdown")), ("value", jstr(value))]),
            )]))
        }
        "textDocument/completion" => {
            let legacy = parse_json(&crate::ide::completion_with_analysis(
                &analysis, &doc.text, line, col,
            ))
            .map_err(rpc_invalid)?;
            Ok(completion_items(legacy))
        }
        "textDocument/definition" => {
            let offset = analysis.offset(line, col);
            let Some((target, _, _)) = crate::ide::target_at(&analysis, offset) else {
                return Ok(Json::Null);
            };
            let span = crate::ide::definition_span(&analysis, target);
            Ok(lsp_location(&analysis, span, cfg, *encoding).unwrap_or(Json::Null))
        }
        "textDocument/references" => {
            let include_declaration = params
                .get("context")
                .and_then(|c| c.get("includeDeclaration"))
                .and_then(Json::boolean)
                .unwrap_or(false);
            let Some((target, _, _)) = crate::ide::target_at(&analysis, analysis.offset(line, col))
            else {
                return Ok(Json::Array(Vec::new()));
            };
            let definition = crate::ide::definition_span(&analysis, target);
            let mut locations = Vec::new();
            for span in crate::ide::references(&analysis, line, col) {
                if !include_declaration && span == definition {
                    continue;
                }
                if let Some(location) = lsp_location(&analysis, span, cfg, *encoding) {
                    if location
                        .get("uri")
                        .and_then(Json::string)
                        .map_or(false, |uri| uri.starts_with("file:"))
                    {
                        locations.push(location);
                    }
                }
            }
            Ok(arr(locations))
        }
        "textDocument/prepareRename" => {
            let Some((name, span)) = crate::ide::prepare_rename(&analysis, line, col) else {
                return Ok(Json::Null);
            };
            Ok(obj([
                (
                    "range",
                    span_range(&analysis, span, *encoding).unwrap_or(Json::Null),
                ),
                ("placeholder", jstr(name)),
            ]))
        }
        "textDocument/rename" => {
            let name = params
                .get("newName")
                .and_then(Json::string)
                .ok_or_else(|| rpc_invalid("rename requires `newName`"))?;
            if let Err(error) = crate::ide::rename(&analysis, line, col, name) {
                return Err(rpc_invalid(error));
            }
            let Some((target, _, _)) = crate::ide::target_at(&analysis, analysis.offset(line, col))
            else {
                return Err(rpc_invalid("nothing to rename here"));
            };
            let definition = crate::ide::definition_span(&analysis, target);
            let mut changes: BTreeMap<String, Vec<Json>> = BTreeMap::new();
            for span in crate::ide::references(&analysis, line, col) {
                if let Some(uri) =
                    uri_for_span(&analysis, span, cfg).filter(|uri| uri.starts_with("file:"))
                {
                    let edit = obj([
                        (
                            "range",
                            span_range(&analysis, span, *encoding).unwrap_or(Json::Null),
                        ),
                        ("newText", jstr(name)),
                    ]);
                    changes.entry(uri).or_default().push(edit);
                }
            }
            if !changes.values().flatten().any(|_| true) {
                return Err(rpc_invalid("nothing to rename here"));
            }
            let _ = definition;
            Ok(obj([(
                "changes",
                Json::Object(
                    changes
                        .into_iter()
                        .map(|(uri, edits)| (uri, arr(edits)))
                        .collect(),
                ),
            )]))
        }
        "textDocument/documentSymbol" => {
            let legacy = parse_json(&crate::ide::symbols(&analysis)).map_err(rpc_invalid)?;
            Ok(document_symbols(legacy, &doc.text, *encoding))
        }
        "textDocument/formatting" => {
            let formatted = crate::ide::format_source(&doc.text);
            if formatted == doc.text {
                Ok(Json::Array(Vec::new()))
            } else {
                Ok(arr([obj([
                    ("range", whole_document_range(&doc.text, *encoding)),
                    ("newText", jstr(formatted)),
                ])]))
            }
        }
        "nexul/tests" => {
            let legacy = parse_json(&crate::ide::tests(&analysis)).map_err(rpc_invalid)?;
            Ok(test_items(legacy, &doc.text, *encoding))
        }
        _ => Err(RpcError {
            code: -32601,
            message: format!("method `{}` is not supported", method),
        }),
    }
}

fn normalized_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    if let Ok(path) = std::fs::canonicalize(&absolute) {
        return path;
    }
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        use std::path::Component;
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn same_path(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        normalized_path(a)
            .to_string_lossy()
            .eq_ignore_ascii_case(&normalized_path(b).to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        normalized_path(a) == normalized_path(b)
    }
}

fn analysis_file_for_path(
    analysis: &crate::ide::Analysis,
    path: &Path,
) -> Option<nxc_syntax::FileId> {
    analysis.prog.modules.iter().find_map(|module| {
        if module.is_std {
            return None;
        }
        let candidate = source_path(analysis, module.file)?;
        same_path(&candidate, path).then_some(module.file)
    })
}

fn path_field_uri(value: &str) -> Option<String> {
    if value.starts_with("nexul-std:///") {
        Some(value.to_string())
    } else {
        Some(file_uri_from_path(Path::new(value)))
    }
}

fn internal_range_to_lsp(range: &Json, text: Option<&str>, encoding: PositionEncoding) -> Json {
    let line = |key: &str| {
        range
            .get(key)
            .and_then(Json::integer)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(0)
    };
    let start_line = line("line");
    let start_col = line("col");
    let end_line = line("end_line");
    let end_col = line("end_col");
    let (start_char, end_char) = match text {
        Some(text) => (
            ide_column_to_lsp(text, start_line, start_col, encoding).unwrap_or(0),
            ide_column_to_lsp(text, end_line, end_col, encoding).unwrap_or(0),
        ),
        None => (start_col.saturating_sub(1), end_col.saturating_sub(1)),
    };
    lsp_range(
        start_line.saturating_sub(1),
        start_char,
        end_line.saturating_sub(1),
        end_char,
    )
}

fn diagnostic_from_v1(
    diagnostic: &Json,
    text_by_uri: &HashMap<String, String>,
    encoding: PositionEncoding,
) -> Option<(String, Json)> {
    let path = diagnostic.get("path")?.string()?;
    let uri = path_field_uri(path)?;
    let text = text_by_uri.get(&uri).map(String::as_str);
    let range = internal_range_to_lsp(diagnostic.get("range")?, text, encoding);
    let notes = diagnostic.get("notes").and_then(Json::array).unwrap_or(&[]);
    let mut message = diagnostic
        .get("message")
        .and_then(Json::string)
        .unwrap_or("")
        .to_string();
    for note in notes {
        let kind = note.get("kind").and_then(Json::string).unwrap_or("note");
        let content = note.get("message").and_then(Json::string).unwrap_or("");
        message.push_str(&format!("\n{}: {}", kind, content));
    }
    let labels = diagnostic
        .get("labels")
        .and_then(Json::array)
        .unwrap_or(&[]);
    let related = labels
        .iter()
        .filter(|label| label.get("primary").and_then(Json::boolean) != Some(true))
        .filter_map(|label| {
            let label_path = label.get("path")?.string()?;
            let label_uri = path_field_uri(label_path)?;
            let label_text = text_by_uri.get(&label_uri).map(String::as_str);
            Some(obj([
                (
                    "location",
                    obj([
                        ("uri", jstr(label_uri)),
                        (
                            "range",
                            internal_range_to_lsp(label.get("range")?, label_text, encoding),
                        ),
                    ]),
                ),
                (
                    "message",
                    jstr(label.get("message").and_then(Json::string).unwrap_or("")),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    let severity = if diagnostic.get("level").and_then(Json::string) == Some("warning") {
        2
    } else {
        1
    };
    let mut fields = vec![
        ("range", range),
        ("severity", jnum(severity)),
        (
            "code",
            diagnostic.get("code").cloned().unwrap_or(Json::Null),
        ),
        ("source", jstr("nxc")),
        ("message", jstr(message)),
        ("data", obj([("notes", arr(notes.iter().cloned()))])),
    ];
    if !related.is_empty() {
        fields.push(("relatedInformation", arr(related)));
    }
    let code = diagnostic.get("code").and_then(Json::string).unwrap_or("");
    if matches!(code, "W0001" | "W0006") {
        fields.push(("tags", arr([jnum(1)])));
    }
    Some((uri, obj(fields)))
}

fn compile_root(
    analysis: &crate::ide::Analysis,
    documents: &[DocumentSnapshot],
    cfg: &nxc_sema::target::Cfg,
) -> Result<String, String> {
    let root_module = analysis
        .prog
        .modules
        .get(analysis.root.idx())
        .ok_or_else(|| "program has no root module".to_string())?;
    let root_text = analysis.sm.get(root_module.file).text.clone();
    let name = analysis
        .program_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "main.nx".into());
    let dir = analysis.program_path.parent().map(Path::to_path_buf);
    let mut options = CompileOptions::new(Mode::Check);
    options.target = cfg.target.clone();
    options.release = cfg.release;
    options.test_cfg = cfg.test;
    options.deps = crate::pkg::deps_for_lsp(&analysis.program_path)?;
    // Each file in its edition (ADR 0010).
    options.edition = crate::pkg::edition_for(&analysis.program_path, &root_text);
    options.project_edition = Some(crate::pkg::project_edition(&analysis.program_path));
    options.dep_editions = crate::pkg::dep_editions(&options.deps);
    options.overlays = documents
        .iter()
        .map(|DocumentSnapshot(d)| (d.path.clone(), d.text.clone()))
        .collect();
    let compilation = crate::compile_source(&name, root_text.into_bytes(), dir, &options);
    Ok(compilation.json_v1)
}

fn analyze_and_publish(
    documents: &[DocumentSnapshot],
    cfg: &nxc_sema::target::Cfg,
    encoding: PositionEncoding,
) -> Vec<Publication> {
    let document_generations = documents
        .iter()
        .map(|DocumentSnapshot(document)| (document.uri.clone(), document.generation))
        .collect::<Vec<_>>();
    let overlays: Vec<(PathBuf, String)> = documents
        .iter()
        .map(|DocumentSnapshot(d)| (d.path.clone(), d.text.clone()))
        .collect();
    let mut assigned = std::collections::HashSet::new();
    let mut publications = Vec::new();
    for snapshot in documents {
        let DocumentSnapshot(doc) = snapshot;
        if !assigned.insert(doc.uri.clone()) {
            continue;
        }
        let dependency_failure = crate::pkg::find_project(&doc.path).and_then(|project| {
            crate::pkg::deps_for_lsp(&doc.path)
                .err()
                .map(|error| (project, error))
        });
        if let Some((project, error)) = dependency_failure {
            let manifest_path = project.join(crate::pkg::MANIFEST);
            let manifest_document = documents.iter().find_map(|DocumentSnapshot(candidate)| {
                same_path(&candidate.path, &manifest_path).then_some(candidate)
            });
            // find_project canonicalizes its result. Keep the client's alias (8.3
            // paths on Windows, directory symlinks on Unix) when publishing a
            // diagnostic on a manifest that the client has not opened.
            let client_project = doc.path.ancestors()
                .find(|ancestor| same_path(ancestor, &project))
                .unwrap_or(&project);
            let manifest_uri = manifest_document.map(|document| document.uri.clone())
                .unwrap_or_else(|| file_uri_from_path(&client_project.join(crate::pkg::MANIFEST)));
            let mut group = Vec::new();
            for candidate in documents {
                let DocumentSnapshot(candidate_doc) = candidate;
                if normalized_path(&candidate_doc.path).starts_with(&normalized_path(&project))
                    && !same_path(&candidate_doc.path, &manifest_path)
                {
                    assigned.insert(candidate_doc.uri.clone());
                    group.push(candidate_doc);
                }
            }
            for candidate in group {
                publications.push(Publication {
                    uri: candidate.uri.clone(),
                    generation: Some(candidate.generation),
                    version: Some(candidate.version),
                    document_generations: document_generations.clone(),
                    diagnostics: Json::Array(Vec::new()),
                });
            }
            publications.push(Publication {
                uri: manifest_uri,
                generation: manifest_document.map(|document| document.generation),
                version: manifest_document.map(|document| document.version),
                document_generations: document_generations.clone(),
                diagnostics: arr([obj([
                    ("range", lsp_range(0, 0, 0, 0)),
                    ("severity", jnum(1)),
                    ("code", jstr("E0221")),
                    ("source", jstr("nxc")),
                    (
                        "message",
                        jstr(format!(
                            "{}\nhelp: run `nxpm install` after resolving this project dependency; the language server does not fetch packages",
                            error
                        )),
                    ),
                ])]),
            });
            continue;
        }
        let Some(analysis) = crate::ide::analyze_document_with_cfg(
            &doc.path,
            Some(doc.text.clone()),
            overlays.clone(),
            cfg.clone(),
        ) else {
            publications.push(Publication {
                uri: doc.uri.clone(),
                generation: Some(doc.generation),
                version: Some(doc.version),
                document_generations: document_generations.clone(),
                diagnostics: Json::Array(Vec::new()),
            });
            continue;
        };
        let mut user_paths = Vec::new();
        let mut text_by_uri = HashMap::new();
        for module in &analysis.prog.modules {
            let source = analysis.sm.get(module.file);
            if let Some(uri) = standard_uri(&source.name, cfg) {
                text_by_uri.insert(uri, source.text.clone());
            } else if let Some(path) = source_path(&analysis, module.file) {
                let uri = file_uri_from_path(&path);
                text_by_uri.insert(uri, source.text.clone());
                if !module.is_std {
                    user_paths.push(path);
                }
            }
        }
        let mut group = Vec::new();
        for candidate in documents {
            let DocumentSnapshot(candidate_doc) = candidate;
            if user_paths
                .iter()
                .any(|path| same_path(path, &candidate_doc.path))
            {
                assigned.insert(candidate_doc.uri.clone());
                group.push(candidate.clone());
            }
        }
        if group.is_empty() {
            group.push(snapshot.clone());
        }
        let mut diagnostics_by_uri: HashMap<String, Vec<Json>> = group
            .iter()
            .map(|DocumentSnapshot(d)| (d.uri.clone(), Vec::new()))
            .collect();
        match compile_root(&analysis, documents, cfg)
            .and_then(|json| parse_json(&json).map_err(|e| format!("invalid compiler JSON: {}", e)))
        {
            Ok(envelope) => {
                if let Some(items) = envelope.get("diagnostics").and_then(Json::array) {
                    for item in items {
                        if let Some((uri, diagnostic)) =
                            diagnostic_from_v1(item, &text_by_uri, encoding)
                        {
                            // Compiler JSON uses canonical paths, whereas LSP must
                            // publish on the exact URI supplied by didOpen. Route by
                            // physical file identity instead of dropping aliases.
                            let path = path_from_file_uri(&uri);
                            for DocumentSnapshot(document) in &group {
                                if uri == document.uri || path.as_ref()
                                    .is_some_and(|path| same_path(path, &document.path))
                                {
                                    diagnostics_by_uri.get_mut(&document.uri).unwrap()
                                        .push(diagnostic.clone());
                                }
                            }
                        }
                    }
                }
            }
            Err(error) => {
                /* Missing dependencies are logged; normal source diagnostics still publish. */
                let _ = error;
            }
        }
        for DocumentSnapshot(doc) in group {
            publications.push(Publication {
                uri: doc.uri.clone(),
                generation: Some(doc.generation),
                version: Some(doc.version),
                document_generations: document_generations.clone(),
                diagnostics: arr(diagnostics_by_uri.remove(&doc.uri).unwrap_or_default()),
            });
        }
    }
    publications
}

fn worker_loop(jobs: Receiver<Job>, events: Sender<Event>, logger: Arc<Logger>) {
    let mut cache = AnalysisCache::default();
    while let Ok(job) = jobs.recv() {
        match job {
            Job::Stop => break,
            Job::InvalidateCache => cache.cached = None,
            request @ Job::Request { .. } => {
                let key = match &request {
                    Job::Request { key, .. } => key.clone(),
                    _ => unreachable!(),
                };
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    query_cached(&request, &mut cache)
                }))
                .unwrap_or_else(|_| {
                    logger.log(format!("panic while handling request {}", key));
                    Err(RpcError {
                        code: -32603,
                        message: "internal error while analyzing the request".into(),
                    })
                });
                if events
                    .send(Event::Worker(WorkerEvent::Response { key, result }))
                    .is_err()
                {
                    break;
                }
            }
            Job::Publish {
                documents,
                cfg,
                encoding,
            } => {
                let published = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    analyze_and_publish(&documents, &cfg, encoding)
                }))
                .unwrap_or_else(|_| {
                    logger.log("panic while publishing diagnostics");
                    Vec::new()
                });
                if events
                    .send(Event::Worker(WorkerEvent::Published(published)))
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}

fn choose_initialization(
    params: &Json,
) -> Result<(nxc_sema::target::Cfg, PositionEncoding), String> {
    let options = params.get("initializationOptions");
    let target = match options.and_then(|o| o.get("target")).and_then(Json::string) {
        Some(name) => {
            Target::from_name(name).map_err(|e| format!("invalid initialization target: {}", e))?
        }
        None => Target::host(),
    };
    let mode = options
        .and_then(|o| o.get("mode"))
        .and_then(Json::string)
        .unwrap_or("debug");
    let (release, test) = match mode {
        "debug" => (false, false),
        "release" => (true, false),
        "test" => (false, true),
        _ => return Err("initialization mode must be debug, release, or test".into()),
    };
    let encodings = params
        .get("capabilities")
        .and_then(|c| c.get("general"))
        .and_then(|g| g.get("positionEncodings"))
        .and_then(Json::array)
        .unwrap_or(&[]);
    let encoding = if encodings.iter().any(|e| e.string() == Some("utf-8")) {
        PositionEncoding::Utf8
    } else {
        PositionEncoding::Utf16
    };
    Ok((
        nxc_sema::target::Cfg {
            target,
            release,
            test,
        },
        encoding,
    ))
}

fn supports_dynamic_watch(params: &Json) -> bool {
    params
        .get("capabilities")
        .and_then(|c| c.get("workspace"))
        .and_then(|w| w.get("didChangeWatchedFiles"))
        .and_then(|w| w.get("dynamicRegistration"))
        .and_then(Json::boolean)
        == Some(true)
}

fn initialize_result(encoding: PositionEncoding) -> Json {
    let encoding_name = match encoding {
        PositionEncoding::Utf8 => "utf-8",
        PositionEncoding::Utf16 => "utf-16",
    };
    let capabilities = obj([
        ("positionEncoding", jstr(encoding_name)),
        (
            "textDocumentSync",
            obj([
                ("openClose", Json::Bool(true)),
                ("change", jnum(2)),
                ("save", obj([("includeText", Json::Bool(false))])),
            ]),
        ),
        ("hoverProvider", Json::Bool(true)),
        (
            "completionProvider",
            obj([
                ("triggerCharacters", arr([jstr(".")])),
                ("resolveProvider", Json::Bool(false)),
            ]),
        ),
        ("definitionProvider", Json::Bool(true)),
        ("referencesProvider", Json::Bool(true)),
        (
            "renameProvider",
            obj([("prepareProvider", Json::Bool(true))]),
        ),
        ("documentSymbolProvider", Json::Bool(true)),
        ("documentFormattingProvider", Json::Bool(true)),
        (
            "experimental",
            obj([(
                "nexul",
                obj([
                    ("jsonVersion", jnum(1)),
                    ("tests", Json::Bool(true)),
                    ("stdSource", Json::Bool(true)),
                ]),
            )]),
        ),
    ]);
    obj([
        ("capabilities", capabilities),
        (
            "serverInfo",
            obj([
                ("name", jstr("nxc")),
                ("version", jstr(env!("CARGO_PKG_VERSION"))),
            ]),
        ),
    ])
}

fn server_request(id: Json, method: &str, params: Json) -> Json {
    obj([
        ("jsonrpc", jstr("2.0")),
        ("id", id),
        ("method", jstr(method)),
        ("params", params),
    ])
}

/// Runs the Language Server Protocol process. Returns its exit code after `exit` or EOF.
pub fn run(args: &[String]) -> i32 {
    let log_path = match args {
        [] => None,
        [flag, path] if flag == "--log" => Some(PathBuf::from(path)),
        _ => {
            eprintln!("usage: nxc lsp [--log <file>]");
            return 2;
        }
    };
    let log_file = match log_path {
        Some(path) => match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(file) => Some(file),
            Err(error) => {
                eprintln!("cannot open LSP log `{}`: {}", path.display(), error);
                return 2;
            }
        },
        None => None,
    };
    let logger = Arc::new(Logger(Mutex::new(log_file)));
    let panic_log = logger.clone();
    std::panic::set_hook(Box::new(move |info| {
        panic_log.log(format!("panic: {}", info))
    }));

    let (events_tx, events_rx) = mpsc::channel();
    let (jobs_tx, jobs_rx) = mpsc::channel();
    let input_tx = events_tx.clone();
    let input_logger = logger.clone();
    let _reader = match thread::Builder::new()
        .name("nxc-lsp-reader".into())
        .spawn(move || input_loop(input_tx, input_logger))
    {
        Ok(thread) => thread,
        Err(error) => {
            logger.log(format!("cannot start LSP reader: {}", error));
            return 1;
        }
    };
    let worker_tx = events_tx.clone();
    let worker_logger = logger.clone();
    let _worker = match thread::Builder::new()
        .name("nxc-lsp-analysis".into())
        .stack_size(256 << 20)
        .spawn(move || worker_loop(jobs_rx, worker_tx, worker_logger))
    {
        Ok(thread) => thread,
        Err(error) => {
            logger.log(format!("cannot start LSP worker: {}", error));
            return 1;
        }
    };

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let mut runtime = Runtime::new();
    loop {
        let now = Instant::now();
        let due = runtime
            .diagnostic_due
            .values()
            .copied()
            .filter(|time| *time <= now)
            .count();
        if due > 0 {
            runtime.diagnostic_due.retain(|_, time| *time > now);
            let documents = runtime.snapshots();
            if !documents.is_empty() {
                if jobs_tx
                    .send(Job::Publish {
                        documents,
                        cfg: runtime.cfg.clone(),
                        encoding: runtime.encoding,
                    })
                    .is_err()
                {
                    logger.log("analysis worker stopped before diagnostics could be scheduled");
                }
            }
        }
        let timeout = runtime
            .diagnostic_due
            .values()
            .copied()
            .min()
            .map(|when| {
                when.saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(100))
            })
            .unwrap_or(Duration::from_millis(100));
        match events_rx.recv_timeout(timeout) {
            Ok(Event::Input(Ok(message))) => {
                if !handle_message(message, &mut runtime, &jobs_tx, &mut writer, &logger) {
                    break;
                }
            }
            Ok(Event::Input(Err(error))) => {
                logger.log(format!("invalid message: {}", error));
                let response = rpc_error(Json::Null, -32700, "Parse error");
                if write_frame(&mut writer, &response).is_err() {
                    break;
                }
            }
            Ok(Event::Eof) => {
                runtime.exit_code = 0;
                break;
            }
            Ok(Event::Worker(WorkerEvent::Response { key, result })) => {
                if let Some(pending) = runtime.pending.remove(&key) {
                    let message = response_for_pending(&runtime, pending, result);
                    if write_frame(&mut writer, &message).is_err() {
                        break;
                    }
                }
            }
            Ok(Event::Worker(WorkerEvent::Published(publications))) => {
                for publication in publications {
                    if publication_is_stale(&runtime, &publication) {
                        continue;
                    }
                    let mut params = vec![
                        ("uri", jstr(&publication.uri)),
                        ("diagnostics", publication.diagnostics),
                    ];
                    if let Some(version) = publication.version {
                        params.push(("version", jnum(version)));
                    }
                    let message = notification("textDocument/publishDiagnostics", obj(params));
                    if write_frame(&mut writer, &message).is_err() {
                        return 1;
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                runtime.exit_code = 0;
                break;
            }
        }
        if runtime.exited {
            break;
        }
    }
    let _ = jobs_tx.send(Job::Stop);
    let _ = writer.flush();
    runtime.exit_code
}

fn handle_message<W: Write>(
    message: Json,
    runtime: &mut Runtime,
    jobs: &Sender<Job>,
    writer: &mut W,
    logger: &Logger,
) -> bool {
    let Json::Object(_) = message else {
        let _ = write_frame(writer, &rpc_error(Json::Null, -32600, "Invalid Request"));
        return true;
    };
    if message.get("jsonrpc").and_then(Json::string) != Some("2.0") {
        let _ = write_frame(
            writer,
            &rpc_error(
                message.get("id").cloned().unwrap_or(Json::Null),
                -32600,
                "Invalid Request",
            ),
        );
        return true;
    }
    let Some(method) = message.get("method").and_then(Json::string) else {
        // Responses to server-initiated capability-registration requests need no action.
        return true;
    };
    let id = message.get("id").cloned();
    let params = message
        .get("params")
        .cloned()
        .unwrap_or_else(|| Json::Object(BTreeMap::new()));

    if method == "exit" {
        runtime.exit_code = if runtime.shutdown { 0 } else { 1 };
        runtime.exited = true;
        return false;
    }
    if method == "initialize" {
        let Some(id) = id else {
            logger.log("ignored initialize notification");
            return true;
        };
        match choose_initialization(&params) {
            Ok((cfg, encoding)) => {
                runtime.cfg = cfg;
                runtime.encoding = encoding;
                runtime.initialized = true;
                let _ = write_frame(writer, &rpc_response(id, initialize_result(encoding)));
                if supports_dynamic_watch(&params) {
                    let request_id = jstr(format!("nxc-watch-{}", runtime.next_server_id));
                    runtime.next_server_id += 1;
                    let registration = obj([
                        ("id", jstr("nxc-watch-files-v1")),
                        ("method", jstr("workspace/didChangeWatchedFiles")),
                        (
                            "registerOptions",
                            obj([(
                                "watchers",
                                arr([
                                    obj([("globPattern", jstr("**/*.nx"))]),
                                    obj([("globPattern", jstr("**/nexul.toml"))]),
                                ]),
                            )]),
                        ),
                    ]);
                    let request = server_request(
                        request_id,
                        "client/registerCapability",
                        obj([("registrations", arr([registration]))]),
                    );
                    let _ = write_frame(writer, &request);
                }
            }
            Err(error) => {
                let _ = write_frame(writer, &rpc_error(id, -32602, error));
            }
        }
        return true;
    }
    if method == "initialized" {
        return true;
    }
    if method == "shutdown" {
        if let Some(id) = id {
            if !runtime.initialized {
                let _ = write_frame(writer, &rpc_error(id, -32002, "Server not initialized"));
            } else {
                runtime.shutdown = true;
                let _ = write_frame(writer, &rpc_response(id, Json::Null));
            }
        }
        return true;
    }
    if method == "$/cancelRequest" {
        let Some(cancelled_id) = params.get("id") else {
            return true;
        };
        let key = request_key(cancelled_id);
        if let Some(pending) = runtime.pending.remove(&key) {
            let _ = write_frame(writer, &rpc_error(pending.id, -32800, "request cancelled"));
            logger.log(format!("cancelled request {}", key));
        }
        return true;
    }

    match method {
        "textDocument/didOpen" => {
            did_open(&params, runtime, logger);
            return true;
        }
        "textDocument/didChange" => {
            did_change(&params, runtime, logger);
            return true;
        }
        "textDocument/didClose" => {
            did_close(&params, runtime, writer, logger);
            return true;
        }
        "textDocument/didSave" => {
            if let Some(uri) = text_document_uri(&params) {
                if runtime.documents.contains_key(uri) {
                    runtime
                        .diagnostic_due
                        .insert(uri.to_string(), Instant::now() + DIAGNOSTIC_DELAY);
                }
            }
            return true;
        }
        "workspace/didChangeWatchedFiles" => {
            if jobs.send(Job::InvalidateCache).is_err() {
                logger.log("analysis worker stopped before cache invalidation");
            }
            runtime.schedule_all();
            return true;
        }
        _ => {}
    }
    let Some(id) = id else {
        return true;
    };
    if !runtime.initialized {
        let _ = write_frame(writer, &rpc_error(id, -32002, "Server not initialized"));
        return true;
    }
    if runtime.shutdown {
        let _ = write_frame(writer, &rpc_error(id, -32600, "server is shutting down"));
        return true;
    }
    if !matches!(
        method,
        "textDocument/hover"
            | "textDocument/completion"
            | "textDocument/definition"
            | "textDocument/references"
            | "textDocument/prepareRename"
            | "textDocument/rename"
            | "textDocument/documentSymbol"
            | "textDocument/formatting"
            | "nexul/tests"
            | "nexul/stdSource"
    ) {
        let _ = write_frame(
            writer,
            &rpc_error(id, -32601, format!("method `{}` is not supported", method)),
        );
        return true;
    }
    if method == "nexul/stdSource" {
        let uri = params.get("uri").and_then(Json::string);
        let result = uri.and_then(std_module_from_uri).and_then(|module| {
            nxc_sema::load::std_source(&module, &runtime.cfg)
                .map(|text| obj([("text", jstr(text))]))
        });
        match result {
            Some(value) => {
                let _ = write_frame(writer, &rpc_response(id, value));
            }
            None => {
                let _ = write_frame(
                    writer,
                    &rpc_error(id, -32602, "unknown standard-library URI"),
                );
            }
        }
        return true;
    }
    let Some(uri) = text_document_uri(&params).map(str::to_string) else {
        let _ = write_frame(
            writer,
            &rpc_error(id, -32602, "request requires textDocument.uri"),
        );
        return true;
    };
    let document = if let Some(doc) = runtime.documents.get(&uri) {
        (doc.clone(), true)
    } else {
        let Some(path) = path_from_file_uri(&uri) else {
            let _ = write_frame(
                writer,
                &rpc_error(id, -32602, "document URI must use file://"),
            );
            return true;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => (
                Document {
                    uri: uri.clone(),
                    path,
                    text,
                    version: 0,
                    generation: runtime.generations.get(&uri).copied().unwrap_or(0),
                },
                false,
            ),
            Err(error) => {
                let _ = write_frame(
                    writer,
                    &rpc_error(id, -32602, format!("cannot read document: {}", error)),
                );
                return true;
            }
        }
    };
    let (document, is_open) = document;
    let key = request_key(&id);
    let documents = runtime.snapshots();
    let document_generations = documents
        .iter()
        .map(|DocumentSnapshot(document)| (document.uri.clone(), document.generation))
        .collect();
    runtime.pending.insert(
        key.clone(),
        PendingRequest {
            id,
            uri: uri.clone(),
            version: is_open.then_some(document.version),
            generation: document.generation,
            document_generations,
        },
    );
    let job = Job::Request {
        key: key.clone(),
        method: method.to_string(),
        params,
        document: DocumentSnapshot(document),
        documents,
        cfg: runtime.cfg.clone(),
        encoding: runtime.encoding,
        cacheable: is_open,
    };
    if jobs.send(job).is_err() {
        if let Some(pending) = runtime.pending.remove(&key) {
            let _ = write_frame(
                writer,
                &rpc_error(pending.id, -32603, "analysis worker is unavailable"),
            );
        }
    }
    true
}

fn did_open(params: &Json, runtime: &mut Runtime, logger: &Logger) {
    let Some(document) = params.get("textDocument") else {
        logger.log("didOpen missing textDocument");
        return;
    };
    let Some(uri) = document.get("uri").and_then(Json::string) else {
        logger.log("didOpen missing URI");
        return;
    };
    let Some(path) = path_from_file_uri(uri) else {
        logger.log(format!("ignored non-file URI {}", uri));
        return;
    };
    let Some(text) = document.get("text").and_then(Json::string) else {
        logger.log(format!("didOpen missing source text for {}", uri));
        return;
    };
    let version = document.get("version").and_then(Json::integer).unwrap_or(0);
    let generation = runtime.bump_generation(uri);
    runtime.documents.insert(
        uri.to_string(),
        Document {
            uri: uri.to_string(),
            path,
            text: text.to_string(),
            version,
            generation,
        },
    );
    runtime.schedule_all();
}

fn did_change(params: &Json, runtime: &mut Runtime, logger: &Logger) {
    let Some(document_id) = params.get("textDocument") else {
        logger.log("didChange missing textDocument");
        return;
    };
    let Some(uri) = document_id.get("uri").and_then(Json::string) else {
        logger.log("didChange missing URI");
        return;
    };
    let Some(current) = runtime.documents.get(uri).cloned() else {
        logger.log(format!("didChange for unopened document {}", uri));
        return;
    };
    let version = document_id
        .get("version")
        .and_then(Json::integer)
        .unwrap_or(current.version + 1);
    if version <= current.version {
        logger.log(format!(
            "ignored stale document version {} for {}",
            version, uri
        ));
        return;
    }
    let Some(changes) = params.get("contentChanges").and_then(Json::array) else {
        logger.log(format!("didChange missing contentChanges for {}", uri));
        return;
    };
    let mut text = current.text.clone();
    for change in changes {
        let Some(replacement) = change.get("text").and_then(Json::string) else {
            logger.log(format!("didChange has no text for {}", uri));
            return;
        };
        if let Some(range) = change.get("range") {
            let Some(start) = json_position(range.get("start")) else {
                logger.log(format!("invalid change start for {}", uri));
                return;
            };
            let Some(end) = json_position(range.get("end")) else {
                logger.log(format!("invalid change end for {}", uri));
                return;
            };
            let Some(start_byte) = lsp_position_to_byte(&text, start.0, start.1, runtime.encoding)
            else {
                logger.log(format!("change start is outside {}", uri));
                return;
            };
            let Some(end_byte) = lsp_position_to_byte(&text, end.0, end.1, runtime.encoding) else {
                logger.log(format!("change end is outside {}", uri));
                return;
            };
            if end_byte < start_byte {
                logger.log(format!("change range is reversed for {}", uri));
                return;
            }
            text.replace_range(start_byte..end_byte, replacement);
        } else {
            text = replacement.to_string();
        }
    }
    let generation = runtime.bump_generation(uri);
    if let Some(doc) = runtime.documents.get_mut(uri) {
        doc.text = text;
        doc.version = version;
        doc.generation = generation;
    }
    runtime.schedule_all();
}

fn json_position(value: Option<&Json>) -> Option<(usize, usize)> {
    let value = value?;
    let line = usize::try_from(value.get("line")?.integer()?).ok()?;
    let character = usize::try_from(value.get("character")?.integer()?).ok()?;
    Some((line, character))
}

fn did_close<W: Write>(params: &Json, runtime: &mut Runtime, writer: &mut W, logger: &Logger) {
    let Some(uri) = text_document_uri(params) else {
        logger.log("didClose missing textDocument.uri");
        return;
    };
    runtime.documents.remove(uri);
    runtime.diagnostic_due.remove(uri);
    runtime.bump_generation(uri);
    let message = notification(
        "textDocument/publishDiagnostics",
        obj([("uri", jstr(uri)), ("diagnostics", Json::Array(Vec::new()))]),
    );
    let _ = write_frame(writer, &message);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logger() -> Logger {
        Logger(Mutex::new(None))
    }

    #[test]
    fn incremental_positions_handle_utf8_utf16_emoji_and_crlf() {
        let text = "// café 😀\r\nx";
        assert_eq!(
            lsp_position_to_byte(text, 0, 8, PositionEncoding::Utf16),
            Some("// café ".len())
        );
        assert_eq!(
            lsp_position_to_byte(text, 0, 10, PositionEncoding::Utf16),
            Some("// café 😀".len())
        );
        assert_eq!(
            lsp_position_to_byte(text, 0, 9, PositionEncoding::Utf8),
            Some("// café ".len())
        );
        assert_eq!(
            lsp_position_to_byte(text, 0, 13, PositionEncoding::Utf8),
            Some("// café 😀".len())
        );
        assert_eq!(
            lsp_to_ide_position(text, 0, 10, PositionEncoding::Utf16),
            Some((1, 10))
        );
        assert_eq!(
            lsp_to_ide_position(text, 1, 0, PositionEncoding::Utf16),
            Some((2, 1))
        );
        assert_eq!(
            lsp_position_to_byte(text, 0, 9, PositionEncoding::Utf16),
            None,
            "a UTF-16 position cannot split the emoji surrogate pair"
        );
    }

    #[test]
    fn document_symbol_ranges_convert_unicode_prefixes_to_negotiated_units() {
        let symbols = parse_json(
            "[{\"name\":\"main\",\"kind\":\"function\",\"line\":1,\"col\":3,\"children\":[]}]",
        )
        .unwrap();
        let utf8 = document_symbols(symbols.clone(), "é main", PositionEncoding::Utf8).encode();
        let utf16 = document_symbols(symbols, "é main", PositionEncoding::Utf16).encode();
        assert!(utf8.contains("\"character\":3"), "{utf8}");
        assert!(utf16.contains("\"character\":2"), "{utf16}");
    }

    #[test]
    fn file_uris_round_trip_spaces_and_unicode_and_decode_unc_paths() {
        let path = std::env::temp_dir().join("NEXUL space café.nx");
        let uri = file_uri_from_path(&path);
        assert!(uri.contains("%20") && uri.contains("caf%C3%A9"), "{uri}");
        assert!(same_path(&path, &path_from_file_uri(&uri).unwrap()));
        #[cfg(windows)]
        assert_eq!(
            path_from_file_uri("file://server/share/NEXUL.nx").unwrap(),
            PathBuf::from(r"\\server\share\NEXUL.nx")
        );
    }

    #[test]
    fn cancellation_removes_pending_request_and_responds_immediately() {
        let mut runtime = Runtime::new();
        runtime.initialized = true;
        let uri = file_uri_from_path(Path::new("main.nx"));
        let document = Document {
            uri: uri.clone(),
            path: PathBuf::from("main.nx"),
            text: "fn main() {}".into(),
            version: 1,
            generation: 1,
        };
        runtime.documents.insert(uri.clone(), document);
        runtime.generations.insert(uri.clone(), 1);
        let (jobs, queued) = mpsc::channel();
        let mut output = Vec::new();
        let request = parse_json(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":{}}},\"position\":{{\"line\":0,\"character\":3}}}}}}",
            crate::json_str(&uri)
        ))
        .unwrap();
        assert!(handle_message(
            request,
            &mut runtime,
            &jobs,
            &mut output,
            &logger()
        ));
        assert!(runtime.pending.contains_key("7"));
        assert!(matches!(queued.try_recv(), Ok(Job::Request { .. })));

        let cancel = parse_json(
            "{\"jsonrpc\":\"2.0\",\"method\":\"$/cancelRequest\",\"params\":{\"id\":7}}",
        )
        .unwrap();
        assert!(handle_message(
            cancel,
            &mut runtime,
            &jobs,
            &mut output,
            &logger()
        ));
        assert!(!runtime.pending.contains_key("7"));
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("\"code\":-32800"));
    }

    #[test]
    fn changed_document_returns_content_modified_and_stale_cross_file_diagnostics_are_dropped() {
        let mut runtime = Runtime::new();
        let main_uri = "file:///main.nx".to_string();
        let calc_uri = "file:///calc.nx".to_string();
        runtime.documents.insert(
            main_uri.clone(),
            Document {
                uri: main_uri.clone(),
                path: PathBuf::from("main.nx"),
                text: "fn main() {}".into(),
                version: 1,
                generation: 1,
            },
        );
        runtime.documents.insert(
            calc_uri.clone(),
            Document {
                uri: calc_uri.clone(),
                path: PathBuf::from("calc.nx"),
                text: "pub fn value() {}".into(),
                version: 2,
                generation: 2,
            },
        );
        runtime.generations.insert(main_uri.clone(), 1);
        runtime.generations.insert(calc_uri.clone(), 2);
        let response = response_for_pending(
            &runtime,
            PendingRequest {
                id: jnum(3),
                uri: calc_uri.clone(),
                version: Some(1),
                generation: 1,
                document_generations: vec![(calc_uri.clone(), 1)],
            },
            Ok(Json::Null),
        );
        assert!(response.encode().contains("\"code\":-32801"));
        let stale = Publication {
            uri: main_uri.clone(),
            generation: Some(1),
            version: Some(1),
            document_generations: vec![(main_uri, 1), (calc_uri, 1)],
            diagnostics: Json::Array(Vec::new()),
        };
        assert!(publication_is_stale(&runtime, &stale));
    }
}

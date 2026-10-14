//! NEXUL Studio: a local editor for NEXUL with a live web server.
//!
//! The program starts a server on 127.0.0.1 and opens the interface in an application
//! window (Microsoft Edge in app mode, or the default browser). The server gives the
//! interface access to the files of the open folder, runs the NEXUL compiler, and serves
//! web pages with automatic reloading ("Live Server").
//!
//! Security: the server only listens on 127.0.0.1, checks the `Host` header (DNS
//! rebinding), and every API call carries a random token in a custom header (which other
//! web sites cannot send without a CORS authorization that is never granted).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod http;

use nexul_studio::{debug, runner, watch, workspace};

use http::{json_field, json_str, mime, read_request, write_response, Request, Response};
use runner::{command, Runs, Step};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

const INDEX_HTML: &str = include_str!("../ui/index.html");
const STYLE_CSS: &str = include_str!("../ui/style.css");
const LANGS_JS: &str = include_str!("../ui/langs.js");
const EDITOR_JS: &str = include_str!("../ui/editor.js");
const APP_JS: &str = include_str!("../ui/app.js");
const IDE_JS: &str = include_str!("../ui/ide.js");
const PLUGINS_JS: &str = include_str!("../ui/plugins.js");

/// Directories never shown nor searched.
const IGNORED: &[&str] = watch::IGNORED;

struct State {
    token: String,
    port: u16,
    root: Mutex<Option<PathBuf>>,
    nxc: Mutex<Option<PathBuf>>,
    live_version: AtomicU64,
    last_ping: Mutex<Option<Instant>>,
    runs: Mutex<Runs>,
    tmp_counter: AtomicU64,
    debugger: Mutex<Option<debug::Debugger>>,
    /// A file to open at start (given on the command line), relative to `root`.
    start_file: Mutex<Option<String>>,
    /// The watch of the open folder, and the changes it reported: (version, path).
    watcher: Mutex<Option<watch::Watcher>>,
    tree_version: AtomicU64,
    changes: Mutex<Vec<(u64, String)>>,
}

fn random_token() -> String {
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let dir = base.join("NEXUL Studio");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Finds the NEXUL compiler: next to the Studio (installation), then the environment
/// and a development checkout.
fn find_nxc() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("nxc.exe"));
            candidates.push(dir.join("bin").join("nxc.exe"));
        }
    }
    if let Some(p) = std::env::var_os("NEXUL_NXC") {
        candidates.push(PathBuf::from(p));
    }
    if let Some(home) = std::env::var_os("NEXUL_HOME") {
        candidates.push(Path::new(&home).join("bin").join("nxc.exe"));
        candidates.push(Path::new(&home).join("target").join("release").join("nxc.exe"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join("nxc.exe")));
    }
    // A build of the repository (developers).
    candidates.push(PathBuf::from(r"C:\NEXUL\target\release\nxc.exe"));
    candidates.push(PathBuf::from(r"C:\NEXUL\target\debug\nxc.exe"));
    candidates.into_iter().find(|p| p.is_file())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let no_window = args.iter().any(|a| a == "--no-window");
    let port_arg: Option<u16> = args.iter().position(|a| a == "--port").and_then(|i| args.get(i + 1)).and_then(|p| p.parse().ok());
    let listener = match port_arg {
        Some(p) => TcpListener::bind(("127.0.0.1", p)),
        None => TcpListener::bind(("127.0.0.1", 4719)).or_else(|_| TcpListener::bind(("127.0.0.1", 0))),
    }
    .expect("cannot listen on 127.0.0.1");
    let port = listener.local_addr().unwrap().port();
    let state = Arc::new(State {
        token: random_token(),
        port,
        root: Mutex::new(None),
        nxc: Mutex::new(find_nxc()),
        live_version: AtomicU64::new(1),
        last_ping: Mutex::new(None),
        runs: Mutex::new(Runs::default()),
        tmp_counter: AtomicU64::new(0),
        debugger: Mutex::new(None),
        start_file: Mutex::new(None),
        watcher: Mutex::new(None),
        tree_version: AtomicU64::new(1),
        changes: Mutex::new(Vec::new()),
    });
    // `NEXUL Studio.exe <folder>` or `<file>`: open it (file association of `.nx`).
    if let Some(arg) = args.iter().skip(1).find(|a| !a.starts_with("--") && a.parse::<u16>().is_err()) {
        let p = PathBuf::from(arg);
        let p = std::fs::canonicalize(&p).map(|c| strip_verbatim(&c)).unwrap_or(p);
        if p.is_dir() {
            set_root(&state, Some(p));
        } else if p.is_file() {
            if let (Some(dir), Some(name)) = (p.parent(), p.file_name()) {
                // The project of the file (nexul.toml), else its folder.
                let mut root = dir.to_path_buf();
                let mut probe = dir.to_path_buf();
                loop {
                    if probe.join("nexul.toml").is_file() {
                        root = probe.clone();
                        break;
                    }
                    if !probe.pop() {
                        break;
                    }
                }
                let rel = p.strip_prefix(&root).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_else(|_| name.to_string_lossy().to_string());
                set_root(&state, Some(root));
                *state.start_file.lock().unwrap() = Some(rel);
            }
        }
    }
    let url = format!("http://127.0.0.1:{}/?t={}", port, state.token);

    // Watchdog: the server stops a little after the window is closed (or if the window
    // never connected).
    if !no_window {
        let st = state.clone();
        let started = Instant::now();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(2));
            let gone = match *st.last_ping.lock().unwrap() {
                Some(t) => t.elapsed() > Duration::from_secs(12),
                None => started.elapsed() > Duration::from_secs(120),
            };
            if gone {
                std::process::exit(0);
            }
        });
    }
    if no_window {
        println!("NEXUL Studio: {}", url);
    } else {
        open_window(&url);
    }
    for stream in listener.incoming().flatten() {
        let st = state.clone();
        std::thread::spawn(move || handle(stream, st));
    }
}

/// Opens the interface in an application window (Edge app mode), else the browser.
fn open_window(url: &str) {
    let edge_paths = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ];
    let profile = config_dir().join("window");
    for p in edge_paths {
        if Path::new(p).is_file() {
            let ok = std::process::Command::new(p)
                .arg(format!("--app={}", url))
                .arg(format!("--user-data-dir={}", profile.display()))
                .arg("--window-size=1440,900")
                .arg("--no-first-run")
                .arg("--no-default-browser-check")
                .spawn()
                .is_ok();
            if ok {
                return;
            }
        }
    }
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
}

/// Opens a folder (or none): the explorer, the editors and the web preview follow the
/// changes made to its files, inside or outside of Studio.
fn set_root(st: &Arc<State>, root: Option<PathBuf>) {
    *st.watcher.lock().unwrap() = None;
    st.changes.lock().unwrap().clear();
    st.tree_version.fetch_add(1, Ordering::SeqCst);
    if let Some(r) = &root {
        let weak = Arc::downgrade(st);
        let w = watch::Watcher::start(r, move |paths| {
            let Some(st) = weak.upgrade() else { return };
            let v = st.tree_version.fetch_add(1, Ordering::SeqCst) + 1;
            st.live_version.fetch_add(1, Ordering::SeqCst);
            let mut ch = st.changes.lock().unwrap();
            if paths.is_empty() {
                ch.push((v, String::new()));
            }
            for p in paths {
                ch.push((v, p));
            }
            let n = ch.len();
            if n > 2000 {
                ch.drain(..n - 1000);
            }
        });
        *st.watcher.lock().unwrap() = w.ok();
    }
    *st.root.lock().unwrap() = root;
}

fn handle(stream: TcpStream, st: Arc<State>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let Some(req) = read_request(&stream) else { return };
    let resp = route(&req, &st);
    write_response(&stream, &resp);
}

fn host_ok(req: &Request, st: &State) -> bool {
    match req.header("host") {
        Some(h) => h == format!("127.0.0.1:{}", st.port) || h == format!("localhost:{}", st.port),
        None => false,
    }
}

fn route(req: &Request, st: &Arc<State>) -> Response {
    if !host_ok(req, st) {
        return Response::text(403, "forbidden host");
    }
    let path = req.path.as_str();
    match path {
        "/" => {
            if req.q("t") != st.token {
                return Response::text(403, "Ouvrez NEXUL Studio depuis son raccourci.");
            }
            let html = String::from_utf8_lossy(&ui_file("index.html", INDEX_HTML)).replace("__NX_TOKEN__", &st.token);
            Response::new(200, "text/html; charset=utf-8", html)
        }
        "/static/style.css" => Response::new(200, "text/css; charset=utf-8", ui_file("style.css", STYLE_CSS)),
        "/static/langs.js" => Response::new(200, "text/javascript; charset=utf-8", ui_file("langs.js", LANGS_JS)),
        "/static/editor.js" => Response::new(200, "text/javascript; charset=utf-8", ui_file("editor.js", EDITOR_JS)),
        "/static/app.js" => Response::new(200, "text/javascript; charset=utf-8", ui_file("app.js", APP_JS)),
        "/static/ide.js" => Response::new(200, "text/javascript; charset=utf-8", ui_file("ide.js", IDE_JS)),
        "/static/plugins.js" => Response::new(200, "text/javascript; charset=utf-8", ui_file("plugins.js", PLUGINS_JS)),
        "/live-reload" => Response::text(200, &st.live_version.load(Ordering::SeqCst).to_string()),
        "/favicon.ico" => Response::new(200, "image/x-icon", FAVICON),
        _ if path.starts_with("/live/") || path == "/live" => live(req, st),
        _ if path.starts_with("/api/") => {
            if req.header("x-nexul-token") != Some(st.token.as_str()) {
                return Response::error(403, "jeton invalide");
            }
            api(req, st)
        }
        _ => Response::text(404, "introuvable"),
    }
}

/// An interface file: the embedded copy, or the one in `ui/` when developing the
/// interface (`NEXUL_STUDIO_DEV=1`), so that a page reload shows the changes.
fn ui_file(name: &str, embedded: &'static str) -> Vec<u8> {
    if std::env::var_os("NEXUL_STUDIO_DEV").is_some() {
        if let Ok(bytes) = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("ui").join(name)) {
            return bytes;
        }
    }
    embedded.as_bytes().to_vec()
}

const FAVICON: &[u8] = include_bytes!("../assets/nexul-studio.ico");

// ---------------------------------------------------------------------------
// Workspace paths

/// The open folder.
fn root_of(st: &State) -> Result<PathBuf, Response> {
    st.root.lock().unwrap().clone().ok_or_else(|| Response::error(400, "Aucun dossier n'est ouvert."))
}

/// Resolves a path relative to the open folder, refusing anything outside of it.
fn resolve(st: &State, rel: &str) -> Result<PathBuf, Response> {
    workspace::resolve(&root_of(st)?, rel).map_err(fs_error)
}

fn fs_error(e: workspace::FsError) -> Response {
    Response::error(e.status, &e.message)
}

/// The paths of a request, one per line (names never contain line breaks).
fn path_list(text: &str) -> Vec<String> {
    text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

fn rel_of(st: &State, p: &Path) -> String {
    let root = st.root.lock().unwrap().clone().unwrap_or_default();
    p.strip_prefix(&root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

fn tree_json(dir: &Path, root: &Path, depth: usize, count: &mut usize) -> String {
    let Ok(rd) = std::fs::read_dir(dir) else { return "[]".into() };
    let mut entries: Vec<(bool, String, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if IGNORED.contains(&name.as_str()) {
                return None;
            }
            let is_dir = e.file_type().ok()?.is_dir();
            Some((is_dir, name, e.path()))
        })
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())));
    let mut items = Vec::new();
    for (is_dir, name, p) in entries {
        *count += 1;
        if *count > 20000 {
            break;
        }
        let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
        if is_dir {
            let children = if depth < 12 { tree_json(&p, root, depth + 1, count) } else { "[]".into() };
            items.push(format!("{{\"name\":{},\"path\":{},\"dir\":true,\"children\":{}}}", json_str(&name), json_str(&rel), children));
        } else {
            items.push(format!("{{\"name\":{},\"path\":{},\"dir\":false}}", json_str(&name), json_str(&rel)));
        }
    }
    format!("[{}]", items.join(","))
}

fn state_json(st: &State) -> String {
    let root = st.root.lock().unwrap().clone();
    let nxc = st.nxc.lock().unwrap().clone();
    let tree = match &root {
        Some(r) => {
            let mut count = 0;
            tree_json(r, r, 0, &mut count)
        }
        None => "[]".into(),
    };
    format!(
        "{{\"root\":{},\"rootName\":{},\"nxc\":{},\"port\":{},\"tree\":{},\"sep\":{},\"startFile\":{}}}",
        root.as_ref().map(|r| json_str(&r.to_string_lossy())).unwrap_or_else(|| "null".into()),
        root.as_ref().and_then(|r| r.file_name()).map(|n| json_str(&n.to_string_lossy())).unwrap_or_else(|| "null".into()),
        nxc.as_ref().map(|r| json_str(&r.to_string_lossy())).unwrap_or_else(|| "null".into()),
        st.port,
        tree,
        json_str(std::path::MAIN_SEPARATOR_STR),
        st.start_file.lock().unwrap().take().map(|f| json_str(&f)).unwrap_or_else(|| "null".into())
    )
}

// ---------------------------------------------------------------------------
// API

fn api(req: &Request, st: &Arc<State>) -> Response {
    let route = &req.path[5..];
    let result: Result<Response, Response> = (|| {
        Ok(match (req.method.as_str(), route) {
            ("POST", "ping") => {
                *st.last_ping.lock().unwrap() = Some(Instant::now());
                Response::json("{}".into())
            }
            ("GET", "state") => Response::json(state_json(st)),
            // What changed in the open folder since `since` (a version given by a
            // previous call): `paths` is null when everything must be read again.
            ("GET", "changes") => {
                let since: u64 = req.q("since").parse().unwrap_or(0);
                let version = st.tree_version.load(Ordering::SeqCst);
                let ch = st.changes.lock().unwrap();
                let complete = ch.first().map(|(v, _)| *v <= since + 1).unwrap_or(true);
                let mut paths: Vec<&str> = ch.iter().filter(|(v, _)| *v > since).map(|(_, p)| p.as_str()).collect();
                paths.dedup();
                let list = if !complete || paths.iter().any(|p| p.is_empty()) {
                    "null".to_string()
                } else {
                    format!("[{}]", paths.iter().map(|p| json_str(p)).collect::<Vec<_>>().join(","))
                };
                Response::json(format!("{{\"version\":{},\"paths\":{}}}", version, list))
            }
            // The interface's settings and sessions, kept in a file so that they do not
            // depend on the port (the browser's storage is per origin).
            ("GET", "kv") => {
                let text = std::fs::read_to_string(config_dir().join("state.json")).unwrap_or_default();
                Response::json(if text.trim_start().starts_with('{') { text } else { "{}".into() })
            }
            ("POST", "kv") => {
                let text = req.body_text();
                if !text.trim_start().starts_with('{') {
                    return Err(Response::error(400, "état invalide"));
                }
                let dir = config_dir();
                let tmp = dir.join("state.json.tmp");
                std::fs::write(&tmp, text.as_bytes())
                    .and_then(|_| std::fs::rename(&tmp, dir.join("state.json")))
                    .map_err(|e| Response::error(500, &e.to_string()))?;
                Response::json("{}".into())
            }
            ("POST", "open-folder") => {
                let p = PathBuf::from(json_field(&req.body_text(), "path").unwrap_or_default());
                if !p.is_dir() {
                    return Err(Response::error(400, "ce dossier n'existe pas"));
                }
                let p = std::fs::canonicalize(&p).map(|c| strip_verbatim(&c)).unwrap_or(p);
                set_root(st, Some(p));
                Response::json(state_json(st))
            }
            ("POST", "create-project") => {
                let p = PathBuf::from(json_field(&req.body_text(), "path").unwrap_or_default());
                if !p.is_absolute() || p.file_name().is_none() {
                    return Err(Response::error(400, "emplacement invalide"));
                }
                if p.exists() && std::fs::read_dir(&p).map(|mut d| d.next().is_some()).unwrap_or(true) {
                    return Err(Response::error(400, "ce dossier existe déjà et n'est pas vide"));
                }
                std::fs::create_dir_all(&p).map_err(|e| Response::error(500, &format!("création impossible : {}", e)))?;
                let p = std::fs::canonicalize(&p).map(|c| strip_verbatim(&c)).unwrap_or(p);
                set_root(st, Some(p));
                Response::json(state_json(st))
            }
            ("POST", "close-folder") => {
                set_root(st, None);
                Response::json(state_json(st))
            }
            ("POST", "set-nxc") => {
                let p = PathBuf::from(json_field(&req.body_text(), "path").unwrap_or_default());
                if !p.is_file() {
                    return Err(Response::error(400, "fichier introuvable"));
                }
                *st.nxc.lock().unwrap() = Some(p);
                Response::json(state_json(st))
            }
            ("GET", "browse") => browse(&req.q("path"))?,
            ("GET", "file") => {
                let p = resolve(st, &req.q("path"))?;
                let bytes = std::fs::read(&p).map_err(|e| Response::error(404, &format!("lecture impossible : {}", e)))?;
                if bytes.len() > 8 * 1024 * 1024 {
                    return Err(Response::error(413, "fichier trop volumineux pour l'éditeur (8 Mo maximum)"));
                }
                // The text in UTF-8; its encoding on disk is kept for saving.
                let (text, encoding) = workspace::decode_text(&bytes).ok_or_else(|| Response::error(400, "fichier binaire"))?;
                let mut r = Response::new(200, "text/plain; charset=utf-8", text);
                r.headers.push(("X-Nexul-Encoding".into(), encoding.into()));
                r
            }
            // Saving in the encoding of the file (`encoding`, UTF-8 by default).
            ("POST", "file") => {
                let (bytes, encoding) = workspace::encode_text(&req.body_text(), &req.q("encoding"));
                workspace::save(&root_of(st)?, &req.q("path"), &bytes).map_err(fs_error)?;
                st.live_version.fetch_add(1, Ordering::SeqCst);
                Response::json(format!("{{\"encoding\":{}}}", json_str(encoding)))
            }
            ("POST", "new") => {
                let kind = if req.q("kind") == "dir" { workspace::Kind::Dir } else { workspace::Kind::File };
                workspace::create(&root_of(st)?, &req.q("path"), kind, &req.body).map_err(fs_error)?;
                st.live_version.fetch_add(1, Ordering::SeqCst);
                Response::json(state_json(st))
            }
            ("POST", "rename") => {
                let body = req.body_text();
                let (from, to) = (json_field(&body, "from").unwrap_or_default(), json_field(&body, "to").unwrap_or_default());
                workspace::rename(&root_of(st)?, &from, &to).map_err(fs_error)?;
                st.live_version.fetch_add(1, Ordering::SeqCst);
                Response::json(state_json(st))
            }
            ("POST", "delete") => {
                workspace::trash(&root_of(st)?, &req.q("path")).map_err(fs_error)?;
                st.live_version.fetch_add(1, Ordering::SeqCst);
                Response::json(state_json(st))
            }
            // Drag and drop, cut and paste: `items` (one path per line) into `to`.
            ("POST", "move") => {
                let body = req.body_text();
                let items = path_list(&json_field(&body, "items").unwrap_or_default());
                let moved = workspace::move_into(&root_of(st)?, &items, &json_field(&body, "to").unwrap_or_default()).map_err(fs_error)?;
                st.live_version.fetch_add(1, Ordering::SeqCst);
                let pairs: Vec<String> = moved.iter().map(|(a, b)| format!("[{},{}]", json_str(a), json_str(b))).collect();
                Response::json(format!("{{\"moved\":[{}],\"state\":{}}}", pairs.join(","), state_json(st)))
            }
            ("POST", "copy") => {
                let body = req.body_text();
                let items = path_list(&json_field(&body, "items").unwrap_or_default());
                let created = workspace::copy_into(&root_of(st)?, &items, &json_field(&body, "to").unwrap_or_default()).map_err(fs_error)?;
                st.live_version.fetch_add(1, Ordering::SeqCst);
                let list: Vec<String> = created.iter().map(|c| json_str(c)).collect();
                Response::json(format!("{{\"created\":[{}],\"state\":{}}}", list.join(","), state_json(st)))
            }
            // A file dropped from the Explorer: its content, into the folder `dir`.
            ("POST", "import") => {
                let path = workspace::import(&root_of(st)?, &req.q("dir"), &req.q("name"), &req.body).map_err(fs_error)?;
                st.live_version.fetch_add(1, Ordering::SeqCst);
                Response::json(format!("{{\"path\":{}}}", json_str(&path)))
            }
            ("GET", "search") => search(st, &req.q("q"), req.q("case") == "1")?,
            ("POST", "check") => check(st, &req.q("path"))?,
            ("POST", "run") => start_run(st, &req.q("path"), &req.q("kind"), &req.body_text())?,
            ("GET", "run/poll") => {
                let id: u32 = req.q("id").parse().unwrap_or(0);
                let from: usize = req.q("from").parse().unwrap_or(0);
                let runs = st.runs.lock().unwrap();
                let run = runs.map.get(&id).ok_or_else(|| Response::error(404, "exécution inconnue"))?;
                let out = run.output.lock().unwrap();
                let chunk = if from < out.len() { String::from_utf8_lossy(&out[from..]).to_string() } else { String::new() };
                let done = *run.done.lock().unwrap();
                let times: Vec<String> = run.times.lock().unwrap().iter().map(|t| format!("{:.3}", t)).collect();
                Response::json(format!(
                    "{{\"out\":{},\"next\":{},\"done\":{},\"code\":{},\"times\":[{}]}}",
                    json_str(&chunk),
                    out.len(),
                    done.is_some(),
                    done.unwrap_or(0),
                    times.join(",")
                ))
            }
            ("POST", "run/input") => {
                let id: u32 = req.q("id").parse().unwrap_or(0);
                let ok = st.runs.lock().unwrap().input(id, &req.body_text());
                Response::json(format!("{{\"ok\":{}}}", ok))
            }
            ("POST", "run/kill") => {
                let id: u32 = req.q("id").parse().unwrap_or(0);
                st.runs.lock().unwrap().kill(id);
                Response::json("{}".into())
            }
            // Editor services of the compiler (`nxc ide`), on the text being edited.
            ("POST", "ide") => ide(st, req)?,
            ("GET", "std-source") => {
                let nxc = st.nxc.lock().unwrap().clone().ok_or_else(|| Response::error(400, "compilateur nxc introuvable"))?;
                let module = req.q("module");
                if !module.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') {
                    return Err(Response::error(400, "module invalide"));
                }
                let out = command(&nxc, &["ide".into(), "std-source".into(), module], &std::env::temp_dir()).output().map_err(|e| Response::error(500, &e.to_string()))?;
                Response::new(200, "text/plain; charset=utf-8", out.stdout)
            }
            ("GET", "pkg") => pkg_list(st)?,
            ("POST", "pkg") => pkg_command(st, req)?,
            ("POST", "debug/launch") => debug_launch(st, req)?,
            ("POST", "debug/send") => {
                let mut dbg = st.debugger.lock().unwrap();
                let d = dbg.as_mut().ok_or_else(|| Response::error(400, "aucune session de débogage"))?;
                d.send(&req.body_text()).map_err(|e| Response::error(500, &e.to_string()))?;
                Response::json("{}".into())
            }
            ("GET", "debug/poll") => {
                let from: usize = req.q("from").parse().unwrap_or(0);
                let dbg = st.debugger.lock().unwrap();
                match dbg.as_ref() {
                    Some(d) => {
                        let msgs = d.messages.lock().unwrap();
                        let chunk: Vec<&str> = msgs.iter().skip(from).map(|s| s.as_str()).collect();
                        let done = *d.done.lock().unwrap();
                        Response::json(format!("{{\"next\":{},\"done\":{},\"messages\":[{}]}}", msgs.len(), done, chunk.join(",")))
                    }
                    None => Response::json("{\"next\":0,\"done\":true,\"messages\":[]}".into()),
                }
            }
            ("POST", "debug/stop") => {
                if let Some(mut d) = st.debugger.lock().unwrap().take() {
                    d.stop();
                }
                Response::json("{}".into())
            }
            ("GET", "plugins") => plugins_list()?,
            ("GET", "plugin-file") => plugin_file(&req.q("id"), &req.q("file"))?,
            ("POST", "plugins/open-folder") => {
                let dir = config_dir().join("plugins");
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                Response::json("{}".into())
            }
            ("GET", "explain") => {
                let nxc = st.nxc.lock().unwrap().clone().ok_or_else(|| Response::error(400, "compilateur nxc introuvable"))?;
                let code = req.q("code");
                if !code.chars().all(|c| c.is_ascii_alphanumeric()) {
                    return Err(Response::error(400, "code invalide"));
                }
                let out = command(&nxc, &["explain".into(), code], &std::env::temp_dir()).output().map_err(|e| Response::error(500, &e.to_string()))?;
                Response::text(200, &String::from_utf8_lossy(&out.stdout))
            }
            ("POST", "reveal") => {
                let p = resolve(st, &req.q("path"))?;
                let _ = std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn();
                Response::json("{}".into())
            }
            ("POST", "open-external") => {
                let url = req.body_text();
                if url.starts_with(&format!("http://127.0.0.1:{}/live", st.port)) {
                    let _ = std::process::Command::new("cmd").args(["/C", "start", "", &url]).spawn();
                }
                Response::json("{}".into())
            }
            _ => return Err(Response::error(404, "route inconnue")),
        })
    })();
    match result {
        Ok(r) | Err(r) => r,
    }
}

fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => p.to_path_buf(),
    }
}

fn browse(path: &str) -> Result<Response, Response> {
    let mut dirs = Vec::new();
    let (current, parent) = if path.is_empty() {
        // Drives and common places.
        for letter in b'A'..=b'Z' {
            let d = format!("{}:\\", letter as char);
            if Path::new(&d).is_dir() {
                dirs.push(format!("{{\"name\":{},\"path\":{}}}", json_str(&d), json_str(&d)));
            }
        }
        (String::new(), String::new())
    } else {
        let p = PathBuf::from(path);
        let rd = std::fs::read_dir(&p).map_err(|e| Response::error(400, &format!("dossier illisible : {}", e)))?;
        let mut v: Vec<(String, String)> = rd
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| (e.file_name().to_string_lossy().to_string(), e.path().to_string_lossy().to_string()))
            .filter(|(n, _)| !n.starts_with('$') && !n.starts_with('.'))
            .collect();
        v.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        for (n, p) in v {
            dirs.push(format!("{{\"name\":{},\"path\":{}}}", json_str(&n), json_str(&p)));
        }
        let parent = p.parent().map(|q| q.to_string_lossy().to_string()).unwrap_or_default();
        (p.to_string_lossy().to_string(), parent)
    };
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    Ok(Response::json(format!(
        "{{\"path\":{},\"parent\":{},\"dirs\":[{}],\"home\":{},\"desktop\":{}}}",
        json_str(&current),
        json_str(&parent),
        dirs.join(","),
        json_str(&home),
        json_str(&format!("{}\\Desktop", home))
    )))
}

fn search(st: &State, q: &str, case: bool) -> Result<Response, Response> {
    let root = st.root.lock().unwrap().clone().ok_or_else(|| Response::error(400, "aucun dossier ouvert"))?;
    if q.is_empty() {
        return Ok(Response::json("[]".into()));
    }
    let needle = if case { q.to_string() } else { q.to_lowercase() };
    let mut results = Vec::new();
    let mut stack = vec![root.clone()];
    let mut files = 0;
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if IGNORED.contains(&name.as_str()) {
                continue;
            }
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            files += 1;
            if files > 5000 || results.len() >= 1000 {
                break;
            }
            let Ok(meta) = e.metadata() else { continue };
            if meta.len() > 2 * 1024 * 1024 {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else { continue };
            for (i, line) in text.lines().enumerate() {
                let hay = if case { line.to_string() } else { line.to_lowercase() };
                if let Some(col) = hay.find(&needle) {
                    let col_chars = hay[..col].chars().count();
                    let shown: String = line.chars().take(300).collect();
                    results.push(format!(
                        "{{\"path\":{},\"line\":{},\"col\":{},\"text\":{}}}",
                        json_str(&rel_of(st, &p)),
                        i + 1,
                        col_chars,
                        json_str(&shown)
                    ));
                    if results.len() >= 1000 {
                        break;
                    }
                }
            }
        }
    }
    Ok(Response::json(format!("[{}]", results.join(","))))
}

fn check(st: &State, rel: &str) -> Result<Response, Response> {
    let nxc = st.nxc.lock().unwrap().clone().ok_or_else(|| Response::error(400, "compilateur nxc introuvable"))?;
    let p = resolve(st, rel)?;
    let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let out = command(&nxc, &["check".into(), p.to_string_lossy().to_string(), "--json".into()], &dir)
        .output()
        .map_err(|e| Response::error(500, &format!("impossible de lancer nxc : {}", e)))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !text.starts_with('[') {
        return Err(Response::error(500, &format!("réponse inattendue de nxc : {}", String::from_utf8_lossy(&out.stderr))));
    }
    // File names in diagnostics are relative to the checked file's folder.
    let base = json_str(&rel_of(st, &dir));
    Ok(Response::json(format!("{{\"base\":{},\"diagnostics\":{}}}", base, text)))
}

fn start_run(st: &State, rel: &str, kind: &str, args: &str) -> Result<Response, Response> {
    let nxc = st.nxc.lock().unwrap().clone().ok_or_else(|| Response::error(400, "compilateur nxc introuvable : indiquez son chemin dans les paramètres"))?;
    let p = resolve(st, rel)?;
    let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let file = p.to_string_lossy().to_string();
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut steps = Vec::new();
    let mut cleanup = Vec::new();
    match kind {
        "test" => {
            let mut argv = vec!["test".to_string(), file];
            let filter = args.trim();
            if !filter.is_empty() {
                argv.push(filter.to_string());
            }
            steps.push(Step {
                banner: format!("\u{1b}[36m▶ nxc test {}{}\u{1b}[0m\n", name, if filter.is_empty() { String::new() } else { format!(" {}", filter) }),
                cmd: command(&nxc, &argv, &dir),
                interactive: false,
                report: None,
            })
        }
        "build" => {
            let stem = p.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "programme".into());
            let out = dir.join(format!("{}.exe", stem));
            steps.push(Step {
                banner: format!("\u{1b}[36m▶ nxc build {} --release -o {}.exe\u{1b}[0m\n", name, stem),
                cmd: command(&nxc, &["build".into(), file, "--release".into(), "-o".into(), out.to_string_lossy().to_string()], &dir),
                interactive: false,
                report: Some("✓ Compilé en".into()),
            });
        }
        _ => {
            let n = st.tmp_counter.fetch_add(1, Ordering::SeqCst);
            let exe = std::env::temp_dir().join(format!("nexul-studio-{}-{}.exe", std::process::id(), n));
            steps.push(Step {
                banner: format!("\u{1b}[36m▶ Compilation de {}…\u{1b}[0m\n", name),
                cmd: command(&nxc, &["build".into(), file, "-o".into(), exe.to_string_lossy().to_string()], &dir),
                interactive: false,
                report: Some("✓ Compilé en".into()),
            });
            let argv: Vec<String> = args.split_whitespace().map(|s| s.to_string()).collect();
            steps.push(Step { banner: format!("\u{1b}[36m▶ Exécution\u{1b}[0m\n"), cmd: command(&exe, &argv, &dir), interactive: true, report: None });
            cleanup.push(exe);
        }
    }
    let id = st.runs.lock().unwrap().start(steps, cleanup);
    Ok(Response::json(format!("{{\"id\":{}}}", id)))
}

// ---------------------------------------------------------------------------
// Editor services, packages, debugging, plugins

/// Runs a command with `input` on its standard input; returns its standard output.
fn run_with_input(mut cmd: std::process::Command, input: &[u8]) -> Result<Vec<u8>, Response> {
    use std::io::Write;
    use std::process::Stdio;
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| Response::error(500, &format!("impossible de lancer nxc : {}", e)))?;
    if let Some(mut si) = child.stdin.take() {
        let data = input.to_vec();
        std::thread::spawn(move || {
            let _ = si.write_all(&data);
        });
    }
    let out = child.wait_with_output().map_err(|e| Response::error(500, &e.to_string()))?;
    Ok(out.stdout)
}

const IDE_REQUESTS: &[&str] = &["hover", "definition", "completion", "references", "rename", "symbols", "tests", "format"];

/// `nxc ide <req> <file> <line> <col> [name] --stdin`, the body being the editor's text.
fn ide(st: &State, req: &Request) -> Result<Response, Response> {
    let nxc = st.nxc.lock().unwrap().clone().ok_or_else(|| Response::error(400, "compilateur nxc introuvable"))?;
    let kind = req.q("req");
    if !IDE_REQUESTS.contains(&kind.as_str()) {
        return Err(Response::error(400, "requête inconnue"));
    }
    let p = resolve(st, &req.q("path"))?;
    let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let mut argv = vec!["ide".to_string(), kind.clone(), p.to_string_lossy().to_string()];
    for k in ["line", "col"] {
        let v = req.q(k);
        argv.push(if v.chars().all(|c| c.is_ascii_digit()) && !v.is_empty() { v } else { "1".into() });
    }
    if kind == "rename" {
        let name = req.q("name");
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(Response::error(400, "nom invalide"));
        }
        argv.push(name);
    }
    argv.push("--stdin".into());
    let out = run_with_input(command(&nxc, &argv, &dir), &req.body)?;
    if kind == "format" {
        return Ok(Response::new(200, "text/plain; charset=utf-8", out));
    }
    let text = String::from_utf8_lossy(&out).trim().to_string();
    Ok(Response::json(if text.is_empty() { "null".into() } else { text }))
}

fn nxpm_path(st: &State) -> Option<PathBuf> {
    let nxc = st.nxc.lock().unwrap().clone()?;
    let p = nxc.with_file_name("nxpm.exe");
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// The project (`nexul.toml`) of the open folder and its dependencies.
fn pkg_list(st: &State) -> Result<Response, Response> {
    let root = st.root.lock().unwrap().clone().ok_or_else(|| Response::error(400, "aucun dossier ouvert"))?;
    if !root.join("nexul.toml").is_file() {
        return Ok(Response::json("{\"project\":null}".into()));
    }
    let nxpm = nxpm_path(st).ok_or_else(|| Response::error(400, "nxpm introuvable à côté de nxc"))?;
    let out = command(&nxpm, &["list".into(), "--json".into()], &root).output().map_err(|e| Response::error(500, &e.to_string()))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !text.starts_with('{') {
        return Err(Response::error(500, String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(Response::json(text))
}

/// A package command, run in the terminal: init, add, remove, install, update, verify.
fn pkg_command(st: &State, req: &Request) -> Result<Response, Response> {
    let root = st.root.lock().unwrap().clone().ok_or_else(|| Response::error(400, "aucun dossier ouvert"))?;
    let nxpm = nxpm_path(st).ok_or_else(|| Response::error(400, "nxpm introuvable à côté de nxc"))?;
    let cmd = req.q("cmd");
    let mut argv = vec![cmd.clone()];
    let safe = |s: &str| !s.is_empty() && !s.starts_with('-') && !s.contains('\n');
    match cmd.as_str() {
        "init" | "install" | "update" | "verify" => {}
        "remove" => argv.push(req.q("name")),
        "add" => {
            argv.push(req.q("name"));
            if !req.q("path").is_empty() {
                argv.push("--path".into());
                argv.push(req.q("path"));
            } else {
                argv.push("--git".into());
                argv.push(req.q("git"));
                for k in ["tag", "rev", "branch"] {
                    if !req.q(k).is_empty() {
                        argv.push(format!("--{}", k));
                        argv.push(req.q(k));
                    }
                }
            }
        }
        _ => return Err(Response::error(400, "commande inconnue")),
    }
    if !argv.iter().skip(1).all(|a| a.starts_with("--") || safe(a)) {
        return Err(Response::error(400, "paramètres invalides"));
    }
    let banner = format!("\u{1b}[36m▶ nxpm {}\u{1b}[0m\n", argv.join(" "));
    let id = st.runs.lock().unwrap().start(vec![Step { banner, cmd: command(&nxpm, &argv, &root), interactive: false, report: None }], vec![]);
    Ok(Response::json(format!("{{\"id\":{}}}", id)))
}

/// Builds the program with debugging information and starts `lldb-dap`. The interface
/// then sends `initialize`, `launch` and the breakpoints.
fn debug_launch(st: &State, req: &Request) -> Result<Response, Response> {
    let nxc = st.nxc.lock().unwrap().clone().ok_or_else(|| Response::error(400, "compilateur nxc introuvable"))?;
    let adapter = debug::find_adapter(Some(&nxc)).ok_or_else(|| Response::error(400, "lldb-dap introuvable : installez le composant « Débogueur » de NEXUL"))?;
    let p = resolve(st, &req.q("path"))?;
    let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let stem = p.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "programme".into());
    let out_dir = std::env::temp_dir().join("nexul-studio-debug");
    let _ = std::fs::create_dir_all(&out_dir);
    let exe = out_dir.join(format!("{}.exe", stem));
    let out = command(&nxc, &["build".into(), p.to_string_lossy().to_string(), "-o".into(), exe.to_string_lossy().to_string()], &dir)
        .output()
        .map_err(|e| Response::error(500, &e.to_string()))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).to_string();
        return Ok(Response::json(format!("{{\"error\":{}}}", json_str(&msg))));
    }
    if let Some(mut old) = st.debugger.lock().unwrap().take() {
        old.stop();
    }
    let d = debug::Debugger::start(&adapter).map_err(|e| Response::error(500, &format!("lldb-dap : {}", e)))?;
    *st.debugger.lock().unwrap() = Some(d);
    let root = st.root.lock().unwrap().clone().unwrap_or_default();
    Ok(Response::json(format!(
        "{{\"program\":{},\"cwd\":{},\"root\":{},\"sep\":{}}}",
        json_str(&exe.to_string_lossy()),
        json_str(&dir.to_string_lossy()),
        json_str(&root.to_string_lossy()),
        json_str(std::path::MAIN_SEPARATOR_STR)
    )))
}

/// Plugins: `%APPDATA%\NEXUL Studio\plugins\<id>` and the ones installed with the Studio
/// (`plugins\<id>` next to it). Each holds `plugin.json` and its script.
fn plugin_dirs() -> Vec<(PathBuf, bool)> {
    let mut v = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            v.push((d.join("plugins"), true));
        }
    }
    v.push((Path::new(env!("CARGO_MANIFEST_DIR")).join("plugins"), true));
    v.push((config_dir().join("plugins"), false));
    v
}

fn valid_plugin_id(id: &str) -> bool {
    !id.is_empty() && id.len() < 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') && !id.starts_with('.')
}

fn plugins_list() -> Result<Response, Response> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (dir, builtin) in plugin_dirs() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let id = e.file_name().to_string_lossy().to_string();
            if !valid_plugin_id(&id) || !seen.insert(id.clone()) {
                continue;
            }
            let Ok(manifest) = std::fs::read_to_string(e.path().join("plugin.json")) else { continue };
            if !manifest.trim_start().starts_with('{') {
                continue;
            }
            items.push(format!("{{\"id\":{},\"builtin\":{},\"manifest\":{}}}", json_str(&id), builtin, manifest.trim()));
        }
    }
    Ok(Response::json(format!("[{}]", items.join(","))))
}

fn plugin_file(id: &str, file: &str) -> Result<Response, Response> {
    if !valid_plugin_id(id) || !valid_plugin_id(file) {
        return Err(Response::error(400, "nom invalide"));
    }
    for (dir, _) in plugin_dirs() {
        let p = dir.join(id).join(file);
        if p.is_file() {
            let bytes = std::fs::read(&p).map_err(|e| Response::error(500, &e.to_string()))?;
            return Ok(Response::new(200, mime(&p.to_string_lossy()), bytes));
        }
    }
    Err(Response::error(404, "fichier introuvable"))
}

// ---------------------------------------------------------------------------
// Live server

const RELOAD_SCRIPT: &str = r#"<script>(function(){var v=null;function poll(){fetch('/live-reload',{cache:'no-store'}).then(function(r){return r.text()}).then(function(t){if(v===null){v=t}else if(t!==v){location.reload();return}setTimeout(poll,400)}).catch(function(){setTimeout(poll,1500)})}poll()})();</script>"#;

fn live(req: &Request, st: &State) -> Response {
    let rel = req.path.trim_start_matches("/live").trim_start_matches('/');
    let p = match resolve(st, rel) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let p = if p.is_dir() { p.join("index.html") } else { p };
    match std::fs::read(&p) {
        Ok(bytes) => {
            let name = p.to_string_lossy().to_string();
            let ty = mime(&name);
            if ty.starts_with("text/html") {
                let mut html = String::from_utf8_lossy(&bytes).to_string();
                match html.rfind("</body>") {
                    Some(i) => html.insert_str(i, RELOAD_SCRIPT),
                    None => html.push_str(RELOAD_SCRIPT),
                }
                Response::new(200, ty, html)
            } else {
                Response::new(200, ty, bytes)
            }
        }
        Err(_) => Response::new(
            404,
            "text/html; charset=utf-8",
            format!(
                "<!doctype html><meta charset=utf-8><body style=\"font-family:Segoe UI,sans-serif;background:#15161e;color:#c0caf5;display:grid;place-items:center;height:100vh;margin:0\"><div style=\"text-align:center\"><h2>Page introuvable</h2><p style=\"color:#7f849c\">{}</p></div>{}</body>",
                html_escape(rel),
                RELOAD_SCRIPT
            ),
        ),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

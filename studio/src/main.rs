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
mod runner;

use http::{json_field, json_str, mime, read_request, write_response, Request, Response};
use runner::{command, Runs, Step};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

const INDEX_HTML: &str = include_str!("../ui/index.html");
const STYLE_CSS: &str = include_str!("../ui/style.css");
const LANGS_JS: &str = include_str!("../ui/langs.js");
const EDITOR_JS: &str = include_str!("../ui/editor.js");
const APP_JS: &str = include_str!("../ui/app.js");

/// Directories never shown nor searched.
const IGNORED: &[&str] = &[".git", "target", "node_modules", ".nxc-cache", ".vs", ".idea", "__pycache__"];

struct State {
    token: String,
    port: u16,
    root: Mutex<Option<PathBuf>>,
    nxc: Mutex<Option<PathBuf>>,
    live_version: AtomicU64,
    last_ping: Mutex<Option<Instant>>,
    runs: Mutex<Runs>,
    tmp_counter: AtomicU64,
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

/// Finds the NEXUL compiler.
fn find_nxc() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = std::env::var_os("NEXUL_NXC") {
        candidates.push(PathBuf::from(p));
    }
    if let Some(home) = std::env::var_os("NEXUL_HOME") {
        candidates.push(Path::new(&home).join("target").join("release").join("nxc.exe"));
    }
    candidates.push(PathBuf::from(r"C:\NEXUL\target\release\nxc.exe"));
    candidates.push(PathBuf::from(r"C:\NEXUL\target\debug\nxc.exe"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("nxc.exe"));
        }
    }
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
    });
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
    // External file changes also reload the live preview.
    {
        let st = state.clone();
        std::thread::spawn(move || {
            let mut last = 0u128;
            loop {
                std::thread::sleep(Duration::from_millis(700));
                let root = st.root.lock().unwrap().clone();
                if let Some(root) = root {
                    let stamp = newest_mtime(&root, 0);
                    if last != 0 && stamp != last {
                        st.live_version.fetch_add(1, Ordering::SeqCst);
                    }
                    last = stamp;
                }
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

fn newest_mtime(dir: &Path, depth: usize) -> u128 {
    if depth > 8 {
        return 0;
    }
    let mut best = 0u128;
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if IGNORED.contains(&name.as_str()) {
            continue;
        }
        let Ok(md) = e.metadata() else { continue };
        if md.is_dir() {
            best = best.max(newest_mtime(&e.path(), depth + 1));
        } else if let Ok(m) = md.modified() {
            let t = m.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
            best = best.max(t);
        }
    }
    best
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

fn route(req: &Request, st: &State) -> Response {
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

/// Resolves a path relative to the open folder, refusing anything outside of it.
fn resolve(st: &State, rel: &str) -> Result<PathBuf, Response> {
    let root = st.root.lock().unwrap().clone().ok_or_else(|| Response::error(400, "aucun dossier ouvert"))?;
    let rel = rel.replace('\\', "/");
    let mut out = root.clone();
    for comp in Path::new(rel.trim_start_matches('/')).components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            _ => return Err(Response::error(400, "chemin invalide")),
        }
    }
    Ok(out)
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
        "{{\"root\":{},\"rootName\":{},\"nxc\":{},\"port\":{},\"tree\":{},\"sep\":{}}}",
        root.as_ref().map(|r| json_str(&r.to_string_lossy())).unwrap_or_else(|| "null".into()),
        root.as_ref().and_then(|r| r.file_name()).map(|n| json_str(&n.to_string_lossy())).unwrap_or_else(|| "null".into()),
        nxc.as_ref().map(|r| json_str(&r.to_string_lossy())).unwrap_or_else(|| "null".into()),
        st.port,
        tree,
        json_str(std::path::MAIN_SEPARATOR_STR)
    )
}

// ---------------------------------------------------------------------------
// API

fn api(req: &Request, st: &State) -> Response {
    let route = &req.path[5..];
    let result: Result<Response, Response> = (|| {
        Ok(match (req.method.as_str(), route) {
            ("POST", "ping") => {
                *st.last_ping.lock().unwrap() = Some(Instant::now());
                Response::json("{}".into())
            }
            ("GET", "state") => Response::json(state_json(st)),
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
                *st.root.lock().unwrap() = Some(p);
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
                *st.root.lock().unwrap() = Some(p);
                Response::json(state_json(st))
            }
            ("POST", "close-folder") => {
                *st.root.lock().unwrap() = None;
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
                if bytes.iter().take(8000).any(|&b| b == 0) {
                    return Err(Response::error(400, "fichier binaire"));
                }
                Response::new(200, "text/plain; charset=utf-8", bytes)
            }
            ("POST", "file") => {
                let p = resolve(st, &req.q("path"))?;
                if let Some(parent) = p.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&p, &req.body).map_err(|e| Response::error(500, &format!("écriture impossible : {}", e)))?;
                st.live_version.fetch_add(1, Ordering::SeqCst);
                Response::json("{}".into())
            }
            ("POST", "new") => {
                let p = resolve(st, &req.q("path"))?;
                if p.exists() {
                    return Err(Response::error(400, "ce nom existe déjà"));
                }
                if req.q("kind") == "dir" {
                    std::fs::create_dir_all(&p).map_err(|e| Response::error(500, &e.to_string()))?;
                } else {
                    if let Some(parent) = p.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    std::fs::write(&p, &req.body).map_err(|e| Response::error(500, &e.to_string()))?;
                }
                st.live_version.fetch_add(1, Ordering::SeqCst);
                Response::json(state_json(st))
            }
            ("POST", "rename") => {
                let body = req.body_text();
                let from = resolve(st, &json_field(&body, "from").unwrap_or_default())?;
                let to = resolve(st, &json_field(&body, "to").unwrap_or_default())?;
                if to.exists() {
                    return Err(Response::error(400, "ce nom existe déjà"));
                }
                std::fs::rename(&from, &to).map_err(|e| Response::error(500, &e.to_string()))?;
                Response::json(state_json(st))
            }
            ("POST", "delete") => {
                let p = resolve(st, &req.q("path"))?;
                if p == st.root.lock().unwrap().clone().unwrap_or_default() {
                    return Err(Response::error(400, "impossible de supprimer le dossier ouvert"));
                }
                recycle(&p)?;
                st.live_version.fetch_add(1, Ordering::SeqCst);
                Response::json(state_json(st))
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

/// Sends a file or folder to the recycle bin (Windows), so that it can be restored.
fn recycle(p: &Path) -> Result<(), Response> {
    let method = if p.is_dir() { "DeleteDirectory" } else { "DeleteFile" };
    let script = format!(
        "Add-Type -AssemblyName Microsoft.VisualBasic; [Microsoft.VisualBasic.FileIO.FileSystem]::{}($env:NX_TARGET, 'OnlyErrorDialogs', 'SendToRecycleBin')",
        method
    );
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]).env("NX_TARGET", p);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| Response::error(500, &e.to_string()))?;
    if out.status.success() && !p.exists() {
        Ok(())
    } else {
        Err(Response::error(500, &format!("suppression impossible : {}", String::from_utf8_lossy(&out.stderr).trim())))
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
        "test" => steps.push(Step {
            banner: format!("\u{1b}[36m▶ nxc test {}\u{1b}[0m\n", name),
            cmd: command(&nxc, &["test".into(), file], &dir),
            interactive: false,
            report: None,
        }),
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

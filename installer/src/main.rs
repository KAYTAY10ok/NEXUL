//! The installer of NEXUL: an assistant in the style of the Visual Studio installer
//! (components, location, options, progress), shown in an application window.
//!
//! - `NEXUL Setup.exe`: graphical installation.
//! - `NEXUL Setup.exe --silent [--dir <folder>] [--components a,b] [--no-path] [--no-shortcuts]
//!   [--no-associate] [--old-versions auto|remove|backup|keep]`: installation without window.
//! - `<install>\Désinstaller NEXUL.exe` (the same program without its data): removal.
//!
//! Earlier versions found on the computer are removed or backed up, as the user chooses
//! (see `old.rs`).
//!
//! By default NEXUL is installed for the current user in `%LOCALAPPDATA%\Programs\NEXUL`
//! (no administrator rights needed): `bin\` (nxc, nxpm, NEXUL Studio), `llvm\` (code
//! generation and linking, debugger), `examples\`, `docs\`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[allow(dead_code)]
mod compress;
#[allow(dead_code)]
mod http;
mod old;
#[allow(dead_code)]
mod payload;
mod ui;
mod win;

use http::{json_str, read_request, write_response, Request, Response};
use payload::Manifest;
use win::reg::{self, Value};
use win::shell::{Folder, Shortcut};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const UI: &str = include_str!("../ui/index.html");
pub(crate) const ICON: &[u8] = include_bytes!("../../studio/assets/nexul-studio.ico");
pub(crate) const UNINSTALL_EXE: &str = "Désinstaller NEXUL.exe";
const UNINSTALL_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NEXUL";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Default, Clone)]
pub(crate) struct Progress {
    pub(crate) running: bool,
    pub(crate) done: bool,
    pub(crate) error: Option<String>,
    pub(crate) percent: f64,
    pub(crate) step: String,
    pub(crate) file: String,
    pub(crate) log: Vec<String>,
    /// What could not be done (the installation goes on).
    pub(crate) warnings: Vec<String>,
    /// Earlier versions were backed up.
    pub(crate) backups: bool,
}

pub(crate) struct State {
    pub(crate) token: String,
    pub(crate) port: u16,
    pub(crate) exe: PathBuf,
    pub(crate) manifest: Option<Manifest>,
    pub(crate) stub_len: u64,
    pub(crate) uninstall_mode: bool,
    pub(crate) progress: Mutex<Progress>,
    pub(crate) last_ping: Mutex<Option<Instant>>,
    pub(crate) installed_dir: Mutex<Option<PathBuf>>,
    /// The earlier versions found when the window was opened.
    pub(crate) scan: Mutex<Option<old::Scan>>,
}

fn random_token() -> String {
    let mut h = RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.write_u32(std::process::id());
    format!("{:016x}{:016x}", h.finish(), RandomState::new().build_hasher().finish())
}

pub(crate) fn env_path(var: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

pub(crate) fn default_dir() -> PathBuf {
    env_path("LOCALAPPDATA").join("Programs").join("NEXUL")
}

pub(crate) fn hidden(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Writes a text value (`None`: the default value of the key).
fn reg_str(key: &str, name: Option<&str>, value: &str) -> Result<(), String> {
    reg::set(key, name, &Value::Str(value.to_string()))
}

// ---------------------------------------------------------------------------
// Installation

pub(crate) struct Options {
    pub(crate) dir: PathBuf,
    pub(crate) components: Vec<String>,
    pub(crate) path: bool,
    pub(crate) desktop: bool,
    pub(crate) start_menu: bool,
    pub(crate) associate: bool,
    /// The choices about the earlier versions: (item of the scan, `remove`/`backup`/`keep`).
    pub(crate) old: Vec<(usize, String)>,
}

pub(crate) fn set_progress(st: &State, f: impl FnOnce(&mut Progress)) {
    let mut p = st.progress.lock().unwrap();
    f(&mut p);
}

fn log(st: &State, msg: &str) {
    set_progress(st, |p| {
        p.log.push(msg.to_string());
        p.step = msg.to_string();
    });
}

/// Removes or backs up the earlier versions, as chosen.
fn handle_old_versions(st: &State, o: &Options) {
    let scan = st.scan.lock().unwrap().clone();
    let Some(scan) = scan else { return };
    let mut here = Vec::new();
    let mut elevated = Vec::new();
    for (id, action) in &o.old {
        let Some(item) = scan.items.get(*id) else { continue };
        let ops = scan.ops_for(item, action);
        if ops.iter().any(|op| matches!(op, old::Op::Move(..))) {
            set_progress(st, |p| p.backups = true);
        }
        if item.admin {
            if !ops.is_empty() {
                elevated.push(old::JobChoice::new(item, action));
            }
        } else {
            here.extend(ops);
        }
    }
    if here.is_empty() && elevated.is_empty() {
        return;
    }
    log(st, "Traitement des anciennes versions");
    let mut problems = old::run(&here, scan.elevated, &mut |m| log(st, m));
    if !elevated.is_empty() {
        log(st, "Anciennes versions : demande des droits d'administrateur");
        let (done, p) = old::run_elevated(&st.exe, &elevated);
        for m in done {
            log(st, &m);
        }
        problems.extend(p);
    }
    set_progress(st, |p| p.warnings.extend(problems));
}

pub(crate) fn install(st: &State, o: &Options) -> Result<(), String> {
    let m = st.manifest.as_ref().ok_or("aucune donnée d'installation")?;
    let mut comps: Vec<String> = m.components.iter().filter(|c| c.required || o.components.contains(&c.id)).map(|c| c.id.clone()).collect();
    comps.dedup();
    handle_old_versions(st, o);
    std::fs::create_dir_all(&o.dir).map_err(|e| format!("impossible de créer {} : {}\nChoisissez un autre dossier, ou lancez l'installation en tant qu'administrateur.", o.dir.display(), e))?;
    // A test of writing (Program Files without the rights of an administrator).
    let probe = o.dir.join(".nexul-write-test");
    std::fs::write(&probe, b"ok").map_err(|e| format!("impossible d'écrire dans {} : {}\nChoisissez un autre dossier, ou lancez l'installation en tant qu'administrateur.", o.dir.display(), e))?;
    let _ = std::fs::remove_file(&probe);
    // A previous installation in this folder is replaced: its programs (Studio, nxc...)
    // must not be running. Only those of this folder are stopped.
    win::process::kill_in(&o.dir);
    let files: Vec<&payload::FileEntry> = m.files.iter().filter(|f| comps.contains(&f.component)).collect();
    let total: u64 = files.iter().map(|f| f.size).sum::<u64>().max(1);
    let mut written = 0u64;
    let mut installed: Vec<String> = Vec::new();
    let mut src = std::fs::File::open(&st.exe).map_err(|e| e.to_string())?;
    let mut current_comp = String::new();
    for f in files {
        if f.component != current_comp {
            current_comp = f.component.clone();
            let name = m.components.iter().find(|c| c.id == current_comp).map(|c| c.name.clone()).unwrap_or_default();
            log(st, &format!("Installation : {}", name));
        }
        let dest = o.dir.join(f.path.replace('/', "\\"));
        set_progress(st, |p| p.file = f.path.clone());
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut data = Vec::with_capacity(f.size as usize);
        for k in &f.chunks {
            let c = payload::read_chunk(&mut src, k)?;
            data.extend_from_slice(&compress::decompress(&c, k.len as usize)?);
            written += k.len;
            let pct = written as f64 / total as f64 * 92.0;
            set_progress(st, |p| p.percent = pct);
        }
        if payload::sha256(&data) != f.sha256 {
            return Err(format!("{} : contrôle d'intégrité échoué (programme d'installation endommagé)", f.path));
        }
        std::fs::write(&dest, &data).map_err(|e| format!("{} : {}", dest.display(), e))?;
        installed.push(f.path.clone());
    }
    // The linkers: one LLD program, four names.
    let llvm_bin = o.dir.join("llvm").join("bin");
    if llvm_bin.join("lld.exe").is_file() {
        for name in ["lld-link.exe", "ld.lld.exe", "wasm-ld.exe", "ld64.lld.exe"] {
            let target = llvm_bin.join(name);
            let _ = std::fs::remove_file(&target);
            if std::fs::hard_link(llvm_bin.join("lld.exe"), &target).is_err() {
                std::fs::copy(llvm_bin.join("lld.exe"), &target).map_err(|e| e.to_string())?;
            }
            installed.push(format!("llvm/bin/{}", name));
        }
    }
    set_progress(st, |p| p.percent = 94.0);
    // The uninstaller: this program without its data.
    log(st, "Création du programme de désinstallation");
    let program = std::fs::read(&st.exe).map_err(|e| e.to_string())?;
    let stub = &program[..st.stub_len as usize];
    std::fs::write(o.dir.join(UNINSTALL_EXE), stub).map_err(|e| e.to_string())?;
    std::fs::write(o.dir.join("nexul.ico"), ICON).map_err(|e| e.to_string())?;
    installed.push(UNINSTALL_EXE.into());
    installed.push("nexul.ico".into());
    let bin = o.dir.join("bin");
    let studio = bin.join("NEXUL Studio.exe");
    let has_studio = studio.is_file();
    let mut options = Vec::new();
    // PATH of the user.
    if o.path {
        log(st, "Ajout de NEXUL au PATH");
        let b = bin.display().to_string();
        win::env::change_path("User", std::slice::from_ref(&b), Some(&b))?;
        win::env::set_user_var("NEXUL_HOME", &o.dir.display().to_string())?;
        options.push("path");
    }
    // Shortcuts.
    let (home, icon) = (env_path("USERPROFILE"), o.dir.join("nexul.ico"));
    let shortcut = |lnk: &Path, target: &Path, args: &str, desc: &str| -> Result<(), String> {
        let s = Shortcut { target, arguments: args, working_dir: &home, icon: &icon, icon_index: 0, description: desc };
        win::shell::create_shortcut(lnk, &s)
    };
    let desktop = win::shell::known_folder(Folder::Desktop).unwrap_or_else(|| env_path("USERPROFILE").join("Desktop"));
    let start_dir = win::shell::known_folder(Folder::Programs)
        .unwrap_or_else(|| env_path("APPDATA").join(r"Microsoft\Windows\Start Menu\Programs"))
        .join("NEXUL");
    if has_studio && o.desktop {
        log(st, "Raccourci sur le Bureau");
        shortcut(&desktop.join("NEXUL Studio.lnk"), &studio, "", "NEXUL Studio — l'environnement de développement de NEXUL")?;
        options.push("desktop");
    }
    if o.start_menu {
        log(st, "Raccourcis du menu Démarrer");
        std::fs::create_dir_all(&start_dir).map_err(|e| e.to_string())?;
        if has_studio {
            shortcut(&start_dir.join("NEXUL Studio.lnk"), &studio, "", "NEXUL Studio")?;
        }
        let docs = o.dir.join("docs").join("guide.md");
        if docs.is_file() {
            shortcut(&start_dir.join("Guide de NEXUL.lnk"), &docs, "", "Le guide du langage NEXUL")?;
        }
        shortcut(&start_dir.join("Désinstaller NEXUL.lnk"), &o.dir.join(UNINSTALL_EXE), "", "Désinstaller NEXUL")?;
        options.push("startmenu");
    }
    // `.nx` files open in NEXUL Studio.
    if has_studio && o.associate {
        log(st, "Association des fichiers .nx");
        let cmd = format!("\"{}\" \"%1\"", studio.display());
        reg_str(r"HKCU\Software\Classes\.nx", None, "NEXUL.Source")?;
        reg_str(r"HKCU\Software\Classes\NEXUL.Source", None, "Programme NEXUL")?;
        reg_str(r"HKCU\Software\Classes\NEXUL.Source\DefaultIcon", None, &format!("{},0", o.dir.join("nexul.ico").display()))?;
        reg_str(r"HKCU\Software\Classes\NEXUL.Source\shell\open\command", None, &cmd)?;
        win::shell::notify_associations();
        options.push("associate");
    }
    // "Programs and features" entry.
    log(st, "Enregistrement dans les applications installées");
    let size_kb: u64 = m.files.iter().filter(|f| comps.contains(&f.component)).map(|f| f.size).sum::<u64>() / 1024;
    let un = format!("\"{}\"", o.dir.join(UNINSTALL_EXE).display());
    reg_str(UNINSTALL_KEY, Some("DisplayName"), "NEXUL")?;
    reg_str(UNINSTALL_KEY, Some("DisplayVersion"), &m.version)?;
    reg_str(UNINSTALL_KEY, Some("Publisher"), "NEXUL")?;
    reg_str(UNINSTALL_KEY, Some("InstallLocation"), &o.dir.display().to_string())?;
    reg_str(UNINSTALL_KEY, Some("UninstallString"), &un)?;
    reg_str(UNINSTALL_KEY, Some("DisplayIcon"), &o.dir.join("nexul.ico").display().to_string())?;
    reg::set(UNINSTALL_KEY, Some("EstimatedSize"), &Value::Dword(size_kb.min(u32::MAX as u64) as u32))?;
    reg::set(UNINSTALL_KEY, Some("NoModify"), &Value::Dword(1))?;
    reg::set(UNINSTALL_KEY, Some("NoRepair"), &Value::Dword(1))?;
    // What to remove at uninstallation.
    let mut record = format!("version\t{}\n", m.version);
    for c in &comps {
        record.push_str(&format!("component\t{}\n", c));
    }
    for opt in &options {
        record.push_str(&format!("option\t{}\n", opt));
    }
    for f in &installed {
        record.push_str(&format!("file\t{}\n", f));
    }
    std::fs::write(o.dir.join("install.txt"), record).map_err(|e| e.to_string())?;
    *st.installed_dir.lock().unwrap() = Some(o.dir.clone());
    set_progress(st, |p| {
        p.percent = 100.0;
        p.step = "NEXUL est installé.".into();
        p.file.clear();
    });
    Ok(())
}

/// Removes an installation (`dir` holds `install.txt`).
pub(crate) fn uninstall(st: &State, dir: &Path) -> Result<(), String> {
    win::process::kill_in(dir);
    log(st, "Retrait du PATH, des raccourcis et des associations");
    let scan = old::scan_registrations();
    let problems = old::run(&scan.detach_ops(dir), scan.elevated, &mut |_| {});
    set_progress(st, |p| {
        p.percent = 10.0;
        p.warnings = problems;
    });
    log(st, "Suppression des fichiers");
    old::unrecord(dir, &|f, file| {
        set_progress(st, |p| {
            p.percent = 10.0 + f * 88.0;
            p.file = file.to_string();
        })
    });
    // This program and the folder are removed once it has exited, by a copy of it.
    let me = std::env::current_exe().unwrap_or_default();
    if me.parent().map(|p| p == dir).unwrap_or(false) {
        start_cleanup(&me, dir);
    }
    set_progress(st, |p| {
        p.percent = 100.0;
        p.step = "NEXUL a été désinstallé.".into();
        p.file.clear();
    });
    Ok(())
}

/// The name of the copies of the uninstaller that finish an uninstallation (`%TEMP%`).
const CLEANUP_PREFIX: &str = "nexul-uninstall-";

/// The uninstaller cannot remove its own program while it runs: a copy of it, started from
/// `%TEMP%`, waits for its end, then removes it and the folder if it is empty (the files
/// added by the user stay).
fn start_cleanup(me: &Path, dir: &Path) {
    let pid = std::process::id();
    let helper = std::env::temp_dir().join(format!("{CLEANUP_PREFIX}{pid}.exe"));
    if std::fs::copy(me, &helper).is_ok() {
        let _ = hidden(Command::new(&helper).arg("--finish-uninstall").arg(pid.to_string()).arg(dir))
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

/// `--finish-uninstall <pid> <dir>`: the part done by the copy of the uninstaller.
fn finish_uninstall(pid: u32, dir: &Path) {
    win::process::wait_exit(pid, Duration::from_secs(600));
    let exe = dir.join(UNINSTALL_EXE);
    for _ in 0..40 {
        if std::fs::remove_file(&exe).is_ok() || !exe.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let _ = std::fs::remove_dir(dir);
}

/// The copies left in `%TEMP%` by earlier uninstallations (a running program cannot remove
/// itself): removed when the installer or the uninstaller starts again.
fn remove_old_cleanup_copies() {
    let me = std::env::current_exe().ok();
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with(CLEANUP_PREFIX) && name.ends_with(".exe") && Some(e.path()) != me {
            let _ = std::fs::remove_file(e.path()); // fails if it still runs: next time
        }
    }
}

/// The installation made by this installer (from its entry in the installed applications).
pub(crate) fn existing_install() -> Option<PathBuf> {
    reg::get_text(UNINSTALL_KEY, Some("InstallLocation")).map(PathBuf::from).filter(|p| p.join("install.txt").is_file())
}

// ---------------------------------------------------------------------------
// Interface

fn info_json(st: &State) -> String {
    let existing = existing_install();
    let comps: Vec<String> = st
        .manifest
        .as_ref()
        .map(|m| {
            m.components
                .iter()
                .map(|c| {
                    format!(
                        "{{\"id\":{},\"name\":{},\"description\":{},\"size\":{},\"required\":{},\"default\":{}}}",
                        json_str(&c.id),
                        json_str(&c.name),
                        json_str(&c.description),
                        c.size,
                        c.required,
                        c.default
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let installed_version = existing.as_ref().and_then(|d| std::fs::read_to_string(d.join("install.txt")).ok()).and_then(|t| t.lines().next().and_then(|l| l.strip_prefix("version\t")).map(|s| s.to_string()));
    format!(
        "{{\"mode\":\"{}\",\"version\":{},\"defaultDir\":{},\"existing\":{},\"existingVersion\":{},\"components\":[{}],\"backupRoot\":{}}}",
        if st.uninstall_mode { "uninstall" } else { "install" },
        json_str(st.manifest.as_ref().map(|m| m.version.as_str()).unwrap_or(env!("CARGO_PKG_VERSION"))),
        json_str(&existing.clone().unwrap_or_else(default_dir).display().to_string()),
        existing.as_ref().map(|d| json_str(&d.display().to_string())).unwrap_or_else(|| "null".into()),
        installed_version.map(|v| json_str(&v)).unwrap_or_else(|| "null".into()),
        comps.join(","),
        json_str(&old::backup_root().display().to_string())
    )
}

fn progress_json(st: &State) -> String {
    let p = st.progress.lock().unwrap().clone();
    format!(
        "{{\"running\":{},\"done\":{},\"error\":{},\"percent\":{:.1},\"step\":{},\"file\":{},\"log\":[{}],\"warnings\":[{}],\"backups\":{}}}",
        p.running,
        p.done,
        p.error.as_ref().map(|e| json_str(e)).unwrap_or_else(|| "null".into()),
        p.percent,
        json_str(&p.step),
        json_str(&p.file),
        p.log.iter().map(|l| json_str(l)).collect::<Vec<_>>().join(","),
        p.warnings.iter().map(|l| json_str(l)).collect::<Vec<_>>().join(","),
        p.backups
    )
}

fn api(req: &Request, st: &Arc<State>) -> Response {
    match (req.method.as_str(), &req.path[5..]) {
        ("POST", "ping") => {
            *st.last_ping.lock().unwrap() = Some(Instant::now());
            Response::json("{}".into())
        }
        ("GET", "info") => Response::json(info_json(st)),
        ("GET", "progress") => Response::json(progress_json(st)),
        ("GET", "old-versions") => {
            let scan = old::scan();
            let json = scan.items_json();
            *st.scan.lock().unwrap() = Some(scan);
            Response::json(format!("{{\"items\":{}}}", json))
        }
        ("POST", "install") => {
            if st.progress.lock().unwrap().running {
                return Response::error(400, "déjà en cours");
            }
            let o = Options {
                dir: PathBuf::from(req.q("dir")),
                components: req.q("components").split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
                path: req.q("path") == "1",
                desktop: req.q("desktop") == "1",
                start_menu: req.q("startmenu") == "1",
                associate: req.q("associate") == "1",
                old: req
                    .q("old")
                    .split(',')
                    .filter_map(|c| c.split_once(':'))
                    .filter_map(|(i, a)| Some((i.parse().ok()?, a.to_string())))
                    .filter(|(_, a)| matches!(a.as_str(), "remove" | "backup" | "keep"))
                    .collect(),
            };
            if !o.dir.is_absolute() {
                return Response::error(400, "le dossier d'installation doit être un chemin complet (par exemple C:\\NEXUL)");
            }
            *st.progress.lock().unwrap() = Progress { running: true, step: "Préparation".into(), ..Default::default() };
            let st2 = st.clone();
            std::thread::spawn(move || {
                let r = install(&st2, &o);
                set_progress(&st2, |p| {
                    p.running = false;
                    p.done = true;
                    if let Err(e) = r {
                        p.error = Some(e);
                    }
                });
            });
            Response::json("{}".into())
        }
        ("POST", "uninstall") => {
            // The installation of this uninstaller first, then the registered one.
            let Some(dir) = st.exe.parent().map(|p| p.to_path_buf()).filter(|p| p.join("install.txt").is_file()).or_else(existing_install) else {
                return Response::error(400, "aucune installation de NEXUL trouvée");
            };
            *st.progress.lock().unwrap() = Progress { running: true, step: "Préparation".into(), ..Default::default() };
            let st2 = st.clone();
            std::thread::spawn(move || {
                let r = uninstall(&st2, &dir);
                set_progress(&st2, |p| {
                    p.running = false;
                    p.done = true;
                    if let Err(e) = r {
                        p.error = Some(e);
                    }
                });
            });
            Response::json("{}".into())
        }
        ("POST", "browse") => {
            let start = req.body_text();
            match win::shell::pick_folder("Dossier d'installation de NEXUL", Path::new(&start)) {
                Some(p) => {
                    let p = if p.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case("NEXUL")).unwrap_or(false) { p } else { p.join("NEXUL") };
                    Response::json(format!("{{\"path\":{}}}", json_str(&p.display().to_string())))
                }
                None => Response::json("{\"path\":null}".into()),
            }
        }
        ("POST", "launch") => {
            if let Some(dir) = st.installed_dir.lock().unwrap().clone() {
                let studio = dir.join("bin").join("NEXUL Studio.exe");
                let examples = dir.join("examples");
                let mut cmd = Command::new(&studio);
                if examples.is_dir() {
                    cmd.arg(&examples);
                }
                let _ = cmd.spawn();
            }
            Response::json("{}".into())
        }
        ("POST", "open-backups") => {
            let _ = Command::new("explorer").arg(old::backup_root()).spawn();
            Response::json("{}".into())
        }
        ("POST", "open-folder") => {
            if let Some(dir) = st.installed_dir.lock().unwrap().clone() {
                let _ = Command::new("explorer").arg(dir).spawn();
            }
            Response::json("{}".into())
        }
        ("POST", "quit") => {
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(300));
                std::process::exit(0);
            });
            Response::json("{}".into())
        }
        _ => Response::error(404, "route inconnue"),
    }
}

fn route(req: &Request, st: &Arc<State>) -> Response {
    let host_ok = req.header("host").map(|h| h == format!("127.0.0.1:{}", st.port)).unwrap_or(false);
    if !host_ok {
        return Response::text(403, "forbidden");
    }
    match req.path.as_str() {
        "/" => {
            if req.q("t") != st.token {
                return Response::text(403, "forbidden");
            }
            Response::new(200, "text/html; charset=utf-8", UI.replace("__TOKEN__", &st.token))
        }
        "/icon.ico" => Response::new(200, "image/x-icon", ICON.to_vec()),
        p if p.starts_with("/api/") => {
            if req.header("x-token") != Some(st.token.as_str()) {
                return Response::error(403, "jeton invalide");
            }
            api(req, st)
        }
        _ => Response::text(404, "introuvable"),
    }
}

fn open_window(url: &str) {
    for p in [r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe", r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"] {
        if Path::new(p).is_file() {
            let profile = std::env::temp_dir().join("nexul-setup-window");
            if Command::new(p)
                .arg(format!("--app={}", url))
                .arg(format!("--user-data-dir={}", profile.display()))
                .arg("--window-size=980,680")
                .arg("--no-first-run")
                .arg("--no-default-browser-check")
                .spawn()
                .is_ok()
            {
                return;
            }
        }
    }
    let _ = Command::new("cmd").args(["/C", "start", "", url]).spawn();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // The part started with the rights of an administrator (earlier versions).
    if let Some(i) = args.iter().position(|a| a == "--old-versions-job") {
        let code = args.get(i + 1).map(|j| old::job_main(Path::new(j))).unwrap_or(2);
        std::process::exit(code);
    }
    // The copy of the uninstaller that removes it once it has exited.
    if let Some(i) = args.iter().position(|a| a == "--finish-uninstall") {
        if let (Some(pid), Some(dir)) = (args.get(i + 1).and_then(|p| p.parse().ok()), args.get(i + 2)) {
            finish_uninstall(pid, Path::new(dir));
        }
        std::process::exit(0);
    }
    remove_old_cleanup_copies();
    let exe = std::env::current_exe().unwrap_or_default();
    let (manifest, stub_len) = match payload::read(&exe) {
        Ok((m, s)) => (Some(m), s),
        Err(_) => (None, std::fs::metadata(&exe).map(|m| m.len()).unwrap_or(0)),
    };
    // Without data, this program is the uninstaller.
    let uninstall_mode = manifest.is_none() || args.iter().any(|a| a == "--uninstall");
    let st = Arc::new(State {
        token: random_token(),
        port: 0,
        exe: exe.clone(),
        manifest,
        stub_len,
        uninstall_mode,
        progress: Mutex::new(Progress::default()),
        last_ping: Mutex::new(None),
        installed_dir: Mutex::new(None),
        scan: Mutex::new(None),
    });

    if args.iter().any(|a| a == "--silent") {
        let opt = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned();
        let code = if uninstall_mode {
            let dir = opt("--dir").map(PathBuf::from).or_else(|| exe.parent().map(|p| p.to_path_buf()).filter(|p| p.join("install.txt").is_file())).or_else(existing_install);
            match dir {
                Some(d) => uninstall(&st, &d).map(|_| 0).unwrap_or(1),
                None => 1,
            }
        } else {
            let m = st.manifest.as_ref().unwrap();
            let comps = opt("--components").map(|c| c.split(',').map(|s| s.to_string()).collect()).unwrap_or_else(|| m.components.iter().filter(|c| c.default || c.required).map(|c| c.id.clone()).collect());
            // Earlier versions: `auto` (installations of this installer replaced, other
            // copies backed up), `remove`, `backup` or `keep`.
            let policy = opt("--old-versions").unwrap_or_else(|| "auto".into());
            let mut choices = Vec::new();
            if policy != "keep" {
                let scan = old::scan();
                for (i, item) in scan.items.iter().enumerate() {
                    let a = match policy.as_str() {
                        "backup" if item.kind.is_version() => "backup",
                        "remove" | "backup" => "remove",
                        _ => old::Scan::default_action(item),
                    };
                    choices.push((i, a.to_string()));
                }
                *st.scan.lock().unwrap() = Some(scan);
            }
            let o = Options {
                dir: opt("--dir").map(PathBuf::from).or_else(existing_install).unwrap_or_else(default_dir),
                components: comps,
                path: !args.iter().any(|a| a == "--no-path"),
                desktop: !args.iter().any(|a| a == "--no-shortcuts"),
                start_menu: !args.iter().any(|a| a == "--no-shortcuts"),
                associate: !args.iter().any(|a| a == "--no-associate"),
                old: choices,
            };
            match install(&st, &o) {
                Ok(()) => {
                    let w = st.progress.lock().unwrap().warnings.clone();
                    if !w.is_empty() {
                        let _ = std::fs::write(std::env::temp_dir().join("nexul-setup-error.txt"), w.join("\n"));
                    }
                    0
                }
                Err(e) => {
                    let _ = std::fs::write(std::env::temp_dir().join("nexul-setup-error.txt"), e);
                    1
                }
            }
        };
        std::process::exit(code);
    }

    // The native window (default). `--web-ui`: the HTML page in Edge, kept for one version.
    if !args.iter().any(|a| a == "--web-ui") {
        // `--ui-selftest <folder>`: the window driven by the tests (feature `test-sandbox`).
        let selftest = if cfg!(feature = "test-sandbox") {
            args.iter().position(|a| a == "--ui-selftest").and_then(|i| args.get(i + 1)).map(PathBuf::from)
        } else {
            None
        };
        std::process::exit(ui::run(st, selftest));
    }
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("cannot listen on 127.0.0.1");
    let port = listener.local_addr().unwrap().port();
    let st = Arc::new(State { port, ..Arc::try_unwrap(st).ok().unwrap() });
    let url = format!("http://127.0.0.1:{}/?t={}", port, st.token);
    // The program stops a little after its window is closed.
    {
        let st = st.clone();
        let started = Instant::now();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(2));
            let running = st.progress.lock().unwrap().running;
            let gone = match *st.last_ping.lock().unwrap() {
                Some(t) => t.elapsed() > Duration::from_secs(10),
                None => started.elapsed() > Duration::from_secs(90),
            };
            if gone && !running {
                std::process::exit(0);
            }
        });
    }
    if args.iter().any(|a| a == "--no-window") {
        // For tests: the address of the interface, instead of a window.
        let _ = std::fs::write(std::env::temp_dir().join("nexul-setup-url.txt"), &url);
    } else {
        open_window(&url);
    }
    for stream in listener.incoming().flatten() {
        let st = st.clone();
        std::thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
            if let Some(req) = read_request(&stream) {
                let resp = route(&req, &st);
                write_response(&stream, &resp);
            }
            let _ = (&stream).flush();
        });
    }
}

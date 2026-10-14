//! Earlier versions of NEXUL on this computer.
//!
//! At the start of an installation the computer is examined: installations made by this
//! installer (`install.txt`, entry in the installed applications), other copies of NEXUL
//! or NEXUL Studio (for example the first versions in `C:\Program Files\NEXUL Studio`),
//! and what points at them: PATH entries, shortcuts, the association of `.nx` files,
//! `NEXUL_HOME`. For each version the user chooses to remove it or to back it up. A backup
//! is moved to `%USERPROFILE%\NEXUL - anciennes versions\` and loses its PATH entries,
//! shortcuts, file association and registration: it never interferes with the new version.
//!
//! The operations that need the rights of an administrator (a folder in Program Files, the
//! PATH of the machine) are done by this same program started with elevation
//! (`--old-versions-job <file>`): Windows asks the user for confirmation.

use crate::http::json_str;
use crate::{hidden, powershell, ps_str, UNINSTALL_EXE};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    /// An installation made by this installer (`install.txt`).
    Install,
    /// Another copy of NEXUL or NEXUL Studio.
    Folder,
    /// A PATH entry leading to nxc or NEXUL Studio outside of the versions found.
    PathEntry,
    /// A shortcut to NEXUL leading outside of the versions found (or nowhere).
    Shortcut,
    /// The association of `.nx` files with a program outside of the versions found.
    Assoc,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Install => "install",
            Kind::Folder => "folder",
            Kind::PathEntry => "path",
            Kind::Shortcut => "shortcut",
            Kind::Assoc => "assoc",
        }
    }

    /// A version (removed or backed up), or something that points at nothing worth keeping
    /// (removed or kept).
    pub fn is_version(self) -> bool {
        matches!(self, Kind::Install | Kind::Folder)
    }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub kind: Kind,
    pub name: String,
    pub version: String,
    pub location: PathBuf,
    pub size: u64,
    /// The rights of an administrator are needed.
    pub admin: bool,
    pub details: Vec<String>,
    /// For a PATH entry: `User` or `Machine`.
    pub scope: String,
}

/// An entry of the installed applications.
#[derive(Clone, Debug)]
pub struct Reg {
    pub key: String,
    pub name: String,
    pub version: String,
    pub location: String,
}

#[derive(Clone, Debug, Default)]
pub struct Scan {
    /// The local date and time (names of the backups).
    pub now: String,
    pub elevated: bool,
    pub regs: Vec<Reg>,
    /// `(User|Machine, folder)`.
    pub paths: Vec<(String, String)>,
    /// `(shortcut, target)`.
    pub shortcuts: Vec<(PathBuf, String)>,
    /// The command that opens `.nx` files.
    pub assoc: Option<String>,
    pub home: Option<String>,
    pub items: Vec<Item>,
}

const SCAN: &str = r#"$t=[char]9
'now'+$t+(Get-Date -Format 'yyyy-MM-dd HH''h''mm')
'admin'+$t+([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
foreach($r in 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall','HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall','HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall'){Get-ChildItem -LiteralPath $r -ErrorAction SilentlyContinue|ForEach-Object{$p=Get-ItemProperty -LiteralPath $_.PSPath -ErrorAction SilentlyContinue; if((''+$p.DisplayName) -match 'NEXUL'){'reg'+$t+$_.Name+$t+$p.DisplayName+$t+$p.DisplayVersion+$t+$p.InstallLocation}}}
foreach($s in 'User','Machine'){$v=[Environment]::GetEnvironmentVariable('Path',$s); if($v){foreach($d in $v.Split(';')){try{if($d -and ($d -match 'NEXUL' -or [IO.File]::Exists([IO.Path]::Combine($d,'nxc.exe')) -or [IO.File]::Exists([IO.Path]::Combine($d,'NEXUL Studio.exe')))){'path'+$t+$s+$t+$d}}catch{}}}}
'home'+$t+[Environment]::GetEnvironmentVariable('NEXUL_HOME','User')
$sh=New-Object -ComObject WScript.Shell
foreach($f in 'Desktop','CommonDesktopDirectory','Programs','CommonPrograms'){$d=[Environment]::GetFolderPath($f); if($d -and (Test-Path -LiteralPath $d)){Get-ChildItem -LiteralPath $d -Recurse -Filter '*NEXUL*.lnk' -ErrorAction SilentlyContinue|ForEach-Object{'lnk'+$t+$_.FullName+$t+$sh.CreateShortcut($_.FullName).TargetPath}}}
'assoc'+$t+(Get-ItemProperty -LiteralPath 'HKCU:\Software\Classes\NEXUL.Source\shell\open\command' -ErrorAction SilentlyContinue).'(default)'"#;

/// A path for comparisons: lowercase, `\`, no final `\`.
fn norm(p: &str) -> String {
    p.trim().trim_matches('"').replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

/// `p` is `root` or inside it.
fn under(p: &str, root: &Path) -> bool {
    let (p, r) = (norm(p), norm(&root.to_string_lossy()));
    !r.is_empty() && (p == r || p.starts_with(&format!("{}\\", r)))
}

/// The program of a command line (`"C:\x\y.exe" "%1"`).
fn exe_of(cmd: &str) -> String {
    let c = cmd.trim();
    match c.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or("").to_string(),
        None => c.split(" \"").next().unwrap_or(c).trim().to_string(),
    }
}

/// A build folder of a developer (`...\target\release`): not a version.
fn is_dev(p: &Path) -> bool {
    p.components().any(|c| c.as_os_str().to_string_lossy().eq_ignore_ascii_case("target"))
}

/// The folder of the version that contains `p`, if any.
fn root_of(p: &Path) -> Option<PathBuf> {
    let start = if p.is_dir() { p.to_path_buf() } else { p.parent()?.to_path_buf() };
    if is_dev(&start) || !start.is_dir() {
        return None;
    }
    let ancestors: Vec<&Path> = start.ancestors().take(3).collect();
    ancestors
        .iter()
        .find(|a| a.join("install.txt").is_file() && (a.join("bin").is_dir() || a.join(UNINSTALL_EXE).is_file()))
        .or_else(|| ancestors.iter().find(|a| a.join("NEXUL Studio.exe").is_file() || a.join("bin").join("nxc.exe").is_file()))
        .map(|a| a.to_path_buf())
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    rd.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

/// A file can be created in `dir` by this process.
fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".nexul-write-test-{}", std::process::id()));
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn install_version(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("install.txt")).ok()?;
    text.lines().find_map(|l| l.strip_prefix("version\t")).map(|s| s.trim().to_string())
}

fn cargo_version(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    let line = text.lines().find(|l| l.trim_start().starts_with("version"))?;
    Some(line.split('"').nth(1)?.to_string())
}

/// The folders that are ignored (`NEXUL_SETUP_IGNORE`, separated by `;`): for tests.
fn ignored(p: &Path) -> bool {
    std::env::var("NEXUL_SETUP_IGNORE").map(|v| v.split(';').filter(|s| !s.is_empty()).any(|i| under(&p.to_string_lossy(), Path::new(i)))).unwrap_or(false)
}

/// What is registered on the computer, without looking for versions.
pub fn scan_registrations() -> Scan {
    let mut s = Scan::default();
    for line in powershell(SCAN).unwrap_or_default().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let get = |i: usize| f.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
        match f[0] {
            "now" => s.now = get(1),
            "admin" => s.elevated = get(1) == "True",
            "reg" => s.regs.push(Reg { key: get(1), name: get(2), version: get(3), location: get(4) }),
            "path" => s.paths.push((get(1), get(2))),
            "home" if !get(1).is_empty() => s.home = Some(get(1)),
            "lnk" => s.shortcuts.push((PathBuf::from(get(1)), get(2))),
            "assoc" if !get(1).is_empty() => s.assoc = Some(get(1)),
            _ => {}
        }
    }
    if s.now.is_empty() {
        s.now = "sauvegarde".into();
    }
    s
}

/// The earlier versions of NEXUL and what points at them.
pub fn scan() -> Scan {
    let mut s = scan_registrations();
    let mut folders: Vec<PathBuf> = Vec::new();
    let mut add = |p: Option<PathBuf>| {
        if let Some(p) = p {
            if !ignored(&p) && !folders.iter().any(|q| norm(&q.to_string_lossy()) == norm(&p.to_string_lossy())) {
                folders.push(p);
            }
        }
    };
    for r in &s.regs {
        if !r.location.is_empty() {
            add(root_of(Path::new(&r.location)));
        }
    }
    let mut known = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(v) = std::env::var_os(var) {
            known.push(PathBuf::from(v));
        }
    }
    if let Some(v) = std::env::var_os("LOCALAPPDATA") {
        known.push(PathBuf::from(v).join("Programs"));
    }
    for base in known {
        for name in ["NEXUL", "NEXUL Studio"] {
            let d = base.join(name);
            if d.is_dir() {
                add(root_of(&d).filter(|r| r == &d));
            }
        }
    }
    for (_, d) in &s.paths {
        add(root_of(Path::new(d)));
    }
    for (_, t) in &s.shortcuts {
        add(root_of(Path::new(t)));
    }
    if let Some(c) = &s.assoc {
        add(root_of(Path::new(&exe_of(c))));
    }
    if let Some(h) = &s.home {
        add(root_of(Path::new(h)));
    }

    for f in &folders {
        let install = f.join("install.txt").is_file();
        let regs: Vec<&Reg> = s.regs.iter().filter(|r| !r.location.is_empty() && norm(&r.location) == norm(&f.to_string_lossy())).collect();
        let version = if install { install_version(f) } else { cargo_version(f) }.or_else(|| regs.iter().map(|r| r.version.clone()).find(|v| !v.is_empty())).unwrap_or_default();
        let name = if let Some(r) = regs.iter().find(|r| !r.name.is_empty()) {
            r.name.clone()
        } else if install {
            "NEXUL".to_string()
        } else if f.join("NEXUL Studio.exe").is_file() {
            "NEXUL Studio".to_string()
        } else {
            f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "NEXUL".into())
        };
        let paths: Vec<&(String, String)> = s.paths.iter().filter(|(_, d)| under(d, f)).collect();
        let links: Vec<&(PathBuf, String)> = s.shortcuts.iter().filter(|(_, t)| under(t, f)).collect();
        let assoc = s.assoc.as_ref().map(|c| under(&exe_of(c), f)).unwrap_or(false);
        let home = s.home.as_ref().map(|h| under(h, f)).unwrap_or(false);
        let mut details = Vec::new();
        if !regs.is_empty() {
            details.push("inscrite dans les applications installées".to_string());
        }
        if paths.iter().any(|(sc, _)| sc == "User") {
            details.push("dans le PATH".into());
        }
        if paths.iter().any(|(sc, _)| sc == "Machine") {
            details.push("dans le PATH du système".into());
        }
        match links.len() {
            0 => {}
            1 => details.push("1 raccourci".into()),
            n => details.push(format!("{} raccourcis", n)),
        }
        if assoc {
            details.push("ouvre les fichiers .nx".into());
        }
        if home {
            details.push("NEXUL_HOME".into());
        }
        let admin = !s.elevated
            && (!writable(f)
                || regs.iter().any(|r| r.key.starts_with("HKEY_LOCAL_MACHINE"))
                || paths.iter().any(|(sc, _)| sc == "Machine")
                || links.iter().any(|(l, _)| l.parent().map(|p| !writable(p)).unwrap_or(false)));
        s.items.push(Item {
            kind: if install { Kind::Install } else { Kind::Folder },
            name,
            version,
            location: f.clone(),
            size: dir_size(f),
            admin,
            details,
            scope: String::new(),
        });
    }

    // What points outside of the versions found.
    let inside = |p: &str| folders.iter().any(|f| under(p, f));
    let mut seen_paths: Vec<String> = Vec::new();
    for (scope, d) in &s.paths {
        let key = format!("{}|{}", scope, norm(d));
        if inside(d) || ignored(Path::new(d)) || seen_paths.contains(&key) {
            continue;
        }
        seen_paths.push(key);
        let mut details = Vec::new();
        if !Path::new(d).is_dir() {
            details.push("dossier introuvable".to_string());
        } else if is_dev(Path::new(d)) {
            details.push("version de développement".to_string());
        }
        details.push(if scope == "Machine" { "PATH du système".to_string() } else { "PATH de l'utilisateur".to_string() });
        s.items.push(Item {
            kind: Kind::PathEntry,
            name: "Entrée du PATH".into(),
            version: String::new(),
            location: PathBuf::from(d),
            size: 0,
            admin: scope == "Machine" && !s.elevated,
            details,
            scope: scope.clone(),
        });
    }
    for (lnk, target) in &s.shortcuts {
        if inside(target) || ignored(Path::new(target)) {
            continue;
        }
        let details = vec![if target.is_empty() || !Path::new(target).exists() { "cible introuvable".to_string() } else { format!("ouvre {}", target) }];
        s.items.push(Item {
            kind: Kind::Shortcut,
            name: format!("Raccourci « {} »", lnk.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
            version: String::new(),
            location: lnk.clone(),
            size: 0,
            admin: !s.elevated && lnk.parent().map(|p| !writable(p)).unwrap_or(false),
            details,
            scope: String::new(),
        });
    }
    if let Some(c) = &s.assoc {
        let exe = exe_of(c);
        if !inside(&exe) && !ignored(Path::new(&exe)) {
            s.items.push(Item {
                kind: Kind::Assoc,
                name: "Association des fichiers .nx".into(),
                version: String::new(),
                location: PathBuf::from(&exe),
                size: 0,
                admin: false,
                details: vec![if Path::new(&exe).is_file() { format!("ouvre les fichiers .nx avec {}", exe) } else { "programme introuvable".to_string() }],
                scope: String::new(),
            });
        }
    }
    s
}

/// The folder of the backups.
pub fn backup_root() -> PathBuf {
    std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("NEXUL - anciennes versions")
}

/// An operation on an earlier version (done here or by the program started with elevation).
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Stops NEXUL Studio.
    Kill,
    /// Removes the files of an installation (listed in its `install.txt`).
    Unrecord(PathBuf),
    RemoveDir(PathBuf),
    /// Moves a version to its backup folder: `(from, to, title)`.
    Move(PathBuf, PathBuf, String),
    DeleteFile(PathBuf),
    RegDelete(String),
    /// Removes from the PATH (`User` or `Machine`) a folder and the folders inside it.
    PathRemove(String, String),
    HomeRemove,
    AssocRemove,
}

impl Op {
    pub fn to_line(&self) -> String {
        match self {
            Op::Kill => "kill".into(),
            Op::Unrecord(p) => format!("unrecord\t{}", p.display()),
            Op::RemoveDir(p) => format!("rmdir\t{}", p.display()),
            Op::Move(a, b, t) => format!("move\t{}\t{}\t{}", a.display(), b.display(), t),
            Op::DeleteFile(p) => format!("del\t{}", p.display()),
            Op::RegDelete(k) => format!("reg\t{}", k),
            Op::PathRemove(s, d) => format!("path\t{}\t{}", s, d),
            Op::HomeRemove => "home".into(),
            Op::AssocRemove => "assoc".into(),
        }
    }

    pub fn from_line(line: &str) -> Option<Op> {
        let f: Vec<&str> = line.split('\t').collect();
        let p = |i: usize| f.get(i).map(PathBuf::from);
        Some(match f[0] {
            "kill" => Op::Kill,
            "unrecord" => Op::Unrecord(p(1)?),
            "rmdir" => Op::RemoveDir(p(1)?),
            "move" => Op::Move(p(1)?, p(2)?, f.get(3)?.to_string()),
            "del" => Op::DeleteFile(p(1)?),
            "reg" => Op::RegDelete(f.get(1)?.to_string()),
            "path" => Op::PathRemove(f.get(1)?.to_string(), f.get(2)?.to_string()),
            "home" => Op::HomeRemove,
            "assoc" => Op::AssocRemove,
            _ => return None,
        })
    }
}

fn safe_name(s: &str) -> String {
    s.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '-' } else { c }).collect::<String>().trim().trim_end_matches('.').to_string()
}

impl Scan {
    /// Detaches a folder from the system: PATH entries, shortcuts, file association,
    /// `NEXUL_HOME` and entries of the installed applications that lead into it.
    pub fn detach_ops(&self, dir: &Path) -> Vec<Op> {
        let mut ops = Vec::new();
        for scope in ["User", "Machine"] {
            if self.paths.iter().any(|(s, d)| s == scope && under(d, dir)) {
                ops.push(Op::PathRemove(scope.into(), dir.display().to_string()));
            }
        }
        for (lnk, t) in &self.shortcuts {
            if under(t, dir) {
                ops.push(Op::DeleteFile(lnk.clone()));
            }
        }
        if self.assoc.as_ref().map(|c| under(&exe_of(c), dir)).unwrap_or(false) {
            ops.push(Op::AssocRemove);
        }
        if self.home.as_ref().map(|h| under(h, dir)).unwrap_or(false) {
            ops.push(Op::HomeRemove);
        }
        for r in &self.regs {
            if !r.location.is_empty() && norm(&r.location) == norm(&dir.to_string_lossy()) {
                ops.push(Op::RegDelete(r.key.clone()));
            }
        }
        ops
    }

    /// The operations for the choice of the user about an item (`remove`, `backup`, `keep`).
    pub fn ops_for(&self, item: &Item, action: &str) -> Vec<Op> {
        match (item.kind, action) {
            (Kind::Install | Kind::Folder, "remove" | "backup") => {
                let mut ops = self.detach_ops(&item.location);
                if action == "backup" {
                    let title = if item.version.is_empty() { item.name.clone() } else { format!("{} {}", item.name, item.version) };
                    let base = backup_root().join(safe_name(&format!("{} - {}", title, self.now)));
                    let mut dest = base.clone();
                    let mut n = 2;
                    while dest.exists() {
                        dest = PathBuf::from(format!("{} ({})", base.display(), n));
                        n += 1;
                    }
                    ops.push(Op::Move(item.location.clone(), dest, title));
                } else if item.kind == Kind::Install {
                    ops.push(Op::Unrecord(item.location.clone()));
                } else {
                    ops.push(Op::RemoveDir(item.location.clone()));
                }
                ops
            }
            (Kind::PathEntry, "remove") => vec![Op::PathRemove(item.scope.clone(), item.location.display().to_string())],
            (Kind::Shortcut, "remove") => vec![Op::DeleteFile(item.location.clone())],
            (Kind::Assoc, "remove") => vec![Op::AssocRemove],
            _ => Vec::new(),
        }
    }

    /// The choice made without the user (`--silent`): installations of this installer are
    /// replaced, other copies are backed up, what points at nothing useful is removed.
    pub fn default_action(item: &Item) -> &'static str {
        match item.kind {
            Kind::Install => "remove",
            Kind::Folder => "backup",
            _ => "remove",
        }
    }

    pub fn items_json(&self) -> String {
        let items: Vec<String> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                format!(
                    "{{\"id\":{},\"kind\":\"{}\",\"name\":{},\"version\":{},\"location\":{},\"size\":{},\"admin\":{},\"details\":[{}],\"choices\":{}}}",
                    i,
                    it.kind.name(),
                    json_str(&it.name),
                    json_str(&it.version),
                    json_str(&it.location.display().to_string()),
                    it.size,
                    it.admin,
                    it.details.iter().map(|d| json_str(d)).collect::<Vec<_>>().join(","),
                    if it.kind.is_version() { "[\"remove\",\"backup\"]" } else { "[\"remove\",\"keep\"]" }
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }
}

/// The PowerShell script that removes from the PATH of `scope` the folders `remove` (and
/// the folders inside them), then adds `add`. The variable keeps its raw form
/// (`%USERPROFILE%\...` stays as it is) and the programs are told of the change.
pub fn path_script(scope: &str, remove: &[String], add: Option<&str>) -> String {
    let key = if scope == "Machine" { r"HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment" } else { r"HKCU:\Environment" };
    let rm: Vec<String> = remove.iter().map(|r| ps_str(&norm(r))).collect();
    format!(
        "$k=Get-Item -LiteralPath {key}; $raw=[string]$k.GetValue('Path','',[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames); $rm=@({rm}); $parts=@($raw.Split(';')|Where-Object{{ $e=[Environment]::ExpandEnvironmentVariables($_).Trim().TrimEnd('\\').ToLower(); $_.Trim() -ne '' -and -not ($rm|Where-Object{{ $e -eq $_ -or $e.StartsWith($_+'\\') }}) }}); {add}Set-ItemProperty -LiteralPath {key} -Name Path -Value ($parts -join ';') -Type ExpandString; [Environment]::SetEnvironmentVariable('NEXUL_SETUP_REFRESH',$null,'User')",
        key = ps_str(key),
        rm = rm.join(","),
        add = add.map(|a| format!("$parts+={}; ", ps_str(a))).unwrap_or_default()
    )
}

fn reg_delete(key: &str) -> Result<(), String> {
    let out = hidden(Command::new("reg").args(["delete", key, "/f"])).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("impossible de supprimer la clé {}", key))
    }
}

pub fn notify_associations() {
    let _ = powershell("$sig='[DllImport(\"shell32.dll\")] public static extern void SHChangeNotify(int e,int f,System.IntPtr a,System.IntPtr b);'; $t=Add-Type -MemberDefinition $sig -Name N -Namespace W -PassThru; $t::SHChangeNotify(0x08000000,0,[IntPtr]::Zero,[IntPtr]::Zero)");
}

/// Removes the files of the installation in `dir` (listed in its `install.txt`), then the
/// folders left empty. The files added by the user are kept. `progress(fraction, file)`.
pub fn unrecord(dir: &Path, progress: &dyn Fn(f64, &str)) {
    let record = std::fs::read_to_string(dir.join("install.txt")).unwrap_or_default();
    let mut files: Vec<String> = record.lines().filter_map(|l| l.strip_prefix("file\t")).map(|s| s.to_string()).collect();
    files.push("install.txt".into());
    files.push("nexul.ico".into());
    let me = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok());
    let n = files.len().max(1);
    for (i, f) in files.iter().enumerate() {
        let p = dir.join(f.replace('/', "\\"));
        if me.is_some() && p.canonicalize().ok() == me {
            continue; // the uninstaller itself: removed once it has exited
        }
        let _ = std::fs::remove_file(&p);
        progress(i as f64 / n as f64, f);
    }
    // Empty folders, deepest first, up to `dir` itself.
    let mut dirs: Vec<PathBuf> = files.iter().filter_map(|f| Path::new(&f.replace('/', "\\")).parent().map(|p| dir.join(p))).collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    dirs.dedup();
    for d in dirs {
        let mut cur = d;
        while cur.starts_with(dir) && cur != dir {
            if std::fs::remove_dir(&cur).is_err() {
                break;
            }
            if !cur.pop() {
                break;
            }
        }
    }
    let _ = std::fs::remove_dir(dir);
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let t = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &t)?;
        } else {
            std::fs::copy(e.path(), &t)?;
        }
    }
    Ok(())
}

fn move_dir(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{} : {}", parent.display(), e))?;
    }
    // The same disk: a rename. Otherwise a copy, then the removal of the original.
    let mut last = None;
    for _ in 0..5 {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if e.raw_os_error() == Some(17) => {
                // ERROR_NOT_SAME_DEVICE
                copy_dir(from, to).map_err(|e| format!("copie vers {} : {}", to.display(), e))?;
                return std::fs::remove_dir_all(from).map_err(|e| format!("{} : {}", from.display(), e));
            }
            Err(e) => last = Some(e),
        }
        // A program of the version may still be closing.
        std::thread::sleep(std::time::Duration::from_millis(600));
    }
    Err(format!("{} : {}", from.display(), last.map(|e| e.to_string()).unwrap_or_default()))
}

fn readme(title: &str, from: &Path) -> String {
    format!(
        "Sauvegarde de {title}\r\n\r\nCe dossier contient une ancienne version de NEXUL, déplacée ici par le programme d'installation\r\nd'une version plus récente. Elle se trouvait dans :\r\n    {from}\r\n\r\nElle a été détachée du système : retirée du PATH, sans raccourci, sans association des fichiers\r\n.nx et sans inscription dans les applications installées. Elle ne gêne donc pas la version\r\nactuelle ; ses programmes restent utilisables en les lançant depuis ce dossier.\r\n\r\nPour la supprimer, supprimez simplement ce dossier.\r\n",
        title = title,
        from = from.display()
    )
}

/// Does the operations; `report` receives what is being done. Returns the problems.
pub fn run(ops: &[Op], elevated: bool, report: &mut dyn FnMut(&str)) -> Vec<String> {
    let mut problems = Vec::new();
    for op in ops {
        let r: Result<(), String> = match op {
            Op::Kill => {
                let _ = hidden(Command::new("taskkill").args(["/F", "/IM", "NEXUL Studio.exe"])).output();
                Ok(())
            }
            Op::Unrecord(d) => {
                report(&format!("Suppression de {}", d.display()));
                unrecord(d, &|_, _| {});
                Ok(())
            }
            Op::RemoveDir(d) => {
                report(&format!("Suppression de {}", d.display()));
                std::fs::remove_dir_all(d).map_err(|e| format!("{} : {}", d.display(), e))
            }
            Op::Move(from, to, title) => {
                report(&format!("Sauvegarde de {} dans {}", title, to.display()));
                move_dir(from, to).map(|()| {
                    // The uninstaller of a backup would touch the new version.
                    let _ = std::fs::remove_file(to.join(UNINSTALL_EXE));
                    let _ = std::fs::write(to.join("LISEZ-MOI - sauvegarde.txt"), readme(title, from));
                    if elevated {
                        // The rights of the user on the folder, as on the rest of the profile.
                        let _ = hidden(Command::new("icacls").arg(to).args(["/reset", "/T", "/C", "/Q"])).output();
                    }
                })
            }
            Op::DeleteFile(f) => {
                report(&format!("Suppression du raccourci {}", f.display()));
                let r = std::fs::remove_file(f).map_err(|e| format!("{} : {}", f.display(), e));
                // The folder of the Start menu, once empty.
                if let Some(parent) = f.parent() {
                    if parent.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case("NEXUL")).unwrap_or(false) {
                        let _ = std::fs::remove_dir(parent);
                    }
                }
                r
            }
            Op::RegDelete(k) => {
                report("Retrait de l'inscription dans les applications installées");
                reg_delete(k)
            }
            Op::PathRemove(scope, d) => {
                report(&format!("Retrait de {} du PATH", d));
                powershell(&path_script(scope, &[d.clone()], None)).map(|_| ())
            }
            Op::HomeRemove => powershell("[Environment]::SetEnvironmentVariable('NEXUL_HOME',$null,'User')").map(|_| ()),
            Op::AssocRemove => {
                report("Retrait de l'association des fichiers .nx");
                let _ = reg_delete(r"HKCU\Software\Classes\.nx");
                let r = reg_delete(r"HKCU\Software\Classes\NEXUL.Source");
                notify_associations();
                r
            }
        };
        if let Err(e) = r {
            problems.push(e);
        }
    }
    problems
}

/// Does the operations in this program started with the rights of an administrator.
/// Returns `(done, problems)`.
pub fn run_elevated(exe: &Path, ops: &[Op]) -> (Vec<String>, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("nexul-setup-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let job = dir.join("job.txt");
    let log = dir.join("job.log");
    let _ = std::fs::remove_file(&log);
    let text: String = ops.iter().map(|o| o.to_line() + "\n").collect();
    if let Err(e) = std::fs::write(&job, text) {
        return (vec![], vec![e.to_string()]);
    }
    let script = format!(
        "$p=Start-Process -FilePath {} -ArgumentList {} -Verb RunAs -Wait -PassThru -WindowStyle Hidden; exit $p.ExitCode",
        ps_str(&exe.display().to_string()),
        ps_str(&format!("--old-versions-job \"{}\"", job.display()))
    );
    let started = powershell(&script);
    let mut done = Vec::new();
    let mut problems = Vec::new();
    match std::fs::read_to_string(&log) {
        Ok(text) => {
            for line in text.lines() {
                match line.split_once('\t') {
                    Some(("log", m)) => done.push(m.to_string()),
                    Some(("problem", m)) => problems.push(m.to_string()),
                    _ => {}
                }
            }
        }
        Err(_) => problems.push(match started {
            Err(_) => "Les droits d'administrateur n'ont pas été accordés : les anciennes versions qui les demandent n'ont pas été traitées.".to_string(),
            Ok(_) => "Les anciennes versions qui demandent les droits d'administrateur n'ont pas pu être traitées.".to_string(),
        }),
    }
    let _ = std::fs::remove_dir_all(&dir);
    (done, problems)
}

/// `--old-versions-job <file>`: the part of the program started with elevation.
pub fn job_main(job: &Path) -> i32 {
    let text = std::fs::read_to_string(job).unwrap_or_default();
    let ops: Vec<Op> = text.lines().filter_map(Op::from_line).collect();
    let mut out = String::new();
    let problems = run(&ops, true, &mut |m| {
        out.push_str("log\t");
        out.push_str(m);
        out.push('\n');
    });
    for p in &problems {
        out.push_str(&format!("problem\t{}\n", p.replace('\n', " ")));
    }
    let _ = std::fs::write(job.with_file_name("job.log"), out);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        assert!(under(r"C:\Program Files\NEXUL Studio\NEXUL Studio.exe", Path::new(r"c:\program files\nexul studio\")));
        assert!(!under(r"C:\Program Files\NEXUL Studio 2", Path::new(r"C:\Program Files\NEXUL Studio")));
        assert_eq!(exe_of(r#""C:\a b\NEXUL Studio.exe" "%1""#), r"C:\a b\NEXUL Studio.exe");
        assert_eq!(exe_of(r#"C:\x\s.exe "%1""#), r"C:\x\s.exe");
        assert!(is_dev(Path::new(r"C:\NEXUL\target\release")));
    }

    #[test]
    fn ops_round_trip() {
        let ops = vec![
            Op::Kill,
            Op::Unrecord(r"C:\a\NEXUL".into()),
            Op::Move(r"C:\Program Files\NEXUL Studio".into(), r"C:\Users\é\NEXUL - anciennes versions\NEXUL Studio 0.1.0 - 2026-10-14 15h02".into(), "NEXUL Studio 0.1.0".into()),
            Op::PathRemove("Machine".into(), r"C:\x\bin".into()),
            Op::RegDelete(r"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Uninstall\NEXUL".into()),
            Op::AssocRemove,
        ];
        for op in ops {
            assert_eq!(Op::from_line(&op.to_line()), Some(op));
        }
    }
}

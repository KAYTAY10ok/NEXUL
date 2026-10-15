//! File operations on the workspace (the open folder): creating, renaming, moving,
//! copying, saving and deleting to the recycle bin.
//!
//! Paths given by the interface are relative to the workspace root, with `/` or `\`.
//! Every failure is a sentence for the user, in French, with the HTTP status that the
//! server returns. These functions do not depend on the interface: the web interface uses
//! them through the server, and a native interface can call them directly.

use std::io;
use std::path::{Component, Path, PathBuf};

/// A failure: HTTP status and message for the user.
#[derive(Debug, Clone, PartialEq)]
pub struct FsError {
    pub status: u16,
    pub message: String,
}

impl FsError {
    fn new(status: u16, message: impl Into<String>) -> FsError {
        FsError { status, message: message.into() }
    }
}

pub type FsResult<T> = Result<T, FsError>;

/// What to create.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    File,
    Dir,
}

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "COM¹", "COM²", "COM³", "LPT1", "LPT2", "LPT3", "LPT4",
    "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³",
];

/// Checks one name of a file or folder to create (portable to every Windows version).
pub fn validate_name(name: &str) -> FsResult<()> {
    if name.is_empty() || name.trim().is_empty() {
        return Err(FsError::new(400, "Le nom est vide."));
    }
    if name == "." || name == ".." {
        return Err(FsError::new(400, format!("« {} » n'est pas un nom valide.", name)));
    }
    if let Some(c) = name.chars().find(|c| "<>:\"/\\|?*".contains(*c) || c.is_control()) {
        let shown = if c.is_control() { "caractère de contrôle".to_string() } else { c.to_string() };
        return Err(FsError::new(400, format!("Nom invalide : le caractère « {} » est interdit (\\ / : * ? \" < > | ne peuvent pas apparaître dans un nom).", shown)));
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(FsError::new(400, "Nom invalide : un nom ne peut pas se terminer par un point ou une espace (Windows les supprimerait)."));
    }
    let stem = name.split('.').next().unwrap_or("").trim_end().to_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        return Err(FsError::new(400, format!("« {} » est un nom réservé par Windows (CON, PRN, AUX, NUL, COM1…, LPT1…) : choisissez-en un autre.", name)));
    }
    if name.encode_utf16().count() > 255 {
        return Err(FsError::new(400, "Nom trop long (255 caractères au plus)."));
    }
    Ok(())
}

/// The relative path, split into its names; refuses anything that leaves the workspace.
fn parts(rel: &str) -> FsResult<Vec<String>> {
    let rel = rel.replace('\\', "/");
    let mut out = Vec::new();
    for comp in Path::new(rel.trim_start_matches('/')).components() {
        match comp {
            Component::Normal(c) => out.push(c.to_string_lossy().to_string()),
            Component::CurDir => {}
            _ => return Err(FsError::new(400, "Chemin invalide : il doit rester dans le dossier ouvert.")),
        }
    }
    Ok(out)
}

/// An existing (or not) entry of the workspace.
pub fn resolve(root: &Path, rel: &str) -> FsResult<PathBuf> {
    let mut out = root.to_path_buf();
    for p in parts(rel)? {
        out.push(p);
    }
    Ok(out)
}

/// A path where something will be created: the names that do not exist yet are checked.
pub fn resolve_new(root: &Path, rel: &str) -> FsResult<PathBuf> {
    let names = parts(rel)?;
    if names.is_empty() {
        return Err(FsError::new(400, "Le nom est vide."));
    }
    let mut out = root.to_path_buf();
    for (i, p) in names.iter().enumerate() {
        out.push(p);
        if i + 1 == names.len() || !out.exists() {
            validate_name(p)?;
        }
    }
    Ok(out)
}

/// The path relative to the root, with `/`.
pub fn rel_of(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// Comparison of paths as the file system does it (case-insensitive on Windows).
fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

/// `inner` is `outer` or inside it.
fn is_within(inner: &Path, outer: &Path) -> bool {
    let (i, o): (Vec<_>, Vec<_>) = (inner.components().collect(), outer.components().collect());
    i.len() >= o.len() && i.iter().zip(&o).all(|(a, b)| same_path(Path::new(a.as_os_str()), Path::new(b.as_os_str())))
}

/// A message for an error of the system about `what` (a path shown to the user).
pub fn io_message(e: &io::Error, what: &str) -> FsError {
    let code = e.raw_os_error();
    let (status, msg) = match (e.kind(), code) {
        (_, Some(32)) | (_, Some(33)) => (409, format!("« {} » est utilisé par un autre programme : fermez-le puis réessayez.", what)),
        (io::ErrorKind::NotFound, _) | (_, Some(2)) | (_, Some(3)) => (404, format!("« {} » est introuvable (déplacé ou supprimé en dehors de Studio ?).", what)),
        (io::ErrorKind::PermissionDenied, _) | (_, Some(5)) => (403, format!("Accès refusé à « {} » : vous n'avez pas les droits nécessaires, ou un autre programme l'utilise.", what)),
        (io::ErrorKind::AlreadyExists, _) | (_, Some(80)) | (_, Some(183)) => (409, format!("« {} » existe déjà.", what)),
        (_, Some(145)) => (409, format!("Le dossier « {} » n'est pas vide.", what)),
        (_, Some(123)) | (_, Some(161)) => (400, format!("« {} » n'est pas un nom valide.", what)),
        (_, Some(206)) => (400, format!("Le chemin de « {} » est trop long.", what)),
        (_, Some(112)) | (_, Some(39)) => (507, "Le disque est plein.".to_string()),
        (_, Some(19)) => (403, format!("Le disque de « {} » est protégé en écriture.", what)),
        (_, Some(17)) => (400, "Impossible de déplacer vers un autre disque ici.".to_string()),
        _ => (500, format!("Opération impossible sur « {} » : {}", what, e)),
    };
    FsError::new(status, msg)
}

/// Creates a file (with its content) or a folder; the missing parent folders are created.
pub fn create(root: &Path, rel: &str, kind: Kind, content: &[u8]) -> FsResult<String> {
    let p = resolve_new(root, rel)?;
    let shown = rel_of(root, &p);
    if p.exists() {
        return Err(FsError::new(409, format!("« {} » existe déjà.", shown)));
    }
    if let Some(parent) = p.parent() {
        if parent.is_file() {
            return Err(FsError::new(409, format!("« {} » est un fichier, pas un dossier.", rel_of(root, parent))));
        }
        std::fs::create_dir_all(parent).map_err(|e| io_message(&e, &rel_of(root, parent)))?;
    }
    match kind {
        Kind::Dir => std::fs::create_dir(&p).map_err(|e| io_message(&e, &shown))?,
        Kind::File => {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&p).map_err(|e| io_message(&e, &shown))?;
            f.write_all(content).map_err(|e| io_message(&e, &shown))?;
        }
    }
    Ok(shown)
}

/// Saves the content of a file (created if needed, with its folders). The content is
/// written to a temporary file of the same folder, flushed to the disk, then put in
/// place of the file in one step: a crash or a full disk never leaves half a file.
pub fn save(root: &Path, rel: &str, content: &[u8]) -> FsResult<()> {
    let p = if resolve(root, rel)?.exists() { resolve(root, rel)? } else { resolve_new(root, rel)? };
    let shown = rel_of(root, &p);
    if p.is_dir() {
        return Err(FsError::new(409, format!("« {} » est un dossier.", shown)));
    }
    if std::fs::metadata(&p).map(|m| m.permissions().readonly()).unwrap_or(false) {
        return Err(FsError::new(403, format!("« {} » est en lecture seule : retirez cet attribut (Propriétés du fichier) ou enregistrez sous un autre nom.", shown)));
    }
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_message(&e, &rel_of(root, parent)))?;
    }
    write_atomic(&p, content).map_err(|e| io_message(&e, &shown))
}

/// `nom.ext` → `.nom.ext.<pid>.nxtmp`, beside it.
fn temp_beside(p: &Path) -> PathBuf {
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    p.with_file_name(format!(".{}.{}.nxtmp", name, std::process::id()))
}

fn write_atomic(p: &Path, content: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let tmp = temp_beside(p);
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(content)?;
        f.sync_all()?;
        drop(f);
        replace_file(&tmp, p)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Puts `tmp` in place of `target`, keeping the attributes and the permissions of an
/// existing target (`ReplaceFileW`).
#[cfg(windows)]
fn replace_file(tmp: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn ReplaceFileW(replaced: *const u16, replacement: *const u16, backup: *const u16, flags: u32, exclude: *mut core::ffi::c_void, reserved: *mut core::ffi::c_void) -> i32;
    }
    const REPLACEFILE_IGNORE_MERGE_ERRORS: u32 = 0x2;
    if !target.exists() {
        return std::fs::rename(tmp, target);
    }
    let wide = |p: &Path| p.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<u16>>();
    let (t, r) = (wide(target), wide(tmp));
    let ok = unsafe { ReplaceFileW(t.as_ptr(), r.as_ptr(), std::ptr::null(), REPLACEFILE_IGNORE_MERGE_ERRORS, std::ptr::null_mut(), std::ptr::null_mut()) };
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(windows))]
fn replace_file(tmp: &Path, target: &Path) -> io::Result<()> {
    if let Ok(meta) = std::fs::metadata(target) {
        let _ = std::fs::set_permissions(tmp, meta.permissions());
    }
    std::fs::rename(tmp, target)
}

/// Renames (or moves within the workspace) an entry. A change of case only is allowed.
pub fn rename(root: &Path, from: &str, to: &str) -> FsResult<String> {
    let src = resolve(root, from)?;
    let dst = resolve_new(root, to)?;
    let shown = rel_of(root, &src);
    if !src.exists() {
        return Err(FsError::new(404, format!("« {} » est introuvable (déplacé ou supprimé en dehors de Studio ?).", shown)));
    }
    if same_path(&src, root) {
        return Err(FsError::new(400, "Impossible de renommer le dossier ouvert depuis Studio."));
    }
    if src == dst {
        return Ok(rel_of(root, &dst));
    }
    let case_only = same_path(&src, &dst);
    if !case_only {
        if dst.exists() {
            return Err(FsError::new(409, format!("« {} » existe déjà.", rel_of(root, &dst))));
        }
        if src.is_dir() && is_within(&dst, &src) {
            return Err(FsError::new(400, format!("Impossible de déplacer le dossier « {} » à l'intérieur de lui-même.", shown)));
        }
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_message(&e, &rel_of(root, parent)))?;
    }
    std::fs::rename(&src, &dst).map_err(|e| io_message(&e, &shown))?;
    Ok(rel_of(root, &dst))
}

/// A name that does not exist yet in `dir`: `nom.ext`, `nom (2).ext`, `nom (3).ext`…
pub fn unique_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (2..).map(|n| format!("{} ({}){}", stem, n, ext)).find(|n| !dir.join(n).exists()).unwrap()
}

/// The folder `dir` of the workspace, which must exist.
fn existing_dir(root: &Path, dir: &str) -> FsResult<PathBuf> {
    let d = resolve(root, dir)?;
    if !d.is_dir() {
        return Err(FsError::new(404, format!("Le dossier « {} » est introuvable.", rel_of(root, &d))));
    }
    Ok(d)
}

/// Moves entries into the folder `dir` (drag and drop, cut and paste). Returns the pairs
/// (old path, new path). Nothing is overwritten.
pub fn move_into(root: &Path, items: &[String], dir: &str) -> FsResult<Vec<(String, String)>> {
    let dest = existing_dir(root, dir)?;
    let mut moved = Vec::new();
    for it in items {
        let src = resolve(root, it)?;
        let shown = rel_of(root, &src);
        let name = src.file_name().ok_or_else(|| FsError::new(400, "Impossible de déplacer le dossier ouvert."))?.to_string_lossy().to_string();
        let dst = dest.join(&name);
        if same_path(&src, &dst) {
            continue; // already there
        }
        if src.is_dir() && is_within(&dest, &src) {
            return Err(FsError::new(400, format!("Impossible de déplacer le dossier « {} » à l'intérieur de lui-même.", shown)));
        }
        if dst.exists() {
            return Err(FsError::new(409, format!("« {} » existe déjà dans « {} ».", name, if dir.is_empty() { "le dossier ouvert".to_string() } else { rel_of(root, &dest) })));
        }
        std::fs::rename(&src, &dst).map_err(|e| io_message(&e, &shown))?;
        moved.push((shown, rel_of(root, &dst)));
    }
    Ok(moved)
}

fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir(dst)?;
        for e in std::fs::read_dir(src)? {
            let e = e?;
            copy_tree(&e.path(), &dst.join(e.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(src, dst).map(|_| ())
    }
}

/// Copies entries into the folder `dir`; a name already taken becomes `nom (2).ext`.
pub fn copy_into(root: &Path, items: &[String], dir: &str) -> FsResult<Vec<String>> {
    let dest = existing_dir(root, dir)?;
    let mut created = Vec::new();
    for it in items {
        let src = resolve(root, it)?;
        let shown = rel_of(root, &src);
        if !src.exists() {
            return Err(FsError::new(404, format!("« {} » est introuvable.", shown)));
        }
        if src.is_dir() && is_within(&dest, &src) {
            return Err(FsError::new(400, format!("Impossible de copier le dossier « {} » à l'intérieur de lui-même.", shown)));
        }
        let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let dst = dest.join(unique_name(&dest, &name));
        copy_tree(&src, &dst).map_err(|e| io_message(&e, &shown))?;
        created.push(rel_of(root, &dst));
    }
    Ok(created)
}

/// Adds a file coming from outside (dropped from the Explorer) into `dir`, under a free
/// name; `sub` is its path inside a dropped folder (`a/b/fichier.txt`), or its name.
pub fn import(root: &Path, dir: &str, sub: &str, content: &[u8]) -> FsResult<String> {
    let dest = existing_dir(root, dir)?;
    let names = parts(sub)?;
    let (last, folders) = names.split_last().ok_or_else(|| FsError::new(400, "Le nom est vide."))?;
    let mut d = dest;
    for f in folders {
        validate_name(f)?;
        d.push(f);
    }
    validate_name(last)?;
    std::fs::create_dir_all(&d).map_err(|e| io_message(&e, &rel_of(root, &d)))?;
    let p = d.join(unique_name(&d, last));
    std::fs::write(&p, content).map_err(|e| io_message(&e, &rel_of(root, &p)))?;
    Ok(rel_of(root, &p))
}

/// Copies files and folders given by their full path (dropped from the Explorer, pasted
/// from the clipboard of the system) into `dir`, under free names. Returns the new
/// relative paths.
pub fn import_paths(root: &Path, dir: &str, paths: &[PathBuf]) -> FsResult<Vec<String>> {
    let dest = existing_dir(root, dir)?;
    let mut created = Vec::new();
    for src in paths {
        let shown = src.to_string_lossy().to_string();
        if !src.exists() {
            return Err(FsError::new(404, format!("« {} » est introuvable.", shown)));
        }
        if src.is_dir() && is_within(&dest, src) {
            return Err(FsError::new(400, format!("Impossible de copier le dossier « {} » à l'intérieur de lui-même.", shown)));
        }
        let name = src.file_name().map(|n| n.to_string_lossy().to_string()).ok_or_else(|| FsError::new(400, format!("Impossible de copier « {} » (un disque entier ?).", shown)))?;
        let dst = dest.join(unique_name(&dest, &name));
        copy_tree(src, &dst).map_err(|e| io_message(&e, &shown))?;
        created.push(rel_of(root, &dst));
    }
    Ok(created)
}

/// The encodings of text files that Studio reads and writes back unchanged.
pub const ENCODINGS: &[&str] = &["utf-8", "utf-8-bom", "utf-16le", "utf-16be", "windows-1252"];

/// The characters 0x80–0x9F of Windows-1252 (the others are those of Latin-1).
const CP1252_HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ',
    '\u{9D}', 'ž', 'Ÿ',
];

/// The text of a file and its encoding: UTF-8 (with or without BOM), UTF-16 (with BOM),
/// else Windows-1252. `None` for a binary file.
pub fn decode_text(bytes: &[u8]) -> Option<(String, &'static str)> {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return Some((String::from_utf8_lossy(rest).into_owned(), "utf-8-bom"));
    }
    let utf16 = |rest: &[u8], le: bool| -> String {
        let units: Vec<u16> = rest.chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        String::from_utf16_lossy(&units)
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return Some((utf16(rest, true), "utf-16le"));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return Some((utf16(rest, false), "utf-16be"));
    }
    if bytes.iter().take(8000).any(|&b| b == 0) {
        return None;
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => Some((s.to_string(), "utf-8")),
        Err(_) => Some((bytes.iter().map(|&b| if (0x80..0xA0).contains(&b) { CP1252_HIGH[(b - 0x80) as usize] } else { b as char }).collect(), "windows-1252")),
    }
}

/// The bytes of `text` in `encoding`. A character that Windows-1252 cannot hold makes the
/// file UTF-8: the encoding really used is returned.
pub fn encode_text(text: &str, encoding: &str) -> (Vec<u8>, &'static str) {
    match encoding {
        "utf-8-bom" => {
            let mut v = vec![0xEF, 0xBB, 0xBF];
            v.extend_from_slice(text.as_bytes());
            (v, "utf-8-bom")
        }
        "utf-16le" | "utf-16be" => {
            let le = encoding == "utf-16le";
            let mut v = if le { vec![0xFF, 0xFE] } else { vec![0xFE, 0xFF] };
            for u in text.encode_utf16() {
                v.extend_from_slice(&if le { u.to_le_bytes() } else { u.to_be_bytes() });
            }
            (v, if le { "utf-16le" } else { "utf-16be" })
        }
        "windows-1252" => {
            let mut v = Vec::with_capacity(text.len());
            for c in text.chars() {
                let b = match c as u32 {
                    n @ (0..=0x7F | 0xA0..=0xFF) => Some(n as u8),
                    _ => CP1252_HIGH.iter().position(|&h| h == c).map(|i| 0x80 + i as u8),
                };
                match b {
                    Some(b) => v.push(b),
                    None => return (text.as_bytes().to_vec(), "utf-8"),
                }
            }
            (v, "windows-1252")
        }
        _ => (text.as_bytes().to_vec(), "utf-8"),
    }
}

/// Sends a file or folder of the workspace to the recycle bin.
pub fn trash(root: &Path, rel: &str) -> FsResult<()> {
    let p = resolve(root, rel)?;
    let shown = rel_of(root, &p);
    if same_path(&p, root) || rel.trim_matches(|c| c == '/' || c == '\\').is_empty() {
        return Err(FsError::new(400, "Impossible de supprimer le dossier ouvert depuis Studio."));
    }
    if !p.exists() {
        return Err(FsError::new(404, format!("« {} » est introuvable (déjà supprimé ?).", shown)));
    }
    recycle(&p).map_err(|e| match e {
        Some(e) => io_message(&e, &shown),
        None => FsError::new(500, format!("« {} » n'a pas pu être placé dans la Corbeille (fichier utilisé par un autre programme ?).", shown)),
    })?;
    if p.exists() {
        return Err(FsError::new(409, format!("« {} » n'a pas été supprimé : la suppression a été annulée, ou un autre programme l'utilise.", shown)));
    }
    Ok(())
}

#[cfg(windows)]
#[allow(non_snake_case)]
#[repr(C)]
struct SHFILEOPSTRUCTW {
    hwnd: isize,
    wFunc: u32,
    pFrom: *const u16,
    pTo: *const u16,
    fFlags: u16,
    fAnyOperationsAborted: i32,
    hNameMappings: *mut std::ffi::c_void,
    lpszProgressTitle: *const u16,
}

#[cfg(windows)]
#[link(name = "shell32")]
extern "system" {
    fn SHFileOperationW(op: *mut SHFILEOPSTRUCTW) -> i32;
}

/// The recycle bin of the shell (Windows): can be restored from the Explorer. If the
/// drive has no recycle bin, Windows asks before deleting for good.
#[cfg(windows)]
fn recycle(p: &Path) -> Result<(), Option<io::Error>> {
    use std::os::windows::ffi::OsStrExt;
    const FO_DELETE: u32 = 3;
    const FOF_SILENT: u16 = 0x0004;
    const FOF_NOCONFIRMATION: u16 = 0x0010;
    const FOF_ALLOWUNDO: u16 = 0x0040;
    const FOF_NOERRORUI: u16 = 0x0400;
    const FOF_WANTNUKEWARNING: u16 = 0x4000;
    let abs = std::path::absolute(p).map_err(Some)?;
    // A list of paths ended by two zeros.
    let mut from: Vec<u16> = abs.as_os_str().encode_wide().collect();
    from.push(0);
    from.push(0);
    let mut op = SHFILEOPSTRUCTW {
        hwnd: 0,
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI | FOF_WANTNUKEWARNING,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    let r = unsafe { SHFileOperationW(&mut op) };
    if r != 0 || op.fAnyOperationsAborted != 0 {
        // Codes below 0x71 are system error codes; the others are shell codes.
        return Err(if r > 0 && r < 0x71 { Some(io::Error::from_raw_os_error(r)) } else { None });
    }
    Ok(())
}

#[cfg(not(windows))]
fn recycle(p: &Path) -> Result<(), Option<io::Error>> {
    // Freedesktop trash: ~/.local/share/Trash/files (with its .trashinfo).
    let home = std::env::var_os("HOME").ok_or(None)?;
    let trash = Path::new(&home).join(".local/share/Trash");
    let files = trash.join("files");
    let info = trash.join("info");
    std::fs::create_dir_all(&files).map_err(Some)?;
    std::fs::create_dir_all(&info).map_err(Some)?;
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let target = unique_name(&files, &name);
    let abs = std::path::absolute(p).map_err(Some)?;
    std::fs::write(info.join(format!("{}.trashinfo", target)), format!("[Trash Info]\nPath={}\n", abs.display())).map_err(Some)?;
    std::fs::rename(p, files.join(target)).map_err(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tmp(PathBuf);
    impl Tmp {
        fn new(tag: &str) -> Tmp {
            let d = std::env::temp_dir().join(format!("nexul-studio-test-{}-{}", tag, std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Tmp(d)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn names() {
        assert!(validate_name("main.nx").is_ok());
        assert!(validate_name("été 2026.md").is_ok());
        assert!(validate_name(".gitignore").is_ok());
        for bad in ["", " ", ".", "..", "a<b", "a:b", "a?", "abc.", "abc ", "CON", "con.nx", "Lpt1.txt", "COM¹", "a\u{1}b"] {
            assert!(validate_name(bad).is_err(), "{:?} accepted", bad);
        }
        assert!(validate_name("CONSOLE.nx").is_ok());
        assert!(resolve(Path::new("C:/r"), "../x").is_err());
        assert!(resolve(Path::new("C:/r"), "a/../../x").is_err());
    }

    #[test]
    fn create_save_rename() {
        let t = Tmp::new("csr");
        let r = &t.0;
        assert_eq!(create(r, "src/a.nx", Kind::File, b"x").unwrap(), "src/a.nx");
        assert_eq!(create(r, "src\\lib", Kind::Dir, b"").unwrap(), "src/lib");
        assert_eq!(create(r, "src/a.nx", Kind::File, b"").unwrap_err().status, 409);
        assert_eq!(create(r, "src/a.nx/b.nx", Kind::File, b"").unwrap_err().status, 409);
        assert_eq!(create(r, "x/abc.", Kind::File, b"").unwrap_err().status, 400);
        assert!(!r.join("x").exists(), "nothing is created for an invalid name");
        save(r, "src/a.nx", b"fn main() {}").unwrap();
        assert_eq!(std::fs::read(r.join("src/a.nx")).unwrap(), b"fn main() {}");
        save(r, "new/dir/b.nx", b"b").unwrap();
        // A change of case only.
        assert_eq!(rename(r, "src/a.nx", "src/A.nx").unwrap(), "src/A.nx");
        assert!(std::fs::read_dir(r.join("src")).unwrap().any(|e| e.unwrap().file_name() == "A.nx"));
        assert_eq!(rename(r, "src/A.nx", "src/lib/a.nx").unwrap(), "src/lib/a.nx");
        assert_eq!(rename(r, "src", "src/lib/deeper").unwrap_err().status, 400);
        assert_eq!(rename(r, "nope", "other").unwrap_err().status, 404);
        create(r, "c.nx", Kind::File, b"").unwrap();
        assert_eq!(rename(r, "c.nx", "src").unwrap_err().status, 409);
    }

    #[test]
    fn read_only_file() {
        let t = Tmp::new("ro");
        let r = &t.0;
        create(r, "ro.nx", Kind::File, b"a").unwrap();
        let p = r.join("ro.nx");
        let mut perm = std::fs::metadata(&p).unwrap().permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&p, perm.clone()).unwrap();
        let e = save(r, "ro.nx", b"b").unwrap_err();
        assert!(e.message.contains("lecture seule"), "{}", e.message);
        assert_eq!(std::fs::read(&p).unwrap(), b"a", "unchanged");
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        std::fs::set_permissions(&p, perm).unwrap();
    }

    #[test]
    fn atomic_save() {
        let t = Tmp::new("atomic");
        let r = &t.0;
        create(r, "a.nx", Kind::File, b"old").unwrap();
        #[cfg(windows)]
        {
            // The attributes of the file are kept (here: hidden).
            let ok = std::process::Command::new("attrib").arg("+h").arg(r.join("a.nx")).status().map(|s| s.success()).unwrap_or(false);
            assert!(ok);
        }
        save(r, "a.nx", "nouveau contenu é".as_bytes()).unwrap();
        assert_eq!(std::fs::read_to_string(r.join("a.nx")).unwrap(), "nouveau contenu é");
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            assert!(std::fs::metadata(r.join("a.nx")).unwrap().file_attributes() & 0x2 != 0, "still hidden");
        }
        let left: Vec<_> = std::fs::read_dir(r).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(left, ["a.nx"], "no temporary file left");
        assert!(crate::watch::is_ignored("src/.a.nx.12.nxtmp"));
    }

    #[test]
    fn move_copy_import() {
        let t = Tmp::new("mci");
        let r = &t.0;
        create(r, "a.nx", Kind::File, b"a").unwrap();
        create(r, "d/x.nx", Kind::File, b"x").unwrap();
        create(r, "e", Kind::Dir, b"").unwrap();
        let moved = move_into(r, &["a.nx".into(), "d".into()], "e").unwrap();
        assert_eq!(moved, vec![("a.nx".to_string(), "e/a.nx".to_string()), ("d".to_string(), "e/d".to_string())]);
        assert_eq!(move_into(r, &["e".into()], "e/d").unwrap_err().status, 400);
        assert!(move_into(r, &["e/a.nx".into()], "e").unwrap().is_empty());
        assert_eq!(copy_into(r, &["e/a.nx".into()], "e").unwrap(), vec!["e/a (2).nx".to_string()]);
        assert_eq!(copy_into(r, &["e/d".into()], "").unwrap(), vec!["d".to_string()]);
        assert_eq!(std::fs::read(r.join("d/x.nx")).unwrap(), b"x");
        assert_eq!(copy_into(r, &["e".into()], "e/d").unwrap_err().status, 400);
        assert_eq!(import(r, "", "photo.png", b"p").unwrap(), "photo.png");
        assert_eq!(import(r, "", "photo.png", b"q").unwrap(), "photo (2).png");
        assert_eq!(import(r, "e", "dossier/sous/n.txt", b"n").unwrap(), "e/dossier/sous/n.txt");
        assert!(import(r, "", "../evil.txt", b"").is_err());
        create(r, "e/a.nx.bak", Kind::File, b"").unwrap();
        assert_eq!(move_into(r, &["d/x.nx".into()], "missing").unwrap_err().status, 404);
        // Files and folders from outside of the workspace.
        let out = Tmp::new("mci-out");
        create(&out.0, "dehors/y.txt", Kind::File, b"y").unwrap();
        create(&out.0, "z.txt", Kind::File, b"z").unwrap();
        let paths = [out.0.join("dehors"), out.0.join("z.txt")];
        assert_eq!(import_paths(r, "e", &paths).unwrap(), vec!["e/dehors".to_string(), "e/z.txt".to_string()]);
        assert_eq!(import_paths(r, "e", &paths[1..]).unwrap(), vec!["e/z (2).txt".to_string()]);
        assert_eq!(std::fs::read(r.join("e/dehors/y.txt")).unwrap(), b"y");
        assert_eq!(import_paths(r, "e", &[r.join("e")]).unwrap_err().status, 400);
        assert_eq!(import_paths(r, "", &[out.0.join("absent")]).unwrap_err().status, 404);
    }

    #[test]
    fn encodings() {
        let text = "Ça coûte 5 € — “bien”\r\n";
        for enc in ENCODINGS {
            let (bytes, used) = encode_text(text, enc);
            assert_eq!(&used, enc);
            let (back, found) = decode_text(&bytes).unwrap();
            assert_eq!(back, text, "{}", enc);
            assert_eq!(&found, enc);
        }
        // Windows-1252 cannot hold an emoji: the file becomes UTF-8.
        let (bytes, used) = encode_text("😀 é", "windows-1252");
        assert_eq!(used, "utf-8");
        assert_eq!(decode_text(&bytes).unwrap(), ("😀 é".to_string(), "utf-8"));
        assert_eq!(decode_text(b"\x00\x01binary"), None);
        assert_eq!(decode_text(&[0x63, 0x61, 0x66, 0xE9]).unwrap(), ("café".to_string(), "windows-1252"));
    }

    #[test]
    fn trash_entries() {
        let t = Tmp::new("trash");
        let r = &t.0;
        create(r, "t.txt", Kind::File, b"t").unwrap();
        create(r, "dir/u.txt", Kind::File, b"u").unwrap();
        assert_eq!(trash(r, "").unwrap_err().status, 400);
        assert_eq!(trash(r, "nope.txt").unwrap_err().status, 404);
        if std::env::var_os("NEXUL_STUDIO_TEST_TRASH").is_some() {
            // Uses the real recycle bin of the user: only on demand.
            trash(r, "t.txt").unwrap();
            trash(r, "dir").unwrap();
            assert!(!r.join("t.txt").exists() && !r.join("dir").exists());
        }
    }
}

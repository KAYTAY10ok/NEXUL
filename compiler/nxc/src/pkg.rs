//! `nxpm`, the package manager of NEXUL (spec §11): projects described by `nexul.toml`,
//! dependencies taken from a folder or a Git repository, versions pinned with their
//! integrity in `nexul.lock`.
//!
//! ```toml
//! [package]
//! name = "agenda"
//! version = "0.1.0"
//!
//! [dependencies]
//! dates = { path = "../dates" }
//! json = { git = "https://github.com/exemple/nexul-json", tag = "v1.2.0" }
//! ```
//!
//! The source of a package is its `src` folder (or the package folder itself):
//! `import json` reads `src/lib.nx`, `import json.parser` reads `src/parser.nx`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MANIFEST: &str = "nexul.toml";
pub const LOCK: &str = "nexul.lock";

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Path(String),
    Git { url: String, rev: Option<String>, tag: Option<String>, branch: Option<String> },
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub main: Option<String>,
    pub deps: BTreeMap<String, Source>,
}

/// A resolved dependency.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub name: String,
    /// `path+<dir>` or `git+<url>#<commit>`.
    pub source: String,
    pub dir: PathBuf,
    pub checksum: String,
}

// ---------------------------------------------------------------------------
// TOML subset: tables, `key = "string"`, inline tables `{ k = "v", ... }`.

fn unquote(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

fn parse_inline(s: &str) -> Option<BTreeMap<String, String>> {
    let inner = s.trim().strip_prefix('{')?.strip_suffix('}')?;
    let mut map = BTreeMap::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut parts = Vec::new();
    for c in inner.chars() {
        match c {
            '"' => {
                in_str = !in_str;
                cur.push(c);
            }
            ',' if !in_str => parts.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur);
    }
    for p in parts {
        let (k, v) = p.split_once('=')?;
        map.insert(k.trim().to_string(), unquote(v)?);
    }
    Some(map)
}

pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let mut m = Manifest { name: String::new(), version: "0.1.0".into(), description: String::new(), main: None, deps: BTreeMap::new() };
    let mut section = String::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.split(" #").next().unwrap_or("").trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']').trim().to_string();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { return Err(format!("{}:{}: expected `key = value`", MANIFEST, i + 1)) };
        let (k, v) = (k.trim(), v.trim());
        match section.as_str() {
            "package" => {
                let s = unquote(v).ok_or_else(|| format!("{}:{}: `{}` must be a string", MANIFEST, i + 1, k))?;
                match k {
                    "name" => m.name = s,
                    "version" => m.version = s,
                    "description" => m.description = s,
                    "main" => m.main = Some(s),
                    _ => {}
                }
            }
            "dependencies" => {
                let src = if let Some(s) = unquote(v) {
                    Source::Path(s)
                } else {
                    let t = parse_inline(v).ok_or_else(|| format!("{}:{}: expected `{{ path = \"...\" }}` or `{{ git = \"...\" }}`", MANIFEST, i + 1))?;
                    if let Some(p) = t.get("path") {
                        Source::Path(p.clone())
                    } else if let Some(u) = t.get("git") {
                        Source::Git { url: u.clone(), rev: t.get("rev").cloned(), tag: t.get("tag").cloned(), branch: t.get("branch").cloned() }
                    } else {
                        return Err(format!("{}:{}: dependency `{}` needs `path` or `git`", MANIFEST, i + 1, k));
                    }
                };
                m.deps.insert(k.to_string(), src);
            }
            _ => {}
        }
    }
    if m.name.is_empty() {
        return Err(format!("{}: [package] needs a `name`", MANIFEST));
    }
    Ok(m)
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn write_manifest(m: &Manifest) -> String {
    let mut s = format!("[package]\nname = {}\nversion = {}\n", quote(&m.name), quote(&m.version));
    if !m.description.is_empty() {
        s.push_str(&format!("description = {}\n", quote(&m.description)));
    }
    if let Some(main) = &m.main {
        s.push_str(&format!("main = {}\n", quote(main)));
    }
    s.push_str("\n[dependencies]\n");
    for (name, src) in &m.deps {
        match src {
            Source::Path(p) => s.push_str(&format!("{} = {{ path = {} }}\n", name, quote(p))),
            Source::Git { url, rev, tag, branch } => {
                let mut parts = vec![format!("git = {}", quote(url))];
                if let Some(r) = rev {
                    parts.push(format!("rev = {}", quote(r)));
                }
                if let Some(t) = tag {
                    parts.push(format!("tag = {}", quote(t)));
                }
                if let Some(b) = branch {
                    parts.push(format!("branch = {}", quote(b)));
                }
                s.push_str(&format!("{} = {{ {} }}\n", name, parts.join(", ")));
            }
        }
    }
    s
}

// ---------------------------------------------------------------------------
// SHA-256 (FIPS 180-4), for the integrity of packages.

pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74,
        0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d,
        0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e,
        0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    let mut out = [0u8; 32];
    for i in 0..8 {
        out[4 * i..4 * i + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// The integrity of a package: SHA-256 of its NEXUL sources and manifest (paths and
/// contents, in path order, line endings normalized).
pub fn checksum(dir: &Path) -> String {
    let mut files = Vec::new();
    collect_files(dir, dir, &mut files);
    files.sort();
    let mut data = Vec::new();
    for rel in files {
        let text = std::fs::read(dir.join(&rel)).unwrap_or_default();
        let text = String::from_utf8_lossy(&text).replace("\r\n", "\n");
        data.extend_from_slice(rel.as_bytes());
        data.push(0);
        data.extend_from_slice(text.as_bytes());
        data.push(0);
    }
    format!("sha256:{}", hex(&sha256(&data)))
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "target" {
            continue;
        }
        if p.is_dir() {
            collect_files(root, &p, out);
        } else if name.ends_with(".nx") || name == MANIFEST {
            out.push(p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/"));
        }
    }
}

// ---------------------------------------------------------------------------
// Projects

/// The project containing `start` (a file or folder): the folder of its `nexul.toml`.
pub fn find_project(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_dir() { start.to_path_buf() } else { start.parent()?.to_path_buf() };
    if dir.as_os_str().is_empty() {
        dir = PathBuf::from(".");
    }
    let mut dir = std::fs::canonicalize(&dir).ok()?;
    loop {
        if dir.join(MANIFEST).is_file() {
            return Some(strip_verbatim(&dir));
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => p.to_path_buf(),
    }
}

pub fn load_manifest(project: &Path) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(project.join(MANIFEST)).map_err(|e| format!("cannot read {}: {}", MANIFEST, e))?;
    parse_manifest(&text)
}

/// The main file of a project.
pub fn main_file(project: &Path, m: &Manifest) -> PathBuf {
    if let Some(main) = &m.main {
        return project.join(main);
    }
    for c in ["src/main.nx", "main.nx", "src/lib.nx", "lib.nx"] {
        if project.join(c).is_file() {
            return project.join(c);
        }
    }
    project.join("src/main.nx")
}

/// Where Git packages are kept: `%LOCALAPPDATA%\nexul\packages` or `~/.cache/nexul/packages`.
fn packages_dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("nexul")
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache").join("nexul")).unwrap_or_else(std::env::temp_dir)
    };
    base.join("packages")
}

/// The folder holding the NEXUL sources of a package.
pub fn source_dir(pkg_dir: &Path) -> PathBuf {
    let src = pkg_dir.join("src");
    if src.is_dir() {
        src
    } else {
        pkg_dir.to_path_buf()
    }
}

fn git(args: &[&str], dir: Option<&Path>) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    let out = cmd.output().map_err(|e| format!("cannot run git ({}); install Git to use Git dependencies", e))?;
    if !out.status.success() {
        return Err(format!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Reads `nexul.lock`: name -> (source, checksum).
pub fn read_lock(project: &Path) -> BTreeMap<String, (String, String)> {
    let mut map = BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(project.join(LOCK)) else { return map };
    let (mut name, mut source, mut sum) = (String::new(), String::new(), String::new());
    let flush = |name: &mut String, source: &mut String, sum: &mut String, map: &mut BTreeMap<String, (String, String)>| {
        if !name.is_empty() {
            map.insert(std::mem::take(name), (std::mem::take(source), std::mem::take(sum)));
        }
    };
    for line in text.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            flush(&mut name, &mut source, &mut sum, &mut map);
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = unquote(v).unwrap_or_default();
            match k.trim() {
                "name" => name = v,
                "source" => source = v,
                "checksum" => sum = v,
                _ => {}
            }
        }
    }
    flush(&mut name, &mut source, &mut sum, &mut map);
    map
}

fn write_lock(project: &Path, deps: &[Resolved]) -> Result<(), String> {
    let mut s = String::from("# nexul.lock: exact versions and integrity of the dependencies (generated by nxpm).\n");
    for d in deps {
        s.push_str(&format!("\n[[package]]\nname = {}\nsource = {}\nchecksum = {}\n", quote(&d.name), quote(&d.source), quote(&d.checksum)));
    }
    std::fs::write(project.join(LOCK), s).map_err(|e| format!("cannot write {}: {}", LOCK, e))
}

/// Resolves (and fetches) the dependencies of a project, transitively. With `update`,
/// Git dependencies move to the newest commit of their branch or tag; otherwise the
/// commits of `nexul.lock` are used. The checksum of every package is compared with the
/// lock file: a package changed since it was locked is refused.
pub fn resolve(project: &Path, update: bool, log: &mut dyn FnMut(&str)) -> Result<Vec<Resolved>, String> {
    let lock = read_lock(project);
    let mut out: Vec<Resolved> = Vec::new();
    let mut queue: Vec<(String, Source, PathBuf)> = Vec::new();
    let root = load_manifest(project)?;
    for (n, s) in root.deps {
        queue.push((n, s, project.to_path_buf()));
    }
    while let Some((name, src, base)) = queue.pop() {
        if out.iter().any(|r| r.name == name) {
            continue;
        }
        let (dir, source) = match &src {
            Source::Path(p) => {
                let dir = base.join(p);
                if !dir.is_dir() {
                    return Err(format!("dependency `{}`: folder `{}` not found", name, dir.display()));
                }
                let dir = std::fs::canonicalize(&dir).map(|d| strip_verbatim(&d)).unwrap_or(dir);
                (dir.clone(), format!("path+{}", p))
            }
            Source::Git { url, rev, tag, branch } => {
                let locked = lock.get(&name).and_then(|(s, _)| s.strip_prefix(&format!("git+{}#", url)).map(|c| c.to_string()));
                let mirror = packages_dir().join("git").join(hex(&sha256(url.as_bytes()))[..16].to_string());
                if !mirror.join(".git").is_dir() {
                    log(&format!("cloning {}", url));
                    std::fs::create_dir_all(mirror.parent().unwrap()).map_err(|e| e.to_string())?;
                    git(&["clone", "--quiet", url, &mirror.to_string_lossy()], None)?;
                } else if update || locked.is_none() {
                    log(&format!("fetching {}", url));
                    git(&["fetch", "--quiet", "--tags", "origin"], Some(&mirror))?;
                }
                let wanted = match (rev, tag, branch, &locked, update) {
                    (Some(r), _, _, _, _) => r.clone(),
                    (_, _, _, Some(c), false) => c.clone(),
                    (_, Some(t), _, _, _) => format!("refs/tags/{}", t),
                    (_, _, Some(b), _, _) => format!("origin/{}", b),
                    _ => "origin/HEAD".to_string(),
                };
                let commit = git(&["rev-parse", &format!("{}^{{commit}}", wanted)], Some(&mirror))?;
                let dir = packages_dir().join(format!("{}-{}", name, &commit[..12.min(commit.len())]));
                if !dir.is_dir() {
                    log(&format!("checking out {} {}", name, &commit[..12.min(commit.len())]));
                    git(&["worktree", "add", "--force", "--detach", &dir.to_string_lossy(), &commit], Some(&mirror)).or_else(|_| {
                        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                        git(&["--work-tree", &dir.to_string_lossy(), "checkout", &commit, "--", "."], Some(&mirror))
                    })?;
                }
                (dir, format!("git+{}#{}", url, commit))
            }
        };
        let sum = checksum(&dir);
        if let Some((lsrc, lsum)) = lock.get(&name) {
            if *lsrc == source && !lsum.is_empty() && *lsum != sum && !matches!(src, Source::Path(_)) {
                return Err(format!(
                    "integrity check failed for `{}`: its content changed since it was locked\n  expected {}\n  found    {}\n(run `nxpm update {}` if the change is intended)",
                    name, lsum, sum, name
                ));
            }
        }
        if let Ok(m) = load_manifest(&dir) {
            for (n, s) in m.deps {
                queue.push((n, s, dir.clone()));
            }
        }
        out.push(Resolved { name, source, dir, checksum: sum });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    write_lock(project, &out)?;
    Ok(out)
}

/// The dependencies of the project containing `file`, as given to the compiler:
/// (package name, source folder). Packages already fetched are used as locked.
pub fn deps_for(file: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let Some(project) = find_project(file) else { return Ok(vec![]) };
    let m = load_manifest(&project)?;
    if m.deps.is_empty() {
        return Ok(vec![]);
    }
    let resolved = resolve(&project, false, &mut |msg| eprintln!("nxpm: {}", msg))?;
    Ok(resolved.into_iter().map(|r| (r.name, source_dir(&r.dir))).collect())
}

/// A new project: `nexul.toml`, `src/main.nx` and a `.gitignore`.
pub fn new_project(dir: &Path, name: &str) -> Result<(), String> {
    if dir.join(MANIFEST).exists() {
        return Err(format!("`{}` already contains a {}", dir.display(), MANIFEST));
    }
    std::fs::create_dir_all(dir.join("src")).map_err(|e| e.to_string())?;
    let m = Manifest { name: name.to_string(), version: "0.1.0".into(), description: String::new(), main: None, deps: BTreeMap::new() };
    std::fs::write(dir.join(MANIFEST), write_manifest(&m)).map_err(|e| e.to_string())?;
    let main = dir.join("src").join("main.nx");
    if !main.exists() {
        std::fs::write(
            &main,
            format!("// {} — un programme NEXUL.\n\nfn main() {{\n    println(\"Bonjour depuis {} !\")\n}}\n\n@test\nfn exemple() {{\n    assert(1 + 1 == 2)\n}}\n", name, name),
        )
        .map_err(|e| e.to_string())?;
    }
    let gi = dir.join(".gitignore");
    if !gi.exists() {
        let _ = std::fs::write(gi, "/target\n*.exe\n*.pdb\n");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_vectors() {
        assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let long = vec![b'a'; 1_000_000];
        assert_eq!(hex(&sha256(&long)), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    }

    #[test]
    fn manifest_round_trip() {
        let text = "[package]\nname = \"agenda\"\nversion = \"1.0.0\"\n\n[dependencies]\ndates = { path = \"../dates\" }\njson = { git = \"https://example.org/json\", tag = \"v1\" }\n";
        let m = parse_manifest(text).unwrap();
        assert_eq!(m.name, "agenda");
        assert_eq!(m.deps["dates"], Source::Path("../dates".into()));
        assert!(matches!(&m.deps["json"], Source::Git { tag: Some(t), .. } if t == "v1"));
        assert_eq!(parse_manifest(&write_manifest(&m)).unwrap().deps, m.deps);
    }
}

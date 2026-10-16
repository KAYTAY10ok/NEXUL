//! Locating the NEXUL compiler (`nxc`) and its tools.

use std::path::{Path, PathBuf};

/// The name of a program on this system (`nxc.exe` on Windows).
fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    }
}

/// Finds the NEXUL compiler: next to the Studio (installation), then the environment
/// (`NEXUL_NXC`, `NEXUL_HOME`, `PATH`) and a build of the repository.
pub fn find_nxc() -> Option<PathBuf> {
    let nxc = exe("nxc");
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(me) = std::env::current_exe() {
        if let Some(dir) = me.parent() {
            candidates.push(dir.join(&nxc));
            candidates.push(dir.join("bin").join(&nxc));
        }
    }
    if let Some(p) = std::env::var_os("NEXUL_NXC") {
        candidates.push(PathBuf::from(p));
    }
    if let Some(home) = std::env::var_os("NEXUL_HOME") {
        candidates.push(Path::new(&home).join("bin").join(&nxc));
        candidates.push(Path::new(&home).join("target").join("release").join(&nxc));
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join(&nxc)));
    }
    // A build of the repository (developers).
    if cfg!(windows) {
        candidates.push(PathBuf::from(r"C:\NEXUL\target\release\nxc.exe"));
        candidates.push(PathBuf::from(r"C:\NEXUL\target\debug\nxc.exe"));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// `nxpm` next to `nxc`.
pub fn nxpm_of(nxc: &Path) -> Option<PathBuf> {
    let p = nxc.with_file_name(exe("nxpm"));
    p.is_file().then_some(p)
}

/// Does `nxc --version` say it reads edition 1 of the syntax (ADR 0010)?
/// `nxc 0.2.0 (NEXUL 0.2, editions 0.2 and 1)`; older compilers do not name editions.
pub fn knows_edition_1(nxc: &Path) -> bool {
    let Ok(out) = crate::runner::command(nxc, &["--version".to_string()], Path::new(".")).output() else { return false };
    editions_say_1(&String::from_utf8_lossy(&out.stdout))
}

fn editions_say_1(version: &str) -> bool {
    version.split_once("editions").is_some_and(|(_, list)| list.split(|c: char| !(c.is_ascii_digit() || c == '.')).any(|e| e == "1" || e == "1.0"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editions_of_the_version() {
        assert!(editions_say_1("nxc 0.2.0 (NEXUL 0.2, editions 0.2 and 1)\n"));
        assert!(!editions_say_1("nxc 0.2.0 (NEXUL 0.2)\n"));
        assert!(!editions_say_1("nxc 0.2.0 (NEXUL 0.2, editions 0.2)"));
        assert!(!editions_say_1(""));
    }
}

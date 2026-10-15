//! Helpers shared by the tests that run the real installer in a sandbox
//! (`NEXUL_SETUP_SANDBOX`, feature `test-sandbox`).

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const SETUP: &str = env!("CARGO_BIN_EXE_NEXUL-Setup");
pub const PACK: &str = env!("CARGO_BIN_EXE_nexul-pack");
pub const SLEEPER: &str = env!("CARGO_BIN_EXE_nexul-test-sleeper");
pub const UNINSTALLER: &str = "Désinstaller NEXUL.exe";

pub struct Sandbox {
    pub root: PathBuf,
    pub name: String,
}

impl Sandbox {
    pub fn new(tag: &str) -> Sandbox {
        let name = format!("nexul-setup-e2e-{}-{tag}", std::process::id());
        let root = std::env::temp_dir().join(&name);
        let _ = std::fs::remove_dir_all(&root);
        for d in ["Temp", "Home", "AppData", "LocalAppData", "ProgramFiles", "Staging", "Elsewhere"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        Sandbox { root, name }
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// The real key that stands for `logical` (`HKCU\...`) in the sandbox.
    pub fn key(&self, logical: &str) -> String {
        let (root, rest) = logical.split_once('\\').unwrap();
        format!(r"HKCU\Software\NEXUL-Tests\{}\{}\{}", self.name, root, rest)
    }

    /// A program run with the sandbox.
    pub fn command(&self, program: &Path) -> Command {
        let mut c = Command::new(program);
        c.env("NEXUL_SETUP_SANDBOX", &self.root)
            .env("TEMP", self.path("Temp"))
            .env("TMP", self.path("Temp"))
            .env("USERPROFILE", self.path("Home"))
            .env("APPDATA", self.path("AppData"))
            .env("LOCALAPPDATA", self.path("LocalAppData"))
            .env("ProgramFiles", self.path("ProgramFiles"))
            .env("ProgramFiles(x86)", self.path("ProgramFiles"))
            .env("ProgramW6432", self.path("ProgramFiles"))
            .stdin(Stdio::null());
        c
    }

    pub fn run_silent(&self, program: &Path, args: &[&str]) {
        let status = self.command(program).arg("--silent").args(args).status().unwrap();
        let error = std::fs::read_to_string(self.path(r"Temp\nexul-setup-error.txt")).unwrap_or_default();
        assert!(status.success(), "{} {args:?}: {status}\n{error}", program.display());
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let key = format!(r"HKCU\Software\NEXUL-Tests\{}", self.name);
        let _ = Command::new("reg").args(["delete", &key, "/f"]).output();
        // Their parent too, if no other test uses it (`reg query` lists no subkey).
        let root = r"HKCU\Software\NEXUL-Tests";
        let out = Command::new("reg").args(["query", root]).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        if out.status.success()
            && !text.lines().any(|l| l.trim().to_uppercase().starts_with(r"HKEY_CURRENT_USER\SOFTWARE\NEXUL-TESTS\"))
        {
            let _ = Command::new("reg").args(["delete", root, "/f"]).output();
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// `(type, value)` of a registry value read by `reg.exe` (`None`: the default value).
pub fn reg_value(key: &str, name: Option<&str>) -> Option<(String, String)> {
    let mut c = Command::new("reg");
    c.args(["query", key]);
    match name {
        Some(n) => c.args(["/v", n]),
        None => c.arg("/ve"),
    };
    let out = c.output().unwrap();
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let line = text.lines().find(|l| l.contains("    REG_"))?;
    let fields: Vec<&str> = line.trim_start().splitn(3, "    ").collect();
    Some((fields.get(1)?.to_string(), fields.get(2).unwrap_or(&"").to_string()))
}

pub fn key_exists(key: &str) -> bool {
    Command::new("reg").args(["query", key]).output().unwrap().status.success()
}

/// The target of a shortcut, read by Windows Script Host (independent of the installer).
pub fn shortcut_target(lnk: &Path) -> String {
    let script = format!(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; (New-Object -ComObject WScript.Shell).CreateShortcut('{}').TargetPath",
        lnk.display().to_string().replace('\'', "''")
    );
    let out = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

pub fn create_shortcut(lnk: &Path, target: &Path) {
    let script = format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}'); $s.TargetPath='{}'; $s.Save()",
        lnk.display().to_string().replace('\'', "''"),
        target.display().to_string().replace('\'', "''")
    );
    let ok = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]).status().unwrap();
    assert!(ok.success());
}

/// Builds an installer from `files` (`component`, path, content; `None`: a copy of the
/// sleeper program).
pub fn pack(sb: &Sandbox, out: &str, files: &[(&str, &str, Option<&str>)]) -> PathBuf {
    let staging = sb.path(&format!(r"Staging\{out}"));
    let _ = std::fs::remove_dir_all(&staging);
    for (comp, rel, content) in files {
        let p = staging.join(comp).join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        match content {
            Some(text) => std::fs::write(&p, text).unwrap(),
            None => {
                std::fs::copy(SLEEPER, &p).unwrap();
            }
        }
    }
    std::fs::write(
        staging.join("components.txt"),
        "core\t1\t1\tCompilateur NEXUL\tnxc et nxpm\nstudio\t0\t1\tNEXUL Studio\tL'environnement\nsamples\t0\t1\tExemples\tProgrammes d'exemple\n",
    )
    .unwrap();
    let exe = sb.path(&format!("{out}.exe"));
    let ok = Command::new(PACK).arg(SETUP).arg(&staging).arg(&exe).stdout(Stdio::null()).status().unwrap();
    assert!(ok.success());
    exe
}

pub fn sleeper(sb: &Sandbox, exe: &Path) -> Child {
    sb.command(exe).arg("120").stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap()
}

pub fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while !f() {
        assert!(t0.elapsed() < Duration::from_secs(30), "{what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

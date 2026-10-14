//! Debugging through `lldb-dap` (Debug Adapter Protocol): the server only carries the
//! messages; the interface speaks the protocol (`ui/ide.js`).

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub struct Debugger {
    child: Child,
    stdin: ChildStdin,
    /// Every message received from the adapter (JSON text), in order.
    pub messages: Arc<Mutex<Vec<String>>>,
    pub done: Arc<Mutex<bool>>,
}

/// Finds `lldb-dap`: next to NEXUL's LLVM, then in the usual LLVM folders.
pub fn find_adapter(nxc: Option<&Path>) -> Option<PathBuf> {
    let name = if cfg!(windows) { "lldb-dap.exe" } else { "lldb-dap" };
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(v) = std::env::var_os("NEXUL_LLVM") {
        dirs.push(PathBuf::from(&v).join("bin"));
        dirs.push(PathBuf::from(v));
    }
    if let Some(nxc) = nxc.and_then(|n| n.parent()) {
        dirs.push(nxc.join("llvm").join("bin"));
        dirs.push(nxc.join("..").join("llvm").join("bin"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.join("llvm").join("bin"));
            dirs.push(d.join("..").join("llvm").join("bin"));
        }
    }
    dirs.push(PathBuf::from(r"C:\Program Files\LLVM\bin"));
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

impl Debugger {
    pub fn start(adapter: &Path) -> std::io::Result<Debugger> {
        let mut cmd = Command::new(adapter);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        // The adapter's Python runtime (debugger component of the installer).
        if let Some(dir) = adapter.parent() {
            let py = dir.join("python");
            if py.is_dir() {
                let mut path = std::ffi::OsString::from(&py);
                path.push(";");
                path.push(std::env::var_os("PATH").unwrap_or_default());
                cmd.env("PATH", path).env("PYTHONHOME", &py);
            }
        }
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000);
        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let done = Arc::new(Mutex::new(false));
        let (m, d) = (messages.clone(), done.clone());
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            loop {
                // Headers, then `Content-Length` bytes of JSON.
                let mut len = 0usize;
                loop {
                    let mut line = String::new();
                    match r.read_line(&mut line) {
                        Ok(0) | Err(_) => {
                            *d.lock().unwrap() = true;
                            return;
                        }
                        Ok(_) => {}
                    }
                    let l = line.trim();
                    if l.is_empty() {
                        break;
                    }
                    if let Some(v) = l.strip_prefix("Content-Length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut buf = vec![0u8; len];
                if r.read_exact(&mut buf).is_err() {
                    *d.lock().unwrap() = true;
                    return;
                }
                m.lock().unwrap().push(String::from_utf8_lossy(&buf).to_string());
            }
        });
        Ok(Debugger { child, stdin, messages, done })
    }

    pub fn send(&mut self, json: &str) -> std::io::Result<()> {
        let frame = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
        self.stdin.write_all(frame.as_bytes())?;
        self.stdin.flush()
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

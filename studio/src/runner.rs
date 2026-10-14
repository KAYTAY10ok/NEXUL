//! Running programs from the editor: the output is collected in a buffer that the
//! interface polls, and the standard input can be written to.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Do not open a console window for child processes.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct Run {
    pub output: Arc<Mutex<Vec<u8>>>,
    pub done: Arc<Mutex<Option<i32>>>,
    /// Duration of each finished step, in seconds.
    pub times: Arc<Mutex<Vec<f64>>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    child: Arc<Mutex<Option<Child>>>,
}

#[derive(Default)]
pub struct Runs {
    next: u32,
    pub map: HashMap<u32, Run>,
}

/// A command prepared for running in the background.
pub fn command(program: &Path, args: &[String], cwd: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(cwd);
    // Tools installed for the user (cargo, winget links) may not be in the PATH of a GUI process.
    let mut path = std::env::var_os("PATH").unwrap_or_default();
    let mut extra: Vec<PathBuf> = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        extra.push(Path::new(&local).join("Microsoft").join("WinGet").join("Links"));
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        extra.push(Path::new(&home).join(".cargo").join("bin"));
    }
    for e in extra {
        let mut p = std::ffi::OsString::from(e);
        p.push(";");
        p.push(&path);
        path = p;
    }
    cmd.env("PATH", path);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

fn pump(mut src: impl Read + Send + 'static, out: Arc<Mutex<Vec<u8>>>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match src.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => out.lock().unwrap().extend_from_slice(&buf[..n]),
            }
        }
    })
}

/// One step of a run: a command, and a message printed before it.
pub struct Step {
    pub banner: String,
    pub cmd: Command,
    /// Whether the step's standard input is connected to the editor's terminal.
    pub interactive: bool,
    /// When set, a line « label N,NN s » is printed after the step succeeds.
    pub report: Option<String>,
}

/// Seconds with two decimals and a decimal comma (French typography).
pub fn secs_fr(secs: f64) -> String {
    format!("{:.2}", secs).replace('.', ",")
}

impl Runs {
    /// Starts the steps in sequence; a step that fails stops the run.
    pub fn start(&mut self, steps: Vec<Step>, cleanup: Vec<PathBuf>) -> u32 {
        self.next += 1;
        let id = self.next;
        let output = Arc::new(Mutex::new(Vec::new()));
        let done = Arc::new(Mutex::new(None));
        let times = Arc::new(Mutex::new(Vec::new()));
        let stdin: Arc<Mutex<Option<ChildStdin>>> = Arc::new(Mutex::new(None));
        let child_slot: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        let (o, d, si, cs, tm) = (output.clone(), done.clone(), stdin.clone(), child_slot.clone(), times.clone());
        std::thread::spawn(move || {
            let mut code = 0;
            for mut step in steps {
                if !step.banner.is_empty() {
                    o.lock().unwrap().extend_from_slice(step.banner.as_bytes());
                }
                let started = std::time::Instant::now();
                step.cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
                step.cmd.stdin(if step.interactive { Stdio::piped() } else { Stdio::null() });
                let mut child = match step.cmd.spawn() {
                    Ok(c) => c,
                    Err(e) => {
                        o.lock().unwrap().extend_from_slice(format!("\u{1b}[31mimpossible de lancer la commande : {}\u{1b}[0m\n", e).as_bytes());
                        code = -1;
                        break;
                    }
                };
                let t1 = child.stdout.take().map(|s| pump(s, o.clone()));
                let t2 = child.stderr.take().map(|s| pump(s, o.clone()));
                *si.lock().unwrap() = child.stdin.take();
                *cs.lock().unwrap() = Some(child);
                // Wait without holding the lock, so that `kill` can take the child.
                let status = loop {
                    {
                        let mut slot = cs.lock().unwrap();
                        match slot.as_mut() {
                            Some(c) => {
                                if let Ok(Some(st)) = c.try_wait() {
                                    break Some(st);
                                }
                            }
                            None => break None,
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(25));
                };
                if let Some(t) = t1 {
                    let _ = t.join();
                }
                if let Some(t) = t2 {
                    let _ = t.join();
                }
                *si.lock().unwrap() = None;
                *cs.lock().unwrap() = None;
                code = match status {
                    Some(st) => st.code().unwrap_or(-1),
                    None => -9,
                };
                let secs = started.elapsed().as_secs_f64();
                tm.lock().unwrap().push(secs);
                if code == 0 {
                    if let Some(label) = &step.report {
                        o.lock().unwrap().extend_from_slice(format!("\u{1b}[90m{} {} s\u{1b}[0m\n", label, secs_fr(secs)).as_bytes());
                    }
                }
                if code != 0 {
                    break;
                }
            }
            for f in cleanup {
                let _ = std::fs::remove_file(f);
            }
            *d.lock().unwrap() = Some(code);
        });
        self.map.insert(id, Run { output, done, times, stdin, child: child_slot });
        id
    }

    pub fn input(&self, id: u32, text: &str) -> bool {
        let Some(run) = self.map.get(&id) else { return false };
        let mut slot = run.stdin.lock().unwrap();
        match slot.as_mut() {
            Some(s) => s.write_all(text.as_bytes()).and_then(|_| s.flush()).is_ok(),
            None => false,
        }
    }

    pub fn kill(&self, id: u32) {
        if let Some(run) = self.map.get(&id) {
            if let Some(mut c) = run.child.lock().unwrap().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
}

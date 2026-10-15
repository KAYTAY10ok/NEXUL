//! Processes: stopping the programs of a folder, waiting for a process, elevation of this
//! process, local time.

use super::ffi::*;
use super::from_wide;
use std::path::{Path, PathBuf};
use std::time::Duration;

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// The ids of the running processes.
fn process_ids() -> Vec<u32> {
    let snap = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) });
    if snap.0 == INVALID_HANDLE_VALUE {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut e: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut ok = unsafe { Process32FirstW(snap.0, &mut e) };
    while ok != 0 {
        out.push(e.th32ProcessID);
        ok = unsafe { Process32NextW(snap.0, &mut e) };
    }
    out
}

fn image_path(h: HANDLE) -> Option<PathBuf> {
    let mut buf = vec![0u16; 32768];
    let mut len = buf.len() as u32;
    (unsafe { QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) } != 0)
        .then(|| PathBuf::from(from_wide(&buf[..len as usize])))
}

/// `p` is `dir` or inside it (case-insensitive, `/` or `\`).
fn under(p: &Path, dir: &Path) -> bool {
    let n = |x: &Path| x.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    let (p, d) = (n(p), n(dir));
    !d.is_empty() && (p == d || p.starts_with(&format!("{d}\\")))
}

/// Stops the programs whose executable is in `dir` (or below), except this process, and
/// waits for their end (5 s each at most). Returns how many were stopped. Unlike
/// `taskkill /IM`, the copies of the same program elsewhere (another installation, a
/// build of a developer) are left alone.
pub fn kill_in(dir: &Path) -> usize {
    let me = unsafe { GetCurrentProcessId() };
    let mut stopped = 0;
    for pid in process_ids() {
        if pid == me || pid == 0 {
            continue;
        }
        let h =
            Handle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | SYNCHRONIZE, 0, pid) });
        if h.0.is_null() {
            continue; // a process of another user or of the system
        }
        if image_path(h.0).is_some_and(|p| under(&p, dir)) && unsafe { TerminateProcess(h.0, 1) } != 0 {
            unsafe { WaitForSingleObject(h.0, 5000) };
            stopped += 1;
        }
    }
    stopped
}

/// Waits for the end of the process `pid` at most `timeout`; `true` if it has ended (or
/// does not exist).
pub fn wait_exit(pid: u32, timeout: Duration) -> bool {
    let h = Handle(unsafe { OpenProcess(SYNCHRONIZE, 0, pid) });
    if h.0.is_null() {
        return true;
    }
    unsafe { WaitForSingleObject(h.0, timeout.as_millis().min(u32::MAX as u128 - 1) as u32) == WAIT_OBJECT_0 }
}

/// This process runs with the rights of an administrator (elevated).
pub fn is_elevated() -> bool {
    let mut token: HANDLE = std::ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return false;
    }
    let token = Handle(token);
    let mut elevated = 0u32;
    let mut len = 0u32;
    let ok = unsafe { GetTokenInformation(token.0, TOKEN_ELEVATION, &mut elevated as *mut u32 as *mut _, 4, &mut len) };
    ok != 0 && elevated != 0
}

/// The local date and time, `2026-10-15 14h05` (names of the backups).
pub fn local_time() -> String {
    let mut t = SYSTEMTIME::default();
    unsafe { GetLocalTime(&mut t) };
    format!("{:04}-{:02}-{:02} {:02}h{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    /// Not a test: the program started by `stopping_the_programs_of_a_folder` (a copy of
    /// this test executable) sleeps here.
    #[test]
    #[ignore]
    fn sleeper() {
        if std::env::var_os("NEXUL_TEST_SLEEPER").is_some() {
            std::thread::sleep(Duration::from_secs(60));
        }
    }

    fn start_sleeper(dir: &Path) -> std::process::Child {
        std::fs::create_dir_all(dir).unwrap();
        let exe = dir.join("NEXUL Studio.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        Command::new(&exe)
            .args(["--exact", "win::process::tests::sleeper", "--ignored", "--test-threads=1"])
            .env("NEXUL_TEST_SLEEPER", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }

    #[test]
    fn stopping_the_programs_of_a_folder() {
        let base = std::env::temp_dir().join(format!("nexul-kill-test-{}", std::process::id()));
        let (a, b) = (base.join("NEXUL"), base.join("NEXUL 2"));
        let mut ca = start_sleeper(&a);
        let mut cb = start_sleeper(&b);
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(kill_in(&a), 1);
        assert_eq!(ca.wait().unwrap().code(), Some(1));
        // The program of the same name in the other folder (`NEXUL 2`, a prefix of
        // nothing) still runs.
        assert!(cb.try_wait().unwrap().is_none());
        assert_eq!(kill_in(&a), 0);
        let pid = cb.id();
        assert!(!wait_exit(pid, Duration::from_millis(100)));
        assert_eq!(kill_in(&b), 1);
        assert!(wait_exit(pid, Duration::from_secs(5)));
        let _ = cb.wait();
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn paths_and_time() {
        assert!(under(Path::new(r"C:\Program Files\NEXUL\bin\nxc.exe"), Path::new(r"c:\program files\nexul\")));
        assert!(under(Path::new(r"C:\a"), Path::new(r"C:\a")));
        assert!(!under(Path::new(r"C:\NEXUL 2\x.exe"), Path::new(r"C:\NEXUL")));
        assert!(!under(Path::new(r"C:\x"), Path::new("")));
        let t = local_time();
        assert_eq!(t.len(), 16, "{t}");
        assert_eq!((&t[4..5], &t[7..8], &t[10..11], &t[13..14]), ("-", "-", " ", "h"));
        let _ = is_elevated();
        assert!(wait_exit(u32::MAX - 3, Duration::from_millis(10)));
    }
}

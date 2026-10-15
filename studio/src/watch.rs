//! Watching the files of the workspace: the explorer, the open editors and the web
//! preview follow the changes made outside of Studio.
//!
//! On Windows, `ReadDirectoryChangesW` reports the changes of the whole tree as they
//! happen; elsewhere, the tree is compared every second. Changes inside ignored folders
//! (`.git`, `target`, `node_modules`…) are not reported.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Folders whose changes do not matter to the interface.
pub const IGNORED: &[&str] = &[".git", "target", "node_modules", ".nxc-cache", ".vs", ".idea", "__pycache__"];

/// A watch in progress; it stops when dropped.
pub struct Watcher {
    stop: Arc<AtomicBool>,
    #[cfg(windows)]
    handle: Arc<std::sync::Mutex<isize>>,
}

/// `path` (relative to the root, with `/`) is inside an ignored folder.
pub fn is_ignored(rel: &str) -> bool {
    rel.split('/').any(|c| IGNORED.contains(&c))
}

impl Watcher {
    /// Calls `on_change(relative paths)` from a background thread for each batch of
    /// changes under `root`. An empty list means "something changed, rescan".
    pub fn start(root: &Path, on_change: impl Fn(Vec<String>) + Send + 'static) -> std::io::Result<Watcher> {
        let stop = Arc::new(AtomicBool::new(false));
        imp::start(root.to_path_buf(), stop.clone(), Box::new(on_change)).map(|_h| Watcher {
            stop,
            #[cfg(windows)]
            handle: _h,
        })
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        #[cfg(windows)]
        imp::cancel(&self.handle);
    }
}

type Callback = Box<dyn Fn(Vec<String>) + Send>;

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::Mutex;

    const FILE_LIST_DIRECTORY: u32 = 0x0001;
    const FILE_SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
    const OPEN_EXISTING: u32 = 3;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const INVALID_HANDLE_VALUE: isize = -1;
    // File and folder names, last write, size.
    const NOTIFY: u32 = 0x1 | 0x2 | 0x8 | 0x10;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileW(name: *const u16, access: u32, share: u32, security: *mut std::ffi::c_void, disposition: u32, flags: u32, template: isize) -> isize;
        fn ReadDirectoryChangesW(dir: isize, buffer: *mut u8, len: u32, subtree: i32, filter: u32, returned: *mut u32, overlapped: *mut std::ffi::c_void, routine: *mut std::ffi::c_void) -> i32;
        fn CancelIoEx(handle: isize, overlapped: *mut std::ffi::c_void) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }

    pub fn start(root: PathBuf, stop: Arc<AtomicBool>, on_change: Callback) -> std::io::Result<Arc<Mutex<isize>>> {
        let wide: Vec<u16> = root.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let h = unsafe { CreateFileW(wide.as_ptr(), FILE_LIST_DIRECTORY, FILE_SHARE_ALL, std::ptr::null_mut(), OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, 0) };
        if h == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }
        let handle = Arc::new(Mutex::new(h));
        let shared = handle.clone();
        std::thread::spawn(move || {
            // DWORD-aligned buffer.
            let mut buf = vec![0u32; 16 * 1024];
            loop {
                let mut returned = 0u32;
                let ok = unsafe { ReadDirectoryChangesW(h, buf.as_mut_ptr() as *mut u8, (buf.len() * 4) as u32, 1, NOTIFY, &mut returned, std::ptr::null_mut(), std::ptr::null_mut()) };
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                if ok == 0 {
                    // The folder was removed, or the watch cancelled.
                    break;
                }
                if returned == 0 {
                    // Too many changes for the buffer: rescan.
                    on_change(Vec::new());
                    continue;
                }
                let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, returned as usize) };
                let mut paths = Vec::new();
                let mut off = 0usize;
                loop {
                    // FILE_NOTIFY_INFORMATION: next offset, action, name length in bytes, name.
                    let rd = |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
                    if off + 12 > bytes.len() {
                        break;
                    }
                    let next = rd(off) as usize;
                    let len = rd(off + 8) as usize;
                    let name: Vec<u16> = bytes[off + 12..(off + 12 + len).min(bytes.len())].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                    let rel = String::from_utf16_lossy(&name).replace('\\', "/");
                    if !is_ignored(&rel) && !paths.contains(&rel) {
                        paths.push(rel);
                    }
                    if next == 0 {
                        break;
                    }
                    off += next;
                }
                if !paths.is_empty() {
                    on_change(paths);
                }
            }
            let mut h = shared.lock().unwrap();
            if *h != INVALID_HANDLE_VALUE {
                unsafe { CloseHandle(*h) };
                *h = INVALID_HANDLE_VALUE;
            }
        });
        Ok(handle)
    }

    /// Ends the blocking read of the watching thread, which then closes the handle.
    pub fn cancel(handle: &Arc<Mutex<isize>>) {
        let h = *handle.lock().unwrap();
        if h != INVALID_HANDLE_VALUE {
            unsafe { CancelIoEx(h, std::ptr::null_mut()) };
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn stamp(dir: &Path, depth: usize, out: &mut u128, count: &mut usize) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if IGNORED.contains(&name.as_str()) || depth > 12 || *count > 50_000 {
                continue;
            }
            *count += 1;
            if let Ok(md) = e.metadata() {
                let t = md.modified().ok().and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0);
                *out = out.wrapping_mul(31).wrapping_add(t ^ md.len() as u128 ^ name.len() as u128);
                if md.is_dir() {
                    stamp(&e.path(), depth + 1, out, count);
                }
            }
        }
    }

    pub fn start(root: PathBuf, stop: Arc<AtomicBool>, on_change: Callback) -> std::io::Result<()> {
        std::thread::spawn(move || {
            let mut last = None;
            while !stop.load(Ordering::SeqCst) {
                let (mut s, mut n) = (0u128, 0usize);
                stamp(&root, 0, &mut s, &mut n);
                if last.is_some() && last != Some(s) {
                    on_change(Vec::new());
                }
                last = Some(s);
                std::thread::sleep(Duration::from_secs(1));
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn reports_changes() {
        let root = std::env::temp_dir().join(format!("nexul-studio-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("target")).unwrap();
        let (tx, rx) = mpsc::channel();
        let w = Watcher::start(&root, move |p| {
            let _ = tx.send(p);
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(root.join("target").join("ignored.o"), b"x").unwrap();
        std::fs::write(root.join("a.nx"), b"x").unwrap();
        let mut seen = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline && !seen.iter().any(|p: &String| p == "a.nx") {
            if let Ok(batch) = rx.recv_timeout(Duration::from_millis(500)) {
                if batch.is_empty() && cfg!(not(windows)) {
                    seen.push("a.nx".to_string());
                }
                seen.extend(batch);
            }
        }
        assert!(seen.iter().any(|p| p == "a.nx"), "{:?}", seen);
        assert!(!seen.iter().any(|p| p.starts_with("target")), "{:?}", seen);
        drop(w);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ignored_paths() {
        assert!(is_ignored("target/debug/x"));
        assert!(is_ignored("a/.git/HEAD"));
        assert!(!is_ignored("src/targets.nx"));
    }
}

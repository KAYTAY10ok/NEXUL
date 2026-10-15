//! The operations of the installer on Windows, through the API of the system: registry,
//! environment variables, shortcuts, known folders, folder picker, elevation, processes.
//! They replace PowerShell, `reg.exe` and `taskkill`: no program is started for them, and
//! nothing depends on the execution policy of scripts or on the language mode.
//!
//! With the `test-sandbox` feature (tests only), the variable `NEXUL_SETUP_SANDBOX` (a
//! folder) redirects the registry and the known folders into a sandbox: see [`sandbox`].

pub mod env;
pub mod ffi;
pub mod process;
pub mod reg;
pub mod shell;

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

/// A string as UTF-16 with a final zero, for the API.
pub fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

/// A UTF-16 buffer up to its first zero.
pub fn from_wide(buf: &[u16]) -> String {
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}

/// A zero-terminated UTF-16 string allocated by the system.
///
/// # Safety
/// `p` is null or points to a zero-terminated UTF-16 string.
pub unsafe fn from_wide_ptr(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    while *p.add(n) != 0 {
        n += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
}

/// COM initialized on this thread for the lifetime of the value (single-threaded
/// apartment, as the shell objects want).
pub struct Com {
    owned: bool,
}

impl Com {
    pub fn init() -> Com {
        let hr = unsafe {
            ffi::CoInitializeEx(std::ptr::null_mut(), ffi::COINIT_APARTMENTTHREADED | ffi::COINIT_DISABLE_OLE1DDE)
        };
        // S_FALSE: already initialized here (still to be balanced); an error such as
        // RPC_E_CHANGED_MODE: initialized differently by someone else (not ours to end).
        Com { owned: hr == ffi::S_OK || hr == ffi::S_FALSE }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ffi::CoUninitialize() };
        }
    }
}

/// The message of a Windows error code.
pub fn error_text(code: u32) -> String {
    std::io::Error::from_raw_os_error(code as i32).to_string()
}

/// The sandbox of the tests, if any: `NEXUL_SETUP_SANDBOX`, only in builds with the
/// `test-sandbox` feature. The registry is then redirected under
/// `HKCU\Software\NEXUL-Tests\<name of the folder>\` and the known folders (Desktop, Start
/// menu) into the folder: the real settings of the user are never touched (rule 8 of
/// `docs/refonte/AGENTS.md`).
pub fn sandbox() -> Option<PathBuf> {
    #[cfg(feature = "test-sandbox")]
    {
        std::env::var_os("NEXUL_SETUP_SANDBOX").filter(|v| !v.is_empty()).map(PathBuf::from)
    }
    #[cfg(not(feature = "test-sandbox"))]
    {
        None
    }
}

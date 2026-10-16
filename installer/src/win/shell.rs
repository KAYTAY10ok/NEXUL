//! Shortcuts, known folders, the folder picker, the notification of a change of file
//! association, and starting a program with the rights of an administrator.

use super::ffi::*;
use super::{error_text, from_wide, from_wide_ptr, sandbox, wide, Com};
use std::ffi::c_void;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

/// A COM interface pointer, released when dropped.
pub(crate) struct ComPtr<V> {
    pub(crate) p: *mut c_void,
    _vtbl: PhantomData<V>,
}

impl<V> ComPtr<V> {
    /// # Safety
    /// `p` is an interface pointer whose table starts like `V`, and its reference is ours.
    pub(crate) unsafe fn from_raw(p: *mut c_void) -> Option<ComPtr<V>> {
        (!p.is_null()).then_some(ComPtr { p, _vtbl: PhantomData })
    }

    pub(crate) fn vtbl(&self) -> &V {
        unsafe { &**(self.p as *mut *const V) }
    }

    fn query<W>(&self, iid: &GUID) -> Result<ComPtr<W>, String> {
        let mut out = std::ptr::null_mut();
        let unknown = unsafe { &**(self.p as *mut *const IUnknownVtbl) };
        hr(unsafe { (unknown.QueryInterface)(self.p, iid, &mut out) })?;
        unsafe { ComPtr::from_raw(out) }.ok_or_else(|| "interface absente".into())
    }
}

impl<V> Drop for ComPtr<V> {
    fn drop(&mut self) {
        unsafe {
            let unknown = &**(self.p as *mut *const IUnknownVtbl);
            (unknown.Release)(self.p);
        }
    }
}

fn hr(code: HRESULT) -> Result<(), String> {
    if code >= 0 {
        Ok(())
    } else {
        Err(format!("erreur 0x{:08X}", code as u32))
    }
}

pub(crate) fn create<V>(clsid: &GUID, iid: &GUID) -> Result<ComPtr<V>, String> {
    let mut out = std::ptr::null_mut();
    hr(unsafe { CoCreateInstance(clsid, std::ptr::null_mut(), CLSCTX_INPROC_SERVER, iid, &mut out) })?;
    unsafe { ComPtr::from_raw(out) }.ok_or_else(|| "objet COM absent".into())
}

/// The folders where shortcuts go.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Folder {
    /// The Desktop of the user.
    Desktop,
    /// The Desktop of all users.
    PublicDesktop,
    /// The programs of the Start menu of the user.
    Programs,
    /// The programs of the Start menu of all users.
    CommonPrograms,
}

impl Folder {
    pub const ALL: [Folder; 4] = [Folder::Desktop, Folder::PublicDesktop, Folder::Programs, Folder::CommonPrograms];

    fn id(self) -> &'static GUID {
        match self {
            Folder::Desktop => &FOLDERID_DESKTOP,
            Folder::PublicDesktop => &FOLDERID_PUBLIC_DESKTOP,
            Folder::Programs => &FOLDERID_PROGRAMS,
            Folder::CommonPrograms => &FOLDERID_COMMON_PROGRAMS,
        }
    }

    fn sandbox_name(self) -> &'static str {
        match self {
            Folder::Desktop => "Desktop",
            Folder::PublicDesktop => "PublicDesktop",
            Folder::Programs => "Programs",
            Folder::CommonPrograms => "CommonPrograms",
        }
    }
}

/// The path of a known folder (in the sandbox of the tests: a folder inside it, created).
pub fn known_folder(f: Folder) -> Option<PathBuf> {
    if let Some(sb) = sandbox() {
        let p = sb.join(f.sandbox_name());
        let _ = std::fs::create_dir_all(&p);
        return Some(p);
    }
    let mut p: *mut u16 = std::ptr::null_mut();
    let r = unsafe { SHGetKnownFolderPath(f.id(), 0, std::ptr::null_mut(), &mut p) };
    let path = unsafe { from_wide_ptr(p) };
    unsafe { CoTaskMemFree(p as *mut c_void) };
    (r == S_OK && !path.is_empty()).then(|| PathBuf::from(path))
}

/// What a shortcut contains.
pub struct Shortcut<'a> {
    pub target: &'a Path,
    pub arguments: &'a str,
    pub working_dir: &'a Path,
    pub icon: &'a Path,
    pub icon_index: i32,
    pub description: &'a str,
}

/// Creates (or replaces) the shortcut `lnk`.
pub fn create_shortcut(lnk: &Path, s: &Shortcut) -> Result<(), String> {
    let _com = Com::init();
    let fail = |e: String| format!("raccourci {} : {}", lnk.display(), e);
    let link: ComPtr<IShellLinkWVtbl> = create(&CLSID_SHELL_LINK, &IID_ISHELL_LINK_W).map_err(fail)?;
    let vt = link.vtbl();
    unsafe {
        hr((vt.SetPath)(link.p, wide(s.target).as_ptr())).map_err(fail)?;
        hr((vt.SetArguments)(link.p, wide(s.arguments).as_ptr())).map_err(fail)?;
        hr((vt.SetWorkingDirectory)(link.p, wide(s.working_dir).as_ptr())).map_err(fail)?;
        hr((vt.SetIconLocation)(link.p, wide(s.icon).as_ptr(), s.icon_index)).map_err(fail)?;
        hr((vt.SetDescription)(link.p, wide(s.description).as_ptr())).map_err(fail)?;
    }
    let file: ComPtr<IPersistFileVtbl> = link.query(&IID_IPERSIST_FILE).map_err(fail)?;
    hr(unsafe { (file.vtbl().Save)(file.p, wide(lnk).as_ptr(), 1) }).map_err(fail)
}

/// The target and the arguments of a shortcut (`None` if it cannot be read; an empty
/// target for a shortcut that does not designate a file).
pub fn read_shortcut(lnk: &Path) -> Option<(String, String)> {
    let _com = Com::init();
    let link: ComPtr<IShellLinkWVtbl> = create(&CLSID_SHELL_LINK, &IID_ISHELL_LINK_W).ok()?;
    let file: ComPtr<IPersistFileVtbl> = link.query(&IID_IPERSIST_FILE).ok()?;
    hr(unsafe { (file.vtbl().Load)(file.p, wide(lnk).as_ptr(), STGM_READ) }).ok()?;
    let mut target = vec![0u16; 32768];
    let mut args = vec![0u16; 32768];
    unsafe {
        hr((link.vtbl().GetPath)(link.p, target.as_mut_ptr(), target.len() as i32, std::ptr::null_mut(), 0)).ok()?;
        hr((link.vtbl().GetArguments)(link.p, args.as_mut_ptr(), args.len() as i32)).ok()?;
    }
    Some((from_wide(&target), from_wide(&args)))
}

/// Expands 8.3 path components when the target exists. Returns None if Windows cannot
/// resolve the path; callers can then use the original spelling.
pub fn long_path(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().to_string_lossy().contains('\0') {
        return None;
    }
    let input = wide(path);
    let needed = unsafe { GetLongPathNameW(input.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return None;
    }
    let mut buffer = vec![0u16; needed as usize];
    let written = unsafe { GetLongPathNameW(input.as_ptr(), buffer.as_mut_ptr(), needed) };
    if written == 0 || written >= needed {
        return None;
    }
    buffer.truncate(written as usize);
    Some(PathBuf::from(from_wide(&buffer)))
}

/// Tells the shell that file associations changed (icons, "open with"). Not in the
/// sandbox of the tests.
pub fn notify_associations() {
    if sandbox().is_none() {
        unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, std::ptr::null(), std::ptr::null()) };
    }
}

/// An invisible window, always on top and centered on the screen, owner of the folder
/// picker: the picker opens above the window of the installer rather than behind it.
struct Owner(HWND);

impl Owner {
    fn new() -> Owner {
        unsafe {
            let (w, h) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
            Owner(CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                wide("STATIC").as_ptr(),
                wide("NEXUL").as_ptr(),
                WS_POPUP,
                w / 2,
                h / 2,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null_mut(),
            ))
        }
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { DestroyWindow(self.0) };
        }
    }
}

/// The folder picker of Windows (the one of the Explorer), opened on `start` or on its
/// nearest existing parent. `None` if the user cancels.
pub fn pick_folder(title: &str, start: &Path) -> Option<PathBuf> {
    pick_folder_for(std::ptr::null_mut(), title, start)
}

/// [`pick_folder`] owned by the window `owner` (the wizard), or, if it is null, by an
/// invisible window kept on top.
pub fn pick_folder_for(owner: HWND, title: &str, start: &Path) -> Option<PathBuf> {
    let _com = Com::init();
    let dialog: ComPtr<IFileOpenDialogVtbl> = create(&CLSID_FILE_OPEN_DIALOG, &IID_IFILE_OPEN_DIALOG).ok()?;
    let vt = dialog.vtbl();
    unsafe {
        let mut options = 0u32;
        hr((vt.GetOptions)(dialog.p, &mut options)).ok()?;
        hr((vt.SetOptions)(dialog.p, options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)).ok()?;
        hr((vt.SetTitle)(dialog.p, wide(title).as_ptr())).ok()?;
        if let Some(dir) = start.ancestors().find(|a| a.is_dir()) {
            let mut item = std::ptr::null_mut();
            if SHCreateItemFromParsingName(wide(dir).as_ptr(), std::ptr::null_mut(), &IID_ISHELL_ITEM, &mut item)
                == S_OK
            {
                if let Some(item) = ComPtr::<IShellItemVtbl>::from_raw(item) {
                    let _ = (vt.SetFolder)(dialog.p, item.p);
                }
            }
        }
        let fallback = if owner.is_null() { Some(Owner::new()) } else { None };
        let owner = fallback.as_ref().map_or(owner, |o| o.0);
        if (vt.Show)(dialog.p, owner) != S_OK {
            return None; // cancelled (or failed)
        }
        let mut result = std::ptr::null_mut();
        hr((vt.GetResult)(dialog.p, &mut result)).ok()?;
        let item = ComPtr::<IShellItemVtbl>::from_raw(result)?;
        let mut name: *mut u16 = std::ptr::null_mut();
        hr((item.vtbl().GetDisplayName)(item.p, SIGDN_FILESYSPATH, &mut name)).ok()?;
        let path = from_wide_ptr(name);
        CoTaskMemFree(name as *mut c_void);
        (!path.is_empty()).then(|| PathBuf::from(path))
    }
}

/// Starts `program` with `parameters` through the shell (`verb` `runas`: with the rights
/// of an administrator, after the confirmation of Windows), without window, waits for its
/// end and returns its exit code. `Err` holds the Windows error code (`ERROR_CANCELLED`
/// when the user refuses the elevation).
pub fn run_and_wait(verb: &str, program: &Path, parameters: &str) -> Result<u32, u32> {
    let _com = Com::init();
    let (verb, file, params) = (wide(verb), wide(program), wide(parameters));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        hwnd: std::ptr::null_mut(),
        lpVerb: verb.as_ptr(),
        lpFile: file.as_ptr(),
        lpParameters: params.as_ptr(),
        lpDirectory: std::ptr::null(),
        nShow: SW_HIDE,
        hInstApp: std::ptr::null_mut(),
        lpIDList: std::ptr::null_mut(),
        lpClass: std::ptr::null(),
        hkeyClass: 0,
        dwHotKey: 0,
        hIconOrMonitor: std::ptr::null_mut(),
        hProcess: std::ptr::null_mut(),
    };
    unsafe {
        if ShellExecuteExW(&mut info) == 0 {
            return Err(GetLastError());
        }
        if info.hProcess.is_null() {
            return Ok(0);
        }
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 0u32;
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
        Ok(code)
    }
}

/// The text of an error of [`run_and_wait`].
pub fn run_error_text(code: u32) -> String {
    error_text(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nexul-shell-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn known_folders() {
        for f in Folder::ALL {
            let p = known_folder(f).unwrap_or_else(|| panic!("{f:?}"));
            assert!(p.is_absolute(), "{f:?} {}", p.display());
        }
        let desktop = known_folder(Folder::Desktop).unwrap();
        assert!(desktop.is_dir());
        assert!(known_folder(Folder::Programs).unwrap().ends_with(r"Start Menu\Programs"));
    }

    #[test]
    fn shortcuts_round_trip() {
        let d = temp("lnk");
        let target_dir = d.join("NEXUL été 😀").join("bin");
        std::fs::create_dir_all(&target_dir).unwrap();
        let target = target_dir.join("NEXUL Studio.exe");
        std::fs::write(&target, b"MZ").unwrap();
        let lnk = d.join("NEXUL Studio.lnk");
        let s = Shortcut {
            target: &target,
            arguments: r#""C:\projets\a b""#,
            working_dir: &d,
            icon: &target,
            icon_index: 0,
            description: "NEXUL Studio — l'environnement de développement",
        };
        create_shortcut(&lnk, &s).unwrap();
        let (t, a) = read_shortcut(&lnk).unwrap();
        assert_eq!(PathBuf::from(t).canonicalize().unwrap(), target.canonicalize().unwrap());
        assert_eq!(a, r#""C:\projets\a b""#);
        // Replacing an existing shortcut.
        let other = target_dir.join("nxc.exe");
        std::fs::write(&other, b"MZ").unwrap();
        create_shortcut(&lnk, &Shortcut { target: &other, arguments: "", ..s }).unwrap();
        let (actual, arguments) = read_shortcut(&lnk).unwrap();
        assert_eq!(PathBuf::from(actual).canonicalize().unwrap(), other.canonicalize().unwrap());
        assert_eq!(arguments, "");
        assert_eq!(read_shortcut(&d.join("absent.lnk")), None);
        assert!(create_shortcut(&d.join("absent").join("x.lnk"), &s).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn running_and_waiting() {
        let cmd = PathBuf::from(std::env::var("ComSpec").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".into()));
        assert_eq!(run_and_wait("open", &cmd, "/c exit 7"), Ok(7));
        assert_eq!(run_and_wait("open", &cmd, "/c exit 0"), Ok(0));
        assert!(run_and_wait("open", Path::new(r"C:\nexul-absent\x.exe"), "").is_err());
        notify_associations();
    }

    /// Opens the picker on the user's screen for a moment, then answers it by messages:
    /// run with `cargo test -p nexul-installer -- --ignored picker`.
    #[test]
    #[ignore]
    fn picker() {
        #[link(name = "user32")]
        extern "system" {
            fn FindWindowW(class: *const u16, name: *const u16) -> HWND;
            fn PostMessageW(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> BOOL;
        }
        const WM_COMMAND: u32 = 0x0111;
        let d = temp("picker");
        let answer = |id: usize, title: &'static str| {
            std::thread::spawn(move || {
                for _ in 0..200 {
                    let w = unsafe { FindWindowW(wide("#32770").as_ptr(), wide(title).as_ptr()) };
                    if !w.is_null() {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        unsafe { PostMessageW(w, WM_COMMAND, id, 0) };
                        return true;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                false
            })
        };
        // OK: the current folder (the start folder, or its nearest existing parent).
        let t = answer(1, "Test NEXUL A");
        assert_eq!(pick_folder("Test NEXUL A", &d.join("pas encore").join("NEXUL")), Some(d.clone()));
        assert!(t.join().unwrap());
        // Cancel.
        let t = answer(2, "Test NEXUL B");
        assert_eq!(pick_folder("Test NEXUL B", &d), None);
        assert!(t.join().unwrap());
        let _ = std::fs::remove_dir_all(&d);
    }
}

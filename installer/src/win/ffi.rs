//! Declarations of the Windows functions, structures and COM interfaces the installer
//! uses, linked to the system libraries (no crate). Only what is called is declared; the
//! COM tables list every method up to the last one used, in the order of the SDK headers.

#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use std::ffi::c_void;

pub type HANDLE = *mut c_void;
pub type HWND = *mut c_void;
pub type HKEY = isize;
pub type HRESULT = i32;
pub type BOOL = i32;

pub const HKEY_CURRENT_USER: HKEY = 0x8000_0001u32 as i32 as isize;
pub const HKEY_LOCAL_MACHINE: HKEY = 0x8000_0002u32 as i32 as isize;
pub const KEY_READ: u32 = 0x2_0019;
pub const KEY_ALL_ACCESS: u32 = 0xF_003F;
pub const REG_SZ: u32 = 1;
pub const REG_EXPAND_SZ: u32 = 2;
pub const REG_DWORD: u32 = 4;
pub const ERROR_SUCCESS: i32 = 0;
pub const ERROR_FILE_NOT_FOUND: i32 = 2;
pub const ERROR_MORE_DATA: i32 = 234;
pub const ERROR_CANCELLED: u32 = 1223;

pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
pub const PROCESS_TERMINATE: u32 = 0x0001;
pub const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
pub const SYNCHRONIZE: u32 = 0x0010_0000;
pub const TH32CS_SNAPPROCESS: u32 = 0x2;
pub const WAIT_OBJECT_0: u32 = 0;
pub const INFINITE: u32 = 0xFFFF_FFFF;
pub const TOKEN_QUERY: u32 = 0x8;
pub const TOKEN_ELEVATION: u32 = 20;

pub const HWND_BROADCAST: HWND = 0xFFFF as HWND;
pub const WM_SETTINGCHANGE: u32 = 0x001A;
pub const SMTO_ABORTIFHUNG: u32 = 0x0002;
pub const WS_POPUP: u32 = 0x8000_0000;
pub const WS_EX_TOPMOST: u32 = 0x0000_0008;
pub const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
pub const SM_CXSCREEN: i32 = 0;
pub const SM_CYSCREEN: i32 = 1;

pub const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
pub const SHCNF_IDLIST: u32 = 0;
pub const SEE_MASK_NOCLOSEPROCESS: u32 = 0x0040;
pub const SEE_MASK_NOASYNC: u32 = 0x0100;
/// No error message box: the error is returned (the elevation prompt still appears).
pub const SEE_MASK_FLAG_NO_UI: u32 = 0x0400;
pub const SW_HIDE: i32 = 0;

pub const COINIT_APARTMENTTHREADED: u32 = 0x2;
pub const COINIT_DISABLE_OLE1DDE: u32 = 0x4;
pub const CLSCTX_INPROC_SERVER: u32 = 0x1;
pub const S_OK: HRESULT = 0;
pub const S_FALSE: HRESULT = 1;
pub const STGM_READ: u32 = 0;
pub const FOS_PICKFOLDERS: u32 = 0x20;
pub const FOS_FORCEFILESYSTEM: u32 = 0x40;
pub const FOS_PATHMUSTEXIST: u32 = 0x800;
pub const SIGDN_FILESYSPATH: u32 = 0x8005_8000;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

const fn guid(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> GUID {
    GUID { data1, data2, data3, data4 }
}

pub const CLSID_SHELL_LINK: GUID = guid(0x0002_1401, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
pub const IID_ISHELL_LINK_W: GUID = guid(0x0002_14F9, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
pub const IID_IPERSIST_FILE: GUID = guid(0x0000_010B, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
pub const CLSID_FILE_OPEN_DIALOG: GUID =
    guid(0xDC1C_5A9C, 0xE88A, 0x4DDE, [0xA5, 0xA1, 0x60, 0xF8, 0x2A, 0x20, 0xAE, 0xF7]);
pub const IID_IFILE_OPEN_DIALOG: GUID =
    guid(0xD57C_7288, 0xD4AD, 0x4768, [0xBE, 0x02, 0x9D, 0x96, 0x95, 0x32, 0xD9, 0x60]);
pub const IID_ISHELL_ITEM: GUID = guid(0x4382_6D1E, 0xE718, 0x42EE, [0xBC, 0x55, 0xA1, 0xE2, 0x61, 0xC3, 0x7B, 0xFE]);
pub const FOLDERID_DESKTOP: GUID = guid(0xB4BF_CC3A, 0xDB2C, 0x424C, [0xB0, 0x29, 0x7F, 0xE9, 0x9A, 0x87, 0xC6, 0x41]);
pub const FOLDERID_PUBLIC_DESKTOP: GUID =
    guid(0xC4AA_340D, 0xF20F, 0x4863, [0xAF, 0xEF, 0xF8, 0x7E, 0xF2, 0xE6, 0xBA, 0x25]);
pub const FOLDERID_PROGRAMS: GUID = guid(0xA77F_5D77, 0x2E2B, 0x44C3, [0xA6, 0xA2, 0xAB, 0xA6, 0x01, 0x05, 0x4A, 0x51]);
pub const FOLDERID_COMMON_PROGRAMS: GUID =
    guid(0x0139_D44E, 0x6AFE, 0x49F2, [0x86, 0x90, 0x3D, 0xAF, 0xCA, 0xE6, 0xFF, 0xB8]);

#[repr(C)]
pub struct PROCESSENTRY32W {
    pub dwSize: u32,
    pub cntUsage: u32,
    pub th32ProcessID: u32,
    pub th32DefaultHeapID: usize,
    pub th32ModuleID: u32,
    pub cntThreads: u32,
    pub th32ParentProcessID: u32,
    pub pcPriClassBase: i32,
    pub dwFlags: u32,
    pub szExeFile: [u16; 260],
}

#[repr(C)]
#[derive(Default)]
pub struct SYSTEMTIME {
    pub wYear: u16,
    pub wMonth: u16,
    pub wDayOfWeek: u16,
    pub wDay: u16,
    pub wHour: u16,
    pub wMinute: u16,
    pub wSecond: u16,
    pub wMilliseconds: u16,
}

#[repr(C)]
pub struct SHELLEXECUTEINFOW {
    pub cbSize: u32,
    pub fMask: u32,
    pub hwnd: HWND,
    pub lpVerb: *const u16,
    pub lpFile: *const u16,
    pub lpParameters: *const u16,
    pub lpDirectory: *const u16,
    pub nShow: i32,
    pub hInstApp: HANDLE,
    pub lpIDList: *mut c_void,
    pub lpClass: *const u16,
    pub hkeyClass: HKEY,
    pub dwHotKey: u32,
    pub hIconOrMonitor: HANDLE,
    pub hProcess: HANDLE,
}

#[link(name = "advapi32")]
extern "system" {
    pub fn RegCreateKeyExW(
        key: HKEY,
        subkey: *const u16,
        reserved: u32,
        class: *mut u16,
        options: u32,
        sam: u32,
        security: *const c_void,
        result: *mut HKEY,
        disposition: *mut u32,
    ) -> i32;
    pub fn RegOpenKeyExW(key: HKEY, subkey: *const u16, options: u32, sam: u32, result: *mut HKEY) -> i32;
    pub fn RegSetValueExW(key: HKEY, name: *const u16, reserved: u32, ty: u32, data: *const u8, len: u32) -> i32;
    pub fn RegQueryValueExW(
        key: HKEY,
        name: *const u16,
        reserved: *mut u32,
        ty: *mut u32,
        data: *mut u8,
        len: *mut u32,
    ) -> i32;
    pub fn RegDeleteValueW(key: HKEY, name: *const u16) -> i32;
    pub fn RegDeleteTreeW(key: HKEY, subkey: *const u16) -> i32;
    pub fn RegDeleteKeyW(key: HKEY, subkey: *const u16) -> i32;
    pub fn RegEnumKeyExW(
        key: HKEY,
        index: u32,
        name: *mut u16,
        name_len: *mut u32,
        reserved: *mut u32,
        class: *mut u16,
        class_len: *mut u32,
        last_write: *mut c_void,
    ) -> i32;
    pub fn RegCloseKey(key: HKEY) -> i32;
    pub fn OpenProcessToken(process: HANDLE, access: u32, token: *mut HANDLE) -> BOOL;
    pub fn GetTokenInformation(token: HANDLE, class: u32, info: *mut c_void, len: u32, ret: *mut u32) -> BOOL;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn GetCurrentProcessId() -> u32;
    pub fn CloseHandle(h: HANDLE) -> BOOL;
    pub fn OpenProcess(access: u32, inherit: BOOL, pid: u32) -> HANDLE;
    pub fn QueryFullProcessImageNameW(process: HANDLE, flags: u32, name: *mut u16, size: *mut u32) -> BOOL;
    pub fn TerminateProcess(process: HANDLE, code: u32) -> BOOL;
    pub fn WaitForSingleObject(h: HANDLE, ms: u32) -> u32;
    pub fn GetExitCodeProcess(process: HANDLE, code: *mut u32) -> BOOL;
    pub fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> HANDLE;
    pub fn Process32FirstW(snapshot: HANDLE, entry: *mut PROCESSENTRY32W) -> BOOL;
    pub fn Process32NextW(snapshot: HANDLE, entry: *mut PROCESSENTRY32W) -> BOOL;
    pub fn ExpandEnvironmentStringsW(src: *const u16, dst: *mut u16, size: u32) -> u32;
    pub fn GetLocalTime(time: *mut SYSTEMTIME);
    pub fn GetLastError() -> u32;
    pub fn GetModuleHandleW(name: *const u16) -> HANDLE;
}

#[link(name = "user32")]
extern "system" {
    pub fn SendMessageTimeoutW(
        hwnd: HWND,
        msg: u32,
        wparam: usize,
        lparam: isize,
        flags: u32,
        timeout: u32,
        result: *mut usize,
    ) -> isize;
    pub fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: HWND,
        menu: HANDLE,
        instance: HANDLE,
        param: *mut c_void,
    ) -> HWND;
    pub fn DestroyWindow(hwnd: HWND) -> BOOL;
    pub fn GetSystemMetrics(index: i32) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    pub fn SHGetKnownFolderPath(id: *const GUID, flags: u32, token: HANDLE, path: *mut *mut u16) -> HRESULT;
    pub fn SHChangeNotify(event: i32, flags: u32, item1: *const c_void, item2: *const c_void);
    pub fn SHCreateItemFromParsingName(
        path: *const u16,
        ctx: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT;
    pub fn ShellExecuteExW(info: *mut SHELLEXECUTEINFOW) -> BOOL;
}

#[link(name = "ole32")]
extern "system" {
    pub fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> HRESULT;
    pub fn CoUninitialize();
    pub fn CoCreateInstance(
        clsid: *const GUID,
        outer: *mut c_void,
        context: u32,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT;
    pub fn CoTaskMemFree(p: *mut c_void);
}

type Method = usize; // a method not called: only its place in the table matters

#[repr(C)]
pub struct IUnknownVtbl {
    pub QueryInterface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(*mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(*mut c_void) -> u32,
}

/// `IShellLinkW` (ShObjIdl_core.h).
#[repr(C)]
pub struct IShellLinkWVtbl {
    pub base: IUnknownVtbl,
    pub GetPath: unsafe extern "system" fn(*mut c_void, *mut u16, i32, *mut c_void, u32) -> HRESULT,
    pub GetIDList: Method,
    pub SetIDList: Method,
    pub GetDescription: Method,
    pub SetDescription: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    pub GetWorkingDirectory: Method,
    pub SetWorkingDirectory: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    pub GetArguments: unsafe extern "system" fn(*mut c_void, *mut u16, i32) -> HRESULT,
    pub SetArguments: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    pub GetHotkey: Method,
    pub SetHotkey: Method,
    pub GetShowCmd: Method,
    pub SetShowCmd: Method,
    pub GetIconLocation: Method,
    pub SetIconLocation: unsafe extern "system" fn(*mut c_void, *const u16, i32) -> HRESULT,
    pub SetRelativePath: Method,
    pub Resolve: Method,
    pub SetPath: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
}

/// `IPersistFile` (ObjIdl.h): `IPersist::GetClassID`, then its own methods.
#[repr(C)]
pub struct IPersistFileVtbl {
    pub base: IUnknownVtbl,
    pub GetClassID: Method,
    pub IsDirty: Method,
    pub Load: unsafe extern "system" fn(*mut c_void, *const u16, u32) -> HRESULT,
    pub Save: unsafe extern "system" fn(*mut c_void, *const u16, BOOL) -> HRESULT,
}

/// `IFileOpenDialog` (ShObjIdl_core.h): `IModalWindow::Show`, then `IFileDialog` up to
/// `GetResult`.
#[repr(C)]
pub struct IFileOpenDialogVtbl {
    pub base: IUnknownVtbl,
    pub Show: unsafe extern "system" fn(*mut c_void, HWND) -> HRESULT,
    pub SetFileTypes: Method,
    pub SetFileTypeIndex: Method,
    pub GetFileTypeIndex: Method,
    pub Advise: Method,
    pub Unadvise: Method,
    pub SetOptions: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    pub GetOptions: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    pub SetDefaultFolder: Method,
    pub SetFolder: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    pub GetFolder: Method,
    pub GetCurrentSelection: Method,
    pub SetFileName: Method,
    pub GetFileName: Method,
    pub SetTitle: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    pub SetOkButtonLabel: Method,
    pub SetFileNameLabel: Method,
    pub GetResult: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

/// `IShellItem` (ShObjIdl_core.h).
#[repr(C)]
pub struct IShellItemVtbl {
    pub base: IUnknownVtbl,
    pub BindToHandler: Method,
    pub GetParent: Method,
    pub GetDisplayName: unsafe extern "system" fn(*mut c_void, u32, *mut *mut u16) -> HRESULT,
}

//! Declarations of the Windows functions used by the window of the installer (user32,
//! gdi32), linked to the system libraries (no crate). The functions that do not exist on
//! every supported Windows (DPI per monitor, `comctl32` version 6) are looked up at run
//! time.

#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms, dead_code)]

use crate::win::ffi::{BOOL, HANDLE, HWND};
use std::ffi::c_void;

pub type HDC = HANDLE;
pub type HFONT = HANDLE;
pub type HBRUSH = HANDLE;
pub type COLORREF = u32;
pub type WNDPROC = unsafe extern "system" fn(HWND, u32, usize, isize) -> isize;

/// `RGB(r, g, b)`.
pub const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: usize,
    pub lParam: isize,
    pub time: u32,
    pub pt: POINT,
    pub lPrivate: u32,
}

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HANDLE,
    pub hIcon: HANDLE,
    pub hCursor: HANDLE,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: HANDLE,
}

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [u8; 32],
}

#[repr(C)]
#[derive(Default)]
pub struct SCROLLINFO {
    pub cbSize: u32,
    pub fMask: u32,
    pub nMin: i32,
    pub nMax: i32,
    pub nPage: u32,
    pub nPos: i32,
    pub nTrackPos: i32,
}

#[repr(C)]
pub struct BITMAPINFOHEADER {
    pub biSize: u32,
    pub biWidth: i32,
    pub biHeight: i32,
    pub biPlanes: u16,
    pub biBitCount: u16,
    pub biCompression: u32,
    pub biSizeImage: u32,
    pub biXPelsPerMeter: i32,
    pub biYPelsPerMeter: i32,
    pub biClrUsed: u32,
    pub biClrImportant: u32,
}

#[repr(C)]
pub struct ACTCTXW {
    pub cbSize: u32,
    pub dwFlags: u32,
    pub lpSource: *const u16,
    pub wProcessorArchitecture: u16,
    pub wLangId: u16,
    pub lpAssemblyDirectory: *const u16,
    pub lpResourceName: *const u16,
    pub lpApplicationName: *const u16,
    pub hModule: HANDLE,
}

#[repr(C)]
pub struct INITCOMMONCONTROLSEX {
    pub dwSize: u32,
    pub dwICC: u32,
}

// Messages.
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_ERASEBKGND: u32 = 0x0014;
pub const WM_SETFONT: u32 = 0x0030;
pub const WM_SETICON: u32 = 0x0080;
pub const WM_COMMAND: u32 = 0x0111;
pub const WM_TIMER: u32 = 0x0113;
pub const WM_VSCROLL: u32 = 0x0115;
pub const WM_CTLCOLOREDIT: u32 = 0x0133;
pub const WM_CTLCOLORLISTBOX: u32 = 0x0134;
pub const WM_CTLCOLORBTN: u32 = 0x0135;
pub const WM_CTLCOLORSTATIC: u32 = 0x0138;
pub const WM_MOUSEWHEEL: u32 = 0x020A;
pub const WM_DPICHANGED: u32 = 0x02E0;
pub const WM_APP: u32 = 0x8000;
pub const DM_GETDEFID: u32 = 0x0400;
pub const DC_HASDEFID: u32 = 0x534B;
pub const BM_GETCHECK: u32 = 0x00F0;
pub const BM_SETCHECK: u32 = 0x00F1;
pub const BM_CLICK: u32 = 0x00F5;
pub const BN_CLICKED: u32 = 0;
pub const EN_CHANGE: u32 = 0x0300;
pub const LB_ADDSTRING: u32 = 0x0180;
pub const LB_GETCOUNT: u32 = 0x018B;
pub const LB_SETTOPINDEX: u32 = 0x0197;
pub const PBM_SETPOS: u32 = 0x0402;
pub const PBM_SETRANGE32: u32 = 0x0406;
pub const IDOK: usize = 1;
pub const IDCANCEL: usize = 2;
pub const BST_CHECKED: usize = 1;

// Styles.
pub const WS_OVERLAPPED: u32 = 0;
pub const WS_CAPTION: u32 = 0x00C0_0000;
pub const WS_SYSMENU: u32 = 0x0008_0000;
pub const WS_MINIMIZEBOX: u32 = 0x0002_0000;
pub const WS_CHILD: u32 = 0x4000_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;
pub const WS_DISABLED: u32 = 0x0800_0000;
pub const WS_CLIPSIBLINGS: u32 = 0x0400_0000;
pub const WS_CLIPCHILDREN: u32 = 0x0200_0000;
pub const WS_VSCROLL: u32 = 0x0020_0000;
pub const WS_GROUP: u32 = 0x0002_0000;
pub const WS_TABSTOP: u32 = 0x0001_0000;
pub const WS_EX_NOPARENTNOTIFY: u32 = 0x0000_0004;
pub const WS_EX_CLIENTEDGE: u32 = 0x0000_0200;
pub const WS_EX_CONTROLPARENT: u32 = 0x0001_0000;
pub const BS_PUSHBUTTON: u32 = 0x0;
pub const BS_DEFPUSHBUTTON: u32 = 0x1;
pub const BS_AUTOCHECKBOX: u32 = 0x3;
pub const BS_AUTORADIOBUTTON: u32 = 0x9;
pub const BS_MULTILINE: u32 = 0x2000;
pub const SS_LEFT: u32 = 0x0;
pub const SS_RIGHT: u32 = 0x2;
pub const SS_NOPREFIX: u32 = 0x80;
pub const SS_ETCHEDHORZ: u32 = 0x10;
pub const SS_ENDELLIPSIS: u32 = 0x4000;
pub const SS_PATHELLIPSIS: u32 = 0x8000;
pub const ES_MULTILINE: u32 = 0x4;
pub const ES_AUTOVSCROLL: u32 = 0x40;
pub const ES_AUTOHSCROLL: u32 = 0x80;
pub const ES_READONLY: u32 = 0x800;
pub const LBS_NOINTEGRALHEIGHT: u32 = 0x100;
pub const LBS_NOSEL: u32 = 0x4000;
pub const CS_VREDRAW: u32 = 0x1;
pub const CS_HREDRAW: u32 = 0x2;

pub const SW_SHOWNOACTIVATE: i32 = 4;
pub const SW_MINIMIZE: i32 = 6;
pub const SW_SHOW: i32 = 5;
pub const SWP_NOZORDER: u32 = 0x4;
pub const SWP_NOACTIVATE: u32 = 0x10;
pub const MB_OK: u32 = 0x0;
pub const MB_ICONERROR: u32 = 0x10;
pub const MB_ICONWARNING: u32 = 0x30;
pub const SB_VERT: i32 = 1;
pub const SIF_RANGE: u32 = 0x1;
pub const SIF_PAGE: u32 = 0x2;
pub const SIF_POS: u32 = 0x4;
pub const SIF_TRACKPOS: u32 = 0x10;
pub const SB_LINEUP: u32 = 0;
pub const SB_LINEDOWN: u32 = 1;
pub const SB_PAGEUP: u32 = 2;
pub const SB_PAGEDOWN: u32 = 3;
pub const SB_THUMBTRACK: u32 = 5;
pub const SB_TOP: u32 = 6;
pub const SB_BOTTOM: u32 = 7;
pub const SW_SCROLLCHILDREN: u32 = 0x1;
pub const SW_INVALIDATE: u32 = 0x2;
pub const SW_ERASE: u32 = 0x4;
pub const ICON_SMALL: usize = 0;
pub const ICON_BIG: usize = 1;
pub const IDC_ARROW: usize = 32512;

pub const DT_CENTER: u32 = 0x1;
pub const DT_VCENTER: u32 = 0x4;
pub const DT_WORDBREAK: u32 = 0x10;
pub const DT_SINGLELINE: u32 = 0x20;
pub const DT_EXPANDTABS: u32 = 0x40;
pub const DT_CALCRECT: u32 = 0x400;
pub const DT_NOPREFIX: u32 = 0x800;
pub const DT_END_ELLIPSIS: u32 = 0x8000;
pub const TRANSPARENT: i32 = 1;
pub const FW_NORMAL: i32 = 400;
pub const FW_SEMIBOLD: i32 = 600;
pub const FW_BOLD: i32 = 700;
pub const DEFAULT_CHARSET: u32 = 1;
pub const CLEARTYPE_QUALITY: u32 = 5;
pub const PS_SOLID: i32 = 0;
pub const NULL_BRUSH: i32 = 5;
pub const NULL_PEN: i32 = 8;
pub const PW_RENDERFULLCONTENT: u32 = 2;
pub const ICC_STANDARD_CLASSES: u32 = 0x4000;
pub const ICC_PROGRESS_CLASS: u32 = 0x20;
/// `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`.
pub const PER_MONITOR_AWARE_V2: isize = -4;

#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassExW(class: *const WNDCLASSEXW) -> u16;
    pub fn DefWindowProcW(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize;
    pub fn GetMessageW(msg: *mut MSG, hwnd: HWND, min: u32, max: u32) -> BOOL;
    pub fn TranslateMessage(msg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(msg: *const MSG) -> isize;
    pub fn IsDialogMessageW(hwnd: HWND, msg: *mut MSG) -> BOOL;
    pub fn PostQuitMessage(code: i32);
    pub fn PostMessageW(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> BOOL;
    pub fn SendMessageW(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize;
    pub fn SetWindowTextW(hwnd: HWND, text: *const u16) -> BOOL;
    pub fn GetWindowTextW(hwnd: HWND, text: *mut u16, max: i32) -> i32;
    pub fn GetWindowTextLengthW(hwnd: HWND) -> i32;
    pub fn ShowWindow(hwnd: HWND, cmd: i32) -> BOOL;
    pub fn IsIconic(hwnd: HWND) -> BOOL;
    pub fn UpdateWindow(hwnd: HWND) -> BOOL;
    pub fn SetWindowPos(hwnd: HWND, after: HWND, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> BOOL;
    pub fn EnableWindow(hwnd: HWND, enable: BOOL) -> BOOL;
    pub fn IsWindowEnabled(hwnd: HWND) -> BOOL;
    pub fn SetFocus(hwnd: HWND) -> HWND;
    pub fn SetTimer(hwnd: HWND, id: usize, ms: u32, func: *const c_void) -> usize;
    pub fn KillTimer(hwnd: HWND, id: usize) -> BOOL;
    pub fn LoadCursorW(instance: HANDLE, name: *const u16) -> HANDLE;
    pub fn CreateIconFromResourceEx(bits: *const u8, size: u32, icon: BOOL, ver: u32, cx: i32, cy: i32, flags: u32) -> HANDLE;
    pub fn MessageBoxW(hwnd: HWND, text: *const u16, caption: *const u16, kind: u32) -> i32;
    pub fn GetClientRect(hwnd: HWND, rect: *mut RECT) -> BOOL;
    pub fn InvalidateRect(hwnd: HWND, rect: *const RECT, erase: BOOL) -> BOOL;
    pub fn BeginPaint(hwnd: HWND, ps: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(hwnd: HWND, ps: *const PAINTSTRUCT) -> BOOL;
    pub fn FillRect(hdc: HDC, rect: *const RECT, brush: HBRUSH) -> i32;
    pub fn DrawTextW(hdc: HDC, text: *const u16, len: i32, rect: *mut RECT, format: u32) -> i32;
    pub fn AdjustWindowRectEx(rect: *mut RECT, style: u32, menu: BOOL, ex_style: u32) -> BOOL;
    pub fn PrintWindow(hwnd: HWND, hdc: HDC, flags: u32) -> BOOL;
    pub fn SetScrollInfo(hwnd: HWND, bar: i32, info: *const SCROLLINFO, redraw: BOOL) -> i32;
    pub fn GetScrollInfo(hwnd: HWND, bar: i32, info: *mut SCROLLINFO) -> BOOL;
    pub fn ScrollWindowEx(
        hwnd: HWND,
        dx: i32,
        dy: i32,
        scroll: *const RECT,
        clip: *const RECT,
        region: HANDLE,
        update: *mut RECT,
        flags: u32,
    ) -> i32;
    pub fn GetParent(hwnd: HWND) -> HWND;
    pub fn DestroyWindow(hwnd: HWND) -> BOOL;
    pub fn IsWindow(hwnd: HWND) -> BOOL;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateFontW(
        height: i32,
        width: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike: u32,
        charset: u32,
        out_precision: u32,
        clip_precision: u32,
        quality: u32,
        pitch: u32,
        face: *const u16,
    ) -> HFONT;
    pub fn DeleteObject(object: HANDLE) -> BOOL;
    pub fn SelectObject(hdc: HDC, object: HANDLE) -> HANDLE;
    pub fn SetTextColor(hdc: HDC, color: COLORREF) -> COLORREF;
    pub fn SetBkColor(hdc: HDC, color: COLORREF) -> COLORREF;
    pub fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    pub fn CreateSolidBrush(color: COLORREF) -> HBRUSH;
    pub fn CreatePen(style: i32, width: i32, color: COLORREF) -> HANDLE;
    pub fn RoundRect(hdc: HDC, l: i32, t: i32, r: i32, b: i32, w: i32, h: i32) -> BOOL;
    pub fn Ellipse(hdc: HDC, l: i32, t: i32, r: i32, b: i32) -> BOOL;
    pub fn GetStockObject(index: i32) -> HANDLE;
    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn DeleteDC(hdc: HDC) -> BOOL;
    pub fn CreateDIBSection(
        hdc: HDC,
        info: *const BITMAPINFOHEADER,
        usage: u32,
        bits: *mut *mut c_void,
        section: HANDLE,
        offset: u32,
    ) -> HANDLE;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn LoadLibraryW(name: *const u16) -> HANDLE;
    pub fn GetProcAddress(module: HANDLE, name: *const u8) -> *mut c_void;
    pub fn CreateActCtxW(ctx: *const ACTCTXW) -> HANDLE;
    pub fn ActivateActCtx(ctx: HANDLE, cookie: *mut usize) -> BOOL;
}

/// A function of `dll` looked up at run time (`None` on an older Windows).
pub fn lookup(dll: &str, name: &str) -> Option<*mut c_void> {
    let module = unsafe { LoadLibraryW(crate::win::wide(dll).as_ptr()) };
    if module.is_null() {
        return None;
    }
    let name = format!("{name}\0");
    let f = unsafe { GetProcAddress(module, name.as_ptr()) };
    (!f.is_null()).then_some(f)
}

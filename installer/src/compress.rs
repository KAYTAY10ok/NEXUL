//! LZMS compression with the Compression API of Windows (`cabinet.dll`, Windows 8 and
//! later), loaded at run time: no library to install or link.

use std::ffi::c_void;

type Handle = isize;
type CreateFn = unsafe extern "system" fn(u32, *const c_void, *mut Handle) -> i32;
type RunFn = unsafe extern "system" fn(Handle, *const u8, usize, *mut u8, usize, *mut usize) -> i32;
type CloseFn = unsafe extern "system" fn(Handle) -> i32;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    fn GetLastError() -> u32;
}

const COMPRESS_ALGORITHM_LZMS: u32 = 5;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

struct Api {
    create_c: CreateFn,
    compress: RunFn,
    close_c: CloseFn,
    create_d: CreateFn,
    decompress: RunFn,
    close_d: CloseFn,
}

fn api() -> Result<Api, String> {
    unsafe {
        let name: Vec<u16> = "cabinet.dll\0".encode_utf16().collect();
        let m = LoadLibraryW(name.as_ptr());
        if m.is_null() {
            return Err("cabinet.dll introuvable (Windows 8 ou plus récent requis)".into());
        }
        let f = |n: &str| -> Result<*mut c_void, String> {
            let c = format!("{}\0", n);
            let p = GetProcAddress(m, c.as_ptr());
            if p.is_null() {
                Err(format!("fonction {} introuvable dans cabinet.dll", n))
            } else {
                Ok(p)
            }
        };
        Ok(Api {
            create_c: std::mem::transmute::<*mut c_void, CreateFn>(f("CreateCompressor")?),
            compress: std::mem::transmute::<*mut c_void, RunFn>(f("Compress")?),
            close_c: std::mem::transmute::<*mut c_void, CloseFn>(f("CloseCompressor")?),
            create_d: std::mem::transmute::<*mut c_void, CreateFn>(f("CreateDecompressor")?),
            decompress: std::mem::transmute::<*mut c_void, RunFn>(f("Decompress")?),
            close_d: std::mem::transmute::<*mut c_void, CloseFn>(f("CloseDecompressor")?),
        })
    }
}

pub fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
    let a = api()?;
    unsafe {
        let mut h: Handle = 0;
        if (a.create_c)(COMPRESS_ALGORITHM_LZMS, std::ptr::null(), &mut h) == 0 {
            return Err(format!("CreateCompressor : erreur {}", GetLastError()));
        }
        let mut needed = 0usize;
        let ok = (a.compress)(h, data.as_ptr(), data.len(), std::ptr::null_mut(), 0, &mut needed);
        if ok == 0 && GetLastError() != ERROR_INSUFFICIENT_BUFFER {
            (a.close_c)(h);
            return Err(format!("Compress : erreur {}", GetLastError()));
        }
        let mut out = vec![0u8; needed.max(16)];
        let mut size = 0usize;
        let ok = (a.compress)(h, data.as_ptr(), data.len(), out.as_mut_ptr(), out.len(), &mut size);
        (a.close_c)(h);
        if ok == 0 {
            return Err(format!("Compress : erreur {}", GetLastError()));
        }
        out.truncate(size);
        Ok(out)
    }
}

pub fn decompress(data: &[u8], size: usize) -> Result<Vec<u8>, String> {
    let a = api()?;
    unsafe {
        let mut h: Handle = 0;
        if (a.create_d)(COMPRESS_ALGORITHM_LZMS, std::ptr::null(), &mut h) == 0 {
            return Err(format!("CreateDecompressor : erreur {}", GetLastError()));
        }
        let mut out = vec![0u8; size];
        let mut got = 0usize;
        let ok = (a.decompress)(h, data.as_ptr(), data.len(), out.as_mut_ptr(), out.len(), &mut got);
        (a.close_d)(h);
        if ok == 0 || got != size {
            return Err(format!("données d'installation endommagées (décompression : erreur {})", GetLastError()));
        }
        Ok(out)
    }
}

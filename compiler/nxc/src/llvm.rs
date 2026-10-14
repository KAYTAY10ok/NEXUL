//! LLVM, used as a library (spec §10.1): `nxc` loads the C interface of LLVM
//! (`LLVM-C.dll`, `libLLVM.so`, `libLLVM.dylib`) at run time, gives it the IR produced by
//! the NEXUL back end, and gets an object file. No C compiler is involved.

use crate::toolchain::Toolchain;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

type Ref = *mut c_void;

#[allow(non_snake_case)]
struct Api {
    LLVMContextCreate: unsafe extern "C" fn() -> Ref,
    LLVMContextDispose: unsafe extern "C" fn(Ref),
    LLVMCreateMemoryBufferWithMemoryRangeCopy: unsafe extern "C" fn(*const c_char, usize, *const c_char) -> Ref,
    LLVMParseIRInContext: unsafe extern "C" fn(Ref, Ref, *mut Ref, *mut *mut c_char) -> i32,
    LLVMDisposeMessage: unsafe extern "C" fn(*mut c_char),
    LLVMVerifyModule: unsafe extern "C" fn(Ref, i32, *mut *mut c_char) -> i32,
    LLVMGetTargetFromTriple: unsafe extern "C" fn(*const c_char, *mut Ref, *mut *mut c_char) -> i32,
    LLVMCreateTargetMachine: unsafe extern "C" fn(Ref, *const c_char, *const c_char, *const c_char, i32, i32, i32) -> Ref,
    LLVMDisposeTargetMachine: unsafe extern "C" fn(Ref),
    LLVMCreateTargetDataLayout: unsafe extern "C" fn(Ref) -> Ref,
    LLVMSetModuleDataLayout: unsafe extern "C" fn(Ref, Ref),
    LLVMDisposeTargetData: unsafe extern "C" fn(Ref),
    LLVMCreatePassBuilderOptions: unsafe extern "C" fn() -> Ref,
    LLVMDisposePassBuilderOptions: unsafe extern "C" fn(Ref),
    LLVMRunPasses: unsafe extern "C" fn(Ref, *const c_char, Ref, Ref) -> Ref,
    LLVMGetErrorMessage: unsafe extern "C" fn(Ref) -> *mut c_char,
    LLVMDisposeErrorMessage: unsafe extern "C" fn(*mut c_char),
    LLVMTargetMachineEmitToFile: unsafe extern "C" fn(Ref, Ref, *const c_char, i32, *mut *mut c_char) -> i32,
    LLVMPrintModuleToString: unsafe extern "C" fn(Ref) -> *mut c_char,
    LLVMDisposeModule: unsafe extern "C" fn(Ref),
    init: Vec<unsafe extern "C" fn()>,
}

unsafe impl Send for Api {}
unsafe impl Sync for Api {}

#[cfg(windows)]
mod dl {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    extern "system" {
        fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    pub fn open(path: &std::path::Path) -> *mut c_void {
        let w: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // LOAD_WITH_ALTERED_SEARCH_PATH: dependencies are searched next to the library.
        unsafe { LoadLibraryExW(w.as_ptr(), std::ptr::null_mut(), 0x8) }
    }
    pub fn sym(h: *mut c_void, name: &str) -> *mut c_void {
        let n = format!("{}\0", name);
        unsafe { GetProcAddress(h, n.as_ptr()) }
    }
}

#[cfg(unix)]
mod dl {
    use std::ffi::{c_char, c_int, c_void, CString};
    #[cfg_attr(target_os = "linux", link(name = "dl"))]
    extern "C" {
        fn dlopen(name: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(h: *mut c_void, name: *const c_char) -> *mut c_void;
    }
    pub fn open(path: &std::path::Path) -> *mut c_void {
        let c = CString::new(path.to_string_lossy().as_bytes()).unwrap();
        unsafe { dlopen(c.as_ptr(), 2 /* RTLD_NOW */) }
    }
    pub fn sym(h: *mut c_void, name: &str) -> *mut c_void {
        let c = CString::new(name).unwrap();
        unsafe { dlsym(h, c.as_ptr()) }
    }
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();
/// LLVM's global state (target registry) is initialized once; code generation itself is
/// thread-safe with one context per compilation.
static INIT: Mutex<bool> = Mutex::new(false);

fn load(lib_path: &Path) -> Result<Api, String> {
    let h = dl::open(lib_path);
    if h.is_null() {
        return Err(format!("cannot load LLVM from `{}`", lib_path.display()));
    }
    macro_rules! f {
        ($name:ident) => {{
            let p = dl::sym(h, stringify!($name));
            if p.is_null() {
                return Err(format!("`{}` lacks the function {}", lib_path.display(), stringify!($name)));
            }
            unsafe { std::mem::transmute::<*mut c_void, _>(p) }
        }};
    }
    let mut init: Vec<unsafe extern "C" fn()> = Vec::new();
    for t in ["X86", "AArch64", "ARM", "RISCV", "WebAssembly"] {
        for part in ["TargetInfo", "Target", "TargetMC", "AsmPrinter", "AsmParser"] {
            let p = dl::sym(h, &format!("LLVMInitialize{}{}", t, part));
            if !p.is_null() {
                init.push(unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn()>(p) });
            }
        }
    }
    Ok(Api {
        LLVMContextCreate: f!(LLVMContextCreate),
        LLVMContextDispose: f!(LLVMContextDispose),
        LLVMCreateMemoryBufferWithMemoryRangeCopy: f!(LLVMCreateMemoryBufferWithMemoryRangeCopy),
        LLVMParseIRInContext: f!(LLVMParseIRInContext),
        LLVMDisposeMessage: f!(LLVMDisposeMessage),
        LLVMVerifyModule: f!(LLVMVerifyModule),
        LLVMGetTargetFromTriple: f!(LLVMGetTargetFromTriple),
        LLVMCreateTargetMachine: f!(LLVMCreateTargetMachine),
        LLVMDisposeTargetMachine: f!(LLVMDisposeTargetMachine),
        LLVMCreateTargetDataLayout: f!(LLVMCreateTargetDataLayout),
        LLVMSetModuleDataLayout: f!(LLVMSetModuleDataLayout),
        LLVMDisposeTargetData: f!(LLVMDisposeTargetData),
        LLVMCreatePassBuilderOptions: f!(LLVMCreatePassBuilderOptions),
        LLVMDisposePassBuilderOptions: f!(LLVMDisposePassBuilderOptions),
        LLVMRunPasses: f!(LLVMRunPasses),
        LLVMGetErrorMessage: f!(LLVMGetErrorMessage),
        LLVMDisposeErrorMessage: f!(LLVMDisposeErrorMessage),
        LLVMTargetMachineEmitToFile: f!(LLVMTargetMachineEmitToFile),
        LLVMPrintModuleToString: f!(LLVMPrintModuleToString),
        LLVMDisposeModule: f!(LLVMDisposeModule),
        init,
    })
}

fn api(tc: &Toolchain) -> Result<&'static Api, String> {
    let r = API.get_or_init(|| match &tc.llvm_lib {
        Some(p) => load(p),
        None => Err(Toolchain::missing_llvm()),
    });
    let api = r.as_ref().map_err(|e| e.clone())?;
    let mut done = INIT.lock().unwrap();
    if !*done {
        for f in &api.init {
            unsafe { f() };
        }
        *done = true;
    }
    Ok(api)
}

unsafe fn take_message(api: &Api, m: *mut c_char) -> String {
    if m.is_null() {
        return String::new();
    }
    let s = CStr::from_ptr(m).to_string_lossy().to_string();
    (api.LLVMDisposeMessage)(m);
    s
}

pub struct CodegenOptions<'a> {
    pub triple: &'a str,
    pub cpu: &'a str,
    pub features: &'a str,
    /// 0 to 3 (`-O0` to `-O3`), or `s`/`z` for size.
    pub opt_level: char,
    pub pic: bool,
    /// LLVM code model: 0 default, 5 medium (RISC-V `medany`, for code above 2 GiB).
    pub code_model: i32,
}

/// Compiles LLVM IR into an object file (or into optimized IR text when `emit_ir`).
pub fn compile(tc: &Toolchain, ir: &str, out: &Path, opts: &CodegenOptions, emit_ir: bool) -> Result<Option<String>, String> {
    let api = api(tc)?;
    unsafe {
        let ctx = (api.LLVMContextCreate)();
        let name = CString::new("nexul").unwrap();
        let buf = (api.LLVMCreateMemoryBufferWithMemoryRangeCopy)(ir.as_ptr() as *const c_char, ir.len(), name.as_ptr());
        let mut module: Ref = std::ptr::null_mut();
        let mut msg: *mut c_char = std::ptr::null_mut();
        if (api.LLVMParseIRInContext)(ctx, buf, &mut module, &mut msg) != 0 {
            let m = take_message(api, msg);
            (api.LLVMContextDispose)(ctx);
            return Err(format!("internal compiler error: invalid LLVM IR (this is a bug in nxc, please report it):\n{}", m));
        }
        let result = (|| -> Result<Option<String>, String> {
            let mut msg: *mut c_char = std::ptr::null_mut();
            if (api.LLVMVerifyModule)(module, 2, &mut msg) != 0 {
                return Err(format!("internal compiler error: LLVM IR verification failed (this is a bug in nxc, please report it):\n{}", take_message(api, msg)));
            }
            take_message(api, msg);
            let triple = CString::new(opts.triple).unwrap();
            let mut target: Ref = std::ptr::null_mut();
            let mut err: *mut c_char = std::ptr::null_mut();
            if (api.LLVMGetTargetFromTriple)(triple.as_ptr(), &mut target, &mut err) != 0 {
                return Err(format!("LLVM does not support the target `{}`: {}", opts.triple, take_message(api, err)));
            }
            let cpu = CString::new(opts.cpu).unwrap();
            let features = CString::new(opts.features).unwrap();
            let level = match opts.opt_level {
                '0' => 0,
                '1' => 1,
                '3' => 3,
                _ => 2,
            };
            let reloc = if opts.pic { 2 } else { 0 };
            let tm = (api.LLVMCreateTargetMachine)(target, triple.as_ptr(), cpu.as_ptr(), features.as_ptr(), level, reloc, opts.code_model);
            if tm.is_null() {
                return Err(format!("cannot create the code generator for `{}`", opts.triple));
            }
            let dl = (api.LLVMCreateTargetDataLayout)(tm);
            (api.LLVMSetModuleDataLayout)(module, dl);
            (api.LLVMDisposeTargetData)(dl);
            let r = (|| -> Result<Option<String>, String> {
                let pipeline = match opts.opt_level {
                    '0' => "default<O0>",
                    '1' => "default<O1>",
                    '3' => "default<O3>",
                    's' => "default<Os>",
                    'z' => "default<Oz>",
                    _ => "default<O2>",
                };
                let pbo = (api.LLVMCreatePassBuilderOptions)();
                let p = CString::new(pipeline).unwrap();
                let e = (api.LLVMRunPasses)(module, p.as_ptr(), tm, pbo);
                (api.LLVMDisposePassBuilderOptions)(pbo);
                if !e.is_null() {
                    let m = (api.LLVMGetErrorMessage)(e);
                    let s = CStr::from_ptr(m).to_string_lossy().to_string();
                    (api.LLVMDisposeErrorMessage)(m);
                    return Err(format!("LLVM optimization failed: {}", s));
                }
                if emit_ir {
                    let s = (api.LLVMPrintModuleToString)(module);
                    return Ok(Some(take_message(api, s)));
                }
                let path = CString::new(out.to_string_lossy().as_bytes()).unwrap();
                let mut err: *mut c_char = std::ptr::null_mut();
                if (api.LLVMTargetMachineEmitToFile)(tm, module, path.as_ptr(), 1, &mut err) != 0 {
                    return Err(format!("LLVM code generation failed: {}", take_message(api, err)));
                }
                Ok(None)
            })();
            (api.LLVMDisposeTargetMachine)(tm);
            r
        })();
        (api.LLVMDisposeModule)(module);
        (api.LLVMContextDispose)(ctx);
        result
    }
}

#![allow(unsafe_op_in_unsafe_fn)]
//! Native dynamic library loading and invocation bridge for Declare Function/Sub.
//!
//! On Windows: LoadLibraryW / GetProcAddress / stdcall invocation.
//! On Linux/macOS: dlopen / dlsym / C-ABI invocation.

use std::ffi::c_void;

#[cfg(target_os = "windows")]
use libffi::middle::{
    Arg as LibffiArg, Cif as LibffiCif, CodePtr as LibffiCodePtr, Type as LibffiType,
};
#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::sync::Mutex;

#[cfg(all(not(target_os = "windows"), not(target_os = "wasi")))]
use std::collections::HashMap;
#[cfg(all(not(target_os = "windows"), not(target_os = "wasi")))]
use std::sync::Mutex;

// ── FFI argument types ──

/// Represents a marshalled argument for a native DLL call.
#[derive(Debug, Clone)]
pub enum FfiArg {
    Long(i32),
    Integer(i16),
    Byte(u8),
    Boolean(i16),
    Double(f64),
    Single(f32),
    LongLong(i64),
    AnsiString(Vec<u8>), // null-terminated narrow string
    String(Vec<u16>),    // null-terminated wide string
    Pointer(*mut c_void),
}

/// Represents the expected return type from a native DLL call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiReturnType {
    Void,
    Long,
    Integer,
    Byte,
    Boolean,
    Double,
    Single,
    LongLong,
    LongPtr,
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
#[derive(Debug, Clone, Copy)]
enum PreparedX64Arg {
    Long(i32),
    Integer(i16),
    Byte(u8),
    Boolean(i16),
    Double(f64),
    Single(f32),
    LongLong(i64),
    Pointer(*mut c_void),
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
impl PreparedX64Arg {
    fn from_ffi_arg(arg: &FfiArg) -> Self {
        match arg {
            FfiArg::Long(v) => Self::Long(*v),
            FfiArg::Integer(v) => Self::Integer(*v),
            FfiArg::Byte(v) => Self::Byte(*v),
            FfiArg::Boolean(v) => Self::Boolean(*v),
            FfiArg::Double(v) => Self::Double(*v),
            FfiArg::Single(v) => Self::Single(*v),
            FfiArg::LongLong(v) => Self::LongLong(*v),
            FfiArg::AnsiString(v) => Self::Pointer(v.as_ptr().cast_mut().cast::<c_void>()),
            FfiArg::String(v) => Self::Pointer(v.as_ptr().cast_mut().cast::<c_void>()),
            FfiArg::Pointer(v) => Self::Pointer(*v),
        }
    }

    fn ffi_type(&self) -> LibffiType {
        match self {
            Self::Long(_) => LibffiType::i32(),
            Self::Integer(_) => LibffiType::i16(),
            Self::Byte(_) => LibffiType::u8(),
            Self::Boolean(_) => LibffiType::i16(),
            Self::Double(_) => LibffiType::f64(),
            Self::Single(_) => LibffiType::f32(),
            Self::LongLong(_) => LibffiType::i64(),
            Self::Pointer(_) => LibffiType::pointer(),
        }
    }

    fn as_arg(&self) -> LibffiArg<'_> {
        match self {
            Self::Long(v) => LibffiArg::new(v),
            Self::Integer(v) => LibffiArg::new(v),
            Self::Byte(v) => LibffiArg::new(v),
            Self::Boolean(v) => LibffiArg::new(v),
            Self::Double(v) => LibffiArg::new(v),
            Self::Single(v) => LibffiArg::new(v),
            Self::LongLong(v) => LibffiArg::new(v),
            Self::Pointer(v) => LibffiArg::new(v),
        }
    }
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn x64_return_type(return_type: FfiReturnType) -> LibffiType {
    match return_type {
        FfiReturnType::Void => LibffiType::void(),
        FfiReturnType::Long => LibffiType::i32(),
        FfiReturnType::Integer => LibffiType::i16(),
        FfiReturnType::Byte => LibffiType::u8(),
        FfiReturnType::Boolean => LibffiType::i16(),
        FfiReturnType::Double => LibffiType::f64(),
        FfiReturnType::Single => LibffiType::f32(),
        FfiReturnType::LongLong => LibffiType::i64(),
        FfiReturnType::LongPtr => LibffiType::pointer(),
    }
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
unsafe fn invoke_stdcall_x64(proc_addr: usize, args: &[FfiArg], return_type: FfiReturnType) -> i64 {
    let prepared_args: Vec<PreparedX64Arg> =
        args.iter().map(PreparedX64Arg::from_ffi_arg).collect();
    let cif = LibffiCif::new(
        prepared_args.iter().map(PreparedX64Arg::ffi_type),
        x64_return_type(return_type),
    );
    let ffi_args: Vec<LibffiArg<'_>> = prepared_args.iter().map(PreparedX64Arg::as_arg).collect();
    let code_ptr = LibffiCodePtr::from_ptr(proc_addr as *const c_void);

    match return_type {
        FfiReturnType::Void => {
            cif.call::<()>(code_ptr, &ffi_args);
            0
        }
        FfiReturnType::Long => cif.call::<i32>(code_ptr, &ffi_args) as i64,
        FfiReturnType::Integer => cif.call::<i16>(code_ptr, &ffi_args) as i64,
        FfiReturnType::Byte => cif.call::<u8>(code_ptr, &ffi_args) as i64,
        FfiReturnType::Boolean => cif.call::<i16>(code_ptr, &ffi_args) as i64,
        FfiReturnType::Double => cif.call::<f64>(code_ptr, &ffi_args).to_bits() as i64,
        FfiReturnType::Single => cif.call::<f32>(code_ptr, &ffi_args).to_bits() as i64,
        FfiReturnType::LongLong => cif.call::<i64>(code_ptr, &ffi_args),
        FfiReturnType::LongPtr => cif.call::<*mut c_void>(code_ptr, &ffi_args) as isize as i64,
    }
}

// ══════════════════════════════════════════════════════════════════════
// Windows implementation
// ══════════════════════════════════════════════════════════════════════

#[cfg(target_os = "windows")]
struct DllCache {
    modules: HashMap<String, usize>, // library name -> HMODULE as usize
}

#[cfg(target_os = "windows")]
static DLL_CACHE: Mutex<Option<DllCache>> = Mutex::new(None);

#[cfg(target_os = "windows")]
unsafe extern "system" {
    fn LoadLibraryW(lp_lib_file_name: *const u16) -> *mut c_void;
    fn GetModuleHandleW(lp_module_name: *const u16) -> *mut c_void;
    fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
    fn WideCharToMultiByte(
        CodePage: u32,
        dwFlags: u32,
        lpWideCharStr: *const u16,
        cchWideChar: i32,
        lpMultiByteStr: *mut u8,
        cbMultiByte: i32,
        lpDefaultChar: *const u8,
        lpUsedDefaultChar: *mut i32,
    ) -> i32;
}

#[cfg(target_os = "windows")]
const CP_ACP: u32 = 0;

// ── Public API (Windows) ──

/// Loads a DLL by name. Returns an opaque module handle.
/// The module is cached so subsequent loads of the same DLL are fast.
#[cfg(target_os = "windows")]
pub fn load_library(library: &str) -> Result<usize, String> {
    let mut guard = DLL_CACHE
        .lock()
        .map_err(|_| "DLL cache lock poisoned".to_string())?;
    let cache = guard.get_or_insert_with(|| DllCache {
        modules: HashMap::new(),
    });

    let key = library.to_ascii_lowercase();
    if let Some(&handle) = cache.modules.get(&key) {
        return Ok(handle);
    }

    for candidate in windows_library_candidates(library) {
        let wide: Vec<u16> = candidate.encode_utf16().chain(std::iter::once(0)).collect();
        let existing = unsafe { GetModuleHandleW(wide.as_ptr()) };
        if !existing.is_null() {
            let handle_val = existing as usize;
            cache.modules.insert(key, handle_val);
            return Ok(handle_val);
        }
    }

    for candidate in windows_library_candidates(library) {
        let wide: Vec<u16> = candidate.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
        if !handle.is_null() {
            let handle_val = handle as usize;
            cache.modules.insert(key, handle_val);
            return Ok(handle_val);
        }
    }
    Err(format!("LoadLibraryW failed for `{}`", library))
}

#[cfg(target_os = "windows")]
fn windows_library_candidates(library: &str) -> Vec<String> {
    let trimmed = library.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut out = vec![trimmed.to_string()];
    if !trimmed.contains(['\\', '/']) && !trimmed.contains('.') {
        out.push(format!("{trimmed}.dll"));
    }
    out
}

#[cfg(target_os = "windows")]
pub fn ansi_c_string(text: &str) -> Vec<u8> {
    if text.is_empty() {
        return vec![0];
    }

    let wide: Vec<u16> = text.encode_utf16().collect();
    let required = unsafe {
        WideCharToMultiByte(
            CP_ACP,
            0,
            wide.as_ptr(),
            wide.len() as i32,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            std::ptr::null_mut(),
        )
    };
    if required <= 0 {
        let mut fallback = text.as_bytes().to_vec();
        fallback.push(0);
        return fallback;
    }

    let mut bytes = vec![0u8; required as usize + 1];
    let written = unsafe {
        WideCharToMultiByte(
            CP_ACP,
            0,
            wide.as_ptr(),
            wide.len() as i32,
            bytes.as_mut_ptr(),
            required,
            std::ptr::null(),
            std::ptr::null_mut(),
        )
    };
    if written <= 0 {
        let mut fallback = text.as_bytes().to_vec();
        fallback.push(0);
        return fallback;
    }
    bytes.truncate(written as usize + 1);
    bytes[written as usize] = 0;
    bytes
}

/// Resolves a function address from a loaded module.
#[cfg(target_os = "windows")]
pub fn get_proc_address(module: usize, name: &str) -> Result<usize, String> {
    let name_bytes: Vec<u8> = name.bytes().chain(std::iter::once(0)).collect();
    let addr = unsafe { GetProcAddress(module as *mut c_void, name_bytes.as_ptr()) };
    if addr.is_null() {
        return Err(format!(
            "GetProcAddress failed for `{}` in module 0x{:X}",
            name, module
        ));
    }
    Ok(addr as usize)
}

/// Resolves a function by ordinal from a loaded module.
#[cfg(target_os = "windows")]
pub fn get_proc_address_ordinal(module: usize, ordinal: u16) -> Result<usize, String> {
    let addr = unsafe { GetProcAddress(module as *mut c_void, ordinal as usize as *const u8) };
    if addr.is_null() {
        return Err(format!(
            "GetProcAddress failed for ordinal {} in module 0x{:X}",
            ordinal, module
        ));
    }
    Ok(addr as usize)
}

/// Invokes a stdcall function pointer with the given arguments.
/// Returns the raw i64 return value (caller interprets based on return type).
#[cfg(target_os = "windows")]
pub fn invoke_stdcall(
    proc_addr: usize,
    args: &[FfiArg],
    return_type: FfiReturnType,
) -> Result<i64, String> {
    // For safety, we limit to a reasonable argument count
    if args.len() > 32 {
        return Err(format!(
            "too many arguments for stdcall invocation: {}",
            args.len()
        ));
    }

    #[cfg(target_arch = "x86_64")]
    {
        let result = unsafe { invoke_stdcall_x64(proc_addr, args, return_type) };
        return Ok(result);
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        // Marshal arguments to raw i64 values for the call on the legacy integer-only path.
        let mut raw_args: Vec<i64> = Vec::with_capacity(args.len());
        for arg in args {
            match arg {
                FfiArg::Long(v) => raw_args.push(*v as i64),
                FfiArg::Integer(v) => raw_args.push(*v as i64),
                FfiArg::Byte(v) => raw_args.push(*v as i64),
                FfiArg::Boolean(v) => raw_args.push(*v as i64),
                FfiArg::Double(v) => raw_args.push(v.to_bits() as i64),
                FfiArg::Single(v) => raw_args.push(v.to_bits() as i64),
                FfiArg::LongLong(v) => raw_args.push(*v),
                FfiArg::AnsiString(v) => raw_args.push(v.as_ptr() as i64),
                FfiArg::String(v) => raw_args.push(v.as_ptr() as i64),
                FfiArg::Pointer(v) => raw_args.push(*v as i64),
            }
        }
        let result = unsafe { invoke_stdcall_raw(proc_addr, &raw_args, return_type) };
        Ok(result)
    }
}

/// Raw stdcall invocation. This dispatches based on argument count
/// for common arities and falls back to a generic approach for larger calls.
#[cfg(target_os = "windows")]
#[cfg(not(target_arch = "x86_64"))]
unsafe fn invoke_stdcall_raw(proc_addr: usize, args: &[i64], _return_type: FfiReturnType) -> i64 {
    // On x86_64 Windows, the calling convention is actually __fastcall (first 4 args in registers).
    // We handle common arities explicitly for safety.
    type Fn0 = unsafe extern "system" fn() -> i64;
    type Fn1 = unsafe extern "system" fn(i64) -> i64;
    type Fn2 = unsafe extern "system" fn(i64, i64) -> i64;
    type Fn3 = unsafe extern "system" fn(i64, i64, i64) -> i64;
    type Fn4 = unsafe extern "system" fn(i64, i64, i64, i64) -> i64;
    type Fn5 = unsafe extern "system" fn(i64, i64, i64, i64, i64) -> i64;
    type Fn6 = unsafe extern "system" fn(i64, i64, i64, i64, i64, i64) -> i64;

    let f = proc_addr;
    match args.len() {
        0 => std::mem::transmute::<usize, Fn0>(f)(),
        1 => std::mem::transmute::<usize, Fn1>(f)(args[0]),
        2 => std::mem::transmute::<usize, Fn2>(f)(args[0], args[1]),
        3 => std::mem::transmute::<usize, Fn3>(f)(args[0], args[1], args[2]),
        4 => std::mem::transmute::<usize, Fn4>(f)(args[0], args[1], args[2], args[3]),
        5 => std::mem::transmute::<usize, Fn5>(f)(args[0], args[1], args[2], args[3], args[4]),
        6 => std::mem::transmute::<usize, Fn6>(f)(
            args[0], args[1], args[2], args[3], args[4], args[5],
        ),
        _ => {
            // Fallback: for large argument counts, call with first 6 args
            // This is a simplification; full coverage would need assembly thunks
            std::mem::transmute::<usize, Fn6>(f)(
                args.first().copied().unwrap_or(0),
                args.get(1).copied().unwrap_or(0),
                args.get(2).copied().unwrap_or(0),
                args.get(3).copied().unwrap_or(0),
                args.get(4).copied().unwrap_or(0),
                args.get(5).copied().unwrap_or(0),
            )
        }
    }
}

// ══════════════════════════════════════════════════════════════════════
// Linux / macOS implementation (dlopen / dlsym / C-ABI)
// ══════════════════════════════════════════════════════════════════════

#[cfg(all(not(target_os = "windows"), not(target_os = "wasi")))]
struct SoCache {
    modules: HashMap<String, usize>, // library name -> dlopen handle as usize
}

#[cfg(all(not(target_os = "windows"), not(target_os = "wasi")))]
static SO_CACHE: Mutex<Option<SoCache>> = Mutex::new(None);

/// Translates a VBA-style library name to a platform-appropriate shared library path.
///
/// VBA `Declare` statements reference Windows DLL names like `"kernel32"` or
/// `"user32.dll"`. On Linux/macOS we attempt a best-effort mapping:
///   - If the name already contains a path separator or a `.so` / `.dylib` suffix, use it as-is.
///   - Strip a trailing `.dll` (case-insensitive) and prepend `lib`, append `.so` (Linux)
///     or `.dylib` (macOS).
///   - For well-known system libraries (`libc`, `libm`, `libpthread`, `libdl`),
///     return the platform canonical name.
#[cfg(all(not(target_os = "windows"), not(target_os = "wasi")))]
fn translate_library_name(library: &str) -> String {
    // Already looks like a Unix shared library path
    if library.contains('/')
        || library.ends_with(".so")
        || library.ends_with(".dylib")
        || library.contains(".so.")
    {
        return library.to_string();
    }

    let base = library
        .strip_suffix(".dll")
        .or_else(|| library.strip_suffix(".DLL"))
        .or_else(|| library.strip_suffix(".Dll"))
        .unwrap_or(library);

    let lower = base.to_ascii_lowercase();

    // Well-known system library mappings
    match lower.as_str() {
        "msvcrt" | "ucrtbase" | "libc" => {
            if cfg!(target_os = "macos") {
                "libSystem.B.dylib".to_string()
            } else {
                "libc.so.6".to_string()
            }
        }
        "libm" => {
            if cfg!(target_os = "macos") {
                "libSystem.B.dylib".to_string()
            } else {
                "libm.so.6".to_string()
            }
        }
        "libpthread" | "pthread" => {
            if cfg!(target_os = "macos") {
                "libSystem.B.dylib".to_string()
            } else {
                "libpthread.so.0".to_string()
            }
        }
        "libdl" | "dl" => {
            if cfg!(target_os = "macos") {
                "libSystem.B.dylib".to_string()
            } else {
                "libdl.so.2".to_string()
            }
        }
        _ => {
            let ext = if cfg!(target_os = "macos") {
                "dylib"
            } else {
                "so"
            };
            // Prepend "lib" if not already present
            if lower.starts_with("lib") {
                format!("{}.{}", base, ext)
            } else {
                format!("lib{}.{}", base, ext)
            }
        }
    }
}

/// Returns the last dlopen/dlsym error as a String, or a generic message.
#[cfg(all(not(target_os = "windows"), not(target_os = "wasi")))]
fn dlerror_string() -> String {
    let err = unsafe { libc::dlerror() };
    if err.is_null() {
        "unknown dl error".to_string()
    } else {
        let cstr = unsafe { std::ffi::CStr::from_ptr(err) };
        cstr.to_string_lossy().into_owned()
    }
}

/// Loads a shared library by name using `dlopen`. Returns an opaque module handle.
/// The module is cached so subsequent loads of the same library are fast.
///
/// The library name is translated from VBA/Windows conventions to Unix conventions:
/// e.g. `"mylib.dll"` becomes `"libmylib.so"` on Linux or `"libmylib.dylib"` on macOS.
#[cfg(all(not(target_os = "windows"), not(target_os = "wasi")))]
pub fn load_library(library: &str) -> Result<usize, String> {
    let mut guard = SO_CACHE
        .lock()
        .map_err(|_| "shared library cache lock poisoned".to_string())?;
    let cache = guard.get_or_insert_with(|| SoCache {
        modules: HashMap::new(),
    });

    let key = library.to_ascii_lowercase();
    if let Some(&handle) = cache.modules.get(&key) {
        return Ok(handle);
    }

    let translated = translate_library_name(library);
    let c_name = std::ffi::CString::new(translated.as_str())
        .map_err(|_| format!("library name `{}` contains null byte", library))?;

    // Clear any prior error
    unsafe {
        libc::dlerror();
    }

    let handle = unsafe { libc::dlopen(c_name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        let err = dlerror_string();
        return Err(format!(
            "dlopen failed for `{}` (translated to `{}`): {}",
            library, translated, err
        ));
    }
    let handle_val = handle as usize;
    cache.modules.insert(key, handle_val);
    Ok(handle_val)
}

/// Resolves a function address from a loaded module using `dlsym`.
#[cfg(target_os = "wasi")]
pub fn load_library(library: &str) -> Result<usize, String> {
    Err(format!(
        "dynamic library loading is not available on wasm32/WASI for `{library}`"
    ))
}

#[cfg(all(not(target_os = "windows"), not(target_os = "wasi")))]
pub fn get_proc_address(module: usize, name: &str) -> Result<usize, String> {
    let c_name = std::ffi::CString::new(name)
        .map_err(|_| format!("symbol name `{}` contains null byte", name))?;

    // Clear any prior error
    unsafe {
        libc::dlerror();
    }

    let addr = unsafe { libc::dlsym(module as *mut c_void, c_name.as_ptr()) };
    if addr.is_null() {
        let err = dlerror_string();
        return Err(format!(
            "dlsym failed for `{}` in module 0x{:X}: {}",
            name, module, err
        ));
    }
    Ok(addr as usize)
}

/// Ordinal-based symbol resolution is not supported on Unix platforms.
/// Shared libraries on Linux/macOS do not use ordinal exports.
#[cfg(target_os = "wasi")]
pub fn get_proc_address(_module: usize, name: &str) -> Result<usize, String> {
    Err(format!(
        "dynamic symbol lookup is not available on wasm32/WASI for `{name}`"
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn get_proc_address_ordinal(_module: usize, ordinal: u16) -> Result<usize, String> {
    Err(format!(
        "ordinal-based symbol resolution (ordinal {}) is not supported on Unix platforms; \
         use named symbol exports instead",
        ordinal
    ))
}

/// Invokes a C-ABI function pointer with the given arguments.
/// Returns the raw i64 return value (caller interprets based on return type).
///
/// On Linux/macOS the platform calling convention is the System V AMD64 ABI (on x86_64)
/// or the standard C ABI on other architectures. We use `extern "C"` function pointer
/// transmutes matching the Windows bridge's arity-dispatch approach.
#[cfg(not(target_os = "windows"))]
pub fn invoke_stdcall(
    proc_addr: usize,
    args: &[FfiArg],
    return_type: FfiReturnType,
) -> Result<i64, String> {
    if args.len() > 32 {
        return Err(format!(
            "too many arguments for native invocation: {}",
            args.len()
        ));
    }

    let mut raw_args: Vec<i64> = Vec::with_capacity(args.len());
    for arg in args {
        match arg {
            FfiArg::Long(v) => raw_args.push(*v as i64),
            FfiArg::Integer(v) => raw_args.push(*v as i64),
            FfiArg::Byte(v) => raw_args.push(*v as i64),
            FfiArg::Boolean(v) => raw_args.push(*v as i64),
            FfiArg::Double(v) => raw_args.push(v.to_bits() as i64),
            FfiArg::Single(v) => raw_args.push(v.to_bits() as i64),
            FfiArg::LongLong(v) => raw_args.push(*v),
            FfiArg::AnsiString(v) => raw_args.push(v.as_ptr() as i64),
            FfiArg::String(v) => raw_args.push(v.as_ptr() as i64),
            FfiArg::Pointer(v) => raw_args.push(*v as i64),
        }
    }

    let result = unsafe { invoke_c_abi_raw(proc_addr, &raw_args, return_type) };
    Ok(result)
}

/// Raw C-ABI invocation using `extern "C"` function pointers.
/// Dispatches based on argument count for common arities.
#[cfg(not(target_os = "windows"))]
unsafe fn invoke_c_abi_raw(proc_addr: usize, args: &[i64], _return_type: FfiReturnType) -> i64 {
    type Fn0 = unsafe extern "C" fn() -> i64;
    type Fn1 = unsafe extern "C" fn(i64) -> i64;
    type Fn2 = unsafe extern "C" fn(i64, i64) -> i64;
    type Fn3 = unsafe extern "C" fn(i64, i64, i64) -> i64;
    type Fn4 = unsafe extern "C" fn(i64, i64, i64, i64) -> i64;
    type Fn5 = unsafe extern "C" fn(i64, i64, i64, i64, i64) -> i64;
    type Fn6 = unsafe extern "C" fn(i64, i64, i64, i64, i64, i64) -> i64;

    let f = proc_addr;
    match args.len() {
        0 => std::mem::transmute::<usize, Fn0>(f)(),
        1 => std::mem::transmute::<usize, Fn1>(f)(args[0]),
        2 => std::mem::transmute::<usize, Fn2>(f)(args[0], args[1]),
        3 => std::mem::transmute::<usize, Fn3>(f)(args[0], args[1], args[2]),
        4 => std::mem::transmute::<usize, Fn4>(f)(args[0], args[1], args[2], args[3]),
        5 => std::mem::transmute::<usize, Fn5>(f)(args[0], args[1], args[2], args[3], args[4]),
        6 => std::mem::transmute::<usize, Fn6>(f)(
            args[0], args[1], args[2], args[3], args[4], args[5],
        ),
        _ => {
            // Fallback: for large argument counts, call with first 6 args.
            // The System V AMD64 ABI passes the first 6 integer args in registers,
            // so this covers the register-passed arguments. Full coverage for more
            // than 6 args would require platform-specific stack manipulation.
            std::mem::transmute::<usize, Fn6>(f)(
                args.first().copied().unwrap_or(0),
                args.get(1).copied().unwrap_or(0),
                args.get(2).copied().unwrap_or(0),
                args.get(3).copied().unwrap_or(0),
                args.get(4).copied().unwrap_or(0),
                args.get(5).copied().unwrap_or(0),
            )
        }
    }
}

// ══════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;

    #[test]
    fn load_kernel32_and_resolve_get_tick_count() {
        let module = load_library("kernel32.dll").expect("kernel32.dll should load");
        assert!(module != 0, "module handle should be non-zero");
        let addr = get_proc_address(module, "GetTickCount").expect("GetTickCount should resolve");
        assert!(addr != 0, "proc address should be non-zero");
    }

    #[test]
    fn invoke_get_tick_count_returns_nonzero() {
        let module = load_library("kernel32.dll").expect("kernel32.dll should load");
        let addr = get_proc_address(module, "GetTickCount").expect("GetTickCount should resolve");
        let result = invoke_stdcall(addr, &[], FfiReturnType::Long).expect("invoke should succeed");
        assert!(result > 0, "GetTickCount should return positive value");
    }

    #[test]
    fn invoke_msvcrt_sqrt_round_trips_double_argument_and_return() {
        let module = load_library("msvcrt.dll").expect("msvcrt.dll should load");
        let addr = get_proc_address(module, "sqrt").expect("sqrt should resolve");
        let result = invoke_stdcall(addr, &[FfiArg::Double(156.25)], FfiReturnType::Double)
            .expect("invoke should succeed");
        let value = f64::from_bits(result as u64);
        assert!(
            (value - 12.5).abs() < 1.0e-10,
            "sqrt should round-trip the double lane; got {value}"
        );
    }

    #[test]
    fn invoke_msvcrt_wcstod_round_trips_double_return_with_pointer_arguments() {
        let module = load_library("msvcrt.dll").expect("msvcrt.dll should load");
        let addr = get_proc_address(module, "wcstod").expect("wcstod should resolve");
        let text: Vec<u16> = "123.5".encode_utf16().chain(std::iter::once(0)).collect();
        let result = invoke_stdcall(
            addr,
            &[
                FfiArg::Pointer(text.as_ptr().cast_mut().cast::<c_void>()),
                FfiArg::Pointer(std::ptr::null_mut()),
            ],
            FfiReturnType::Double,
        )
        .expect("invoke should succeed");
        let value = f64::from_bits(result as u64);
        assert!(
            (value - 123.5).abs() < 1.0e-10,
            "wcstod should round-trip the double return lane; got {value}"
        );
    }

    #[test]
    fn load_library_caches_modules() {
        let h1 = load_library("kernel32.dll").expect("first load");
        let h2 = load_library("kernel32.dll").expect("second load");
        assert_eq!(h1, h2, "same module should return same handle");
    }
}

#[cfg(test)]
#[cfg(not(target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn translate_library_name_strips_dll_suffix() {
        let result = translate_library_name("mylib.dll");
        assert!(
            result == "libmylib.so" || result == "libmylib.dylib",
            "got: {}",
            result
        );
    }

    #[test]
    fn translate_library_name_preserves_so_suffix() {
        assert_eq!(translate_library_name("libfoo.so"), "libfoo.so");
    }

    #[test]
    fn translate_library_name_preserves_dylib_suffix() {
        assert_eq!(translate_library_name("libfoo.dylib"), "libfoo.dylib");
    }

    #[test]
    fn translate_library_name_preserves_path() {
        assert_eq!(
            translate_library_name("/usr/lib/libfoo.so"),
            "/usr/lib/libfoo.so"
        );
    }

    #[test]
    fn translate_library_name_maps_msvcrt_to_libc() {
        let result = translate_library_name("msvcrt");
        assert!(
            result == "libc.so.6" || result == "libSystem.B.dylib",
            "got: {}",
            result
        );
    }

    #[test]
    fn translate_library_name_prepends_lib_prefix() {
        let result = translate_library_name("foo");
        assert!(
            result == "libfoo.so" || result == "libfoo.dylib",
            "got: {}",
            result
        );
    }

    #[test]
    fn translate_library_name_does_not_double_lib_prefix() {
        let result = translate_library_name("libbar");
        assert!(
            result == "libbar.so" || result == "libbar.dylib",
            "got: {}",
            result
        );
    }

    #[test]
    fn ordinal_resolution_returns_error() {
        let result = get_proc_address_ordinal(0, 42);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ordinal"));
    }

    #[test]
    fn load_libc_and_resolve_getpid() {
        let libc_name = if cfg!(target_os = "macos") {
            "libSystem.B.dylib"
        } else {
            "libc.so.6"
        };
        let module = load_library(libc_name).expect("libc should load");
        assert!(module != 0, "module handle should be non-zero");
        let addr = get_proc_address(module, "getpid").expect("getpid should resolve");
        assert!(addr != 0, "proc address should be non-zero");
    }

    #[test]
    fn invoke_getpid_returns_positive() {
        let libc_name = if cfg!(target_os = "macos") {
            "libSystem.B.dylib"
        } else {
            "libc.so.6"
        };
        let module = load_library(libc_name).expect("libc should load");
        let addr = get_proc_address(module, "getpid").expect("getpid should resolve");
        let result = invoke_stdcall(addr, &[], FfiReturnType::Long).expect("invoke should succeed");
        assert!(result > 0, "getpid should return positive value");
    }

    #[test]
    fn load_library_caches_modules() {
        let libc_name = if cfg!(target_os = "macos") {
            "libSystem.B.dylib"
        } else {
            "libc.so.6"
        };
        let h1 = load_library(libc_name).expect("first load");
        let h2 = load_library(libc_name).expect("second load");
        assert_eq!(h1, h2, "same module should return same handle");
    }

    #[test]
    fn vba_style_msvcrt_resolves_getpid() {
        // Simulates a VBA Declare like: Declare Function getpid Lib "msvcrt" () As Long
        let module = load_library("msvcrt").expect("msvcrt (mapped to libc) should load");
        let addr = get_proc_address(module, "getpid").expect("getpid should resolve");
        assert!(addr != 0);
    }
}

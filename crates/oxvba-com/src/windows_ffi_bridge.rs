#![allow(unsafe_op_in_unsafe_fn)]
//! Native dynamic library loading and invocation bridge for Declare Function/Sub.
//!
//! On Windows: LoadLibraryW / GetProcAddress / stdcall invocation.
//! On Linux/macOS: dlopen / dlsym / C-ABI invocation.

use std::ffi::c_void;

#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::sync::Mutex;

#[cfg(not(target_os = "windows"))]
use std::collections::HashMap;
#[cfg(not(target_os = "windows"))]
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
    String(Vec<u16>), // null-terminated wide string
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
    fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
}

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

    let wide: Vec<u16> = library.encode_utf16().chain(std::iter::once(0)).collect();
    let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
    if handle.is_null() {
        return Err(format!("LoadLibraryW failed for `{}`", library));
    }
    let handle_val = handle as usize;
    cache.modules.insert(key, handle_val);
    Ok(handle_val)
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

    // Marshal arguments to raw i64 values for the call
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
            FfiArg::String(v) => raw_args.push(v.as_ptr() as i64),
            FfiArg::Pointer(v) => raw_args.push(*v as i64),
        }
    }

    // Use platform-specific invocation
    let result = unsafe { invoke_stdcall_raw(proc_addr, &raw_args, return_type) };
    Ok(result)
}

/// Raw stdcall invocation. This dispatches based on argument count
/// for common arities and falls back to a generic approach for larger calls.
#[cfg(target_os = "windows")]
#[cfg(target_arch = "x86_64")]
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

#[cfg(not(target_os = "windows"))]
struct SoCache {
    modules: HashMap<String, usize>, // library name -> dlopen handle as usize
}

#[cfg(not(target_os = "windows"))]
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
#[cfg(not(target_os = "windows"))]
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
#[cfg(not(target_os = "windows"))]
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
#[cfg(not(target_os = "windows"))]
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
#[cfg(not(target_os = "windows"))]
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

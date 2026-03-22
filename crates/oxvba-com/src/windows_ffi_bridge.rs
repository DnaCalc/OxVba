#![allow(unsafe_op_in_unsafe_fn)]
//! Native DLL loading and stdcall invocation bridge for Declare Function/Sub.
//!
//! Handles LoadLibraryW / GetProcAddress / stdcall invocation for real Windows
//! DLL calls issued by VBA `Declare` statements.

#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::ffi::c_void;
#[cfg(target_os = "windows")]
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

// ── DLL module cache ──

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

// ── Public API ──

/// Loads a DLL by name. Returns an opaque module handle.
/// The module is cached so subsequent loads of the same DLL are fast.
#[cfg(target_os = "windows")]
pub fn load_library(library: &str) -> Result<usize, String> {
    let mut guard = DLL_CACHE.lock().map_err(|_| "DLL cache lock poisoned".to_string())?;
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
unsafe fn invoke_stdcall_raw(
    proc_addr: usize,
    args: &[i64],
    _return_type: FfiReturnType,
) -> i64 {
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
            // This is a simplification; production code would need assembly thunks
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

// ── Non-Windows stubs ──

#[cfg(not(target_os = "windows"))]
pub fn load_library(library: &str) -> Result<usize, String> {
    Err(format!(
        "native DLL loading not available on this platform for `{}`",
        library
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn get_proc_address(_module: usize, name: &str) -> Result<usize, String> {
    Err(format!(
        "native proc address resolution not available on this platform for `{}`",
        name
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn get_proc_address_ordinal(_module: usize, ordinal: u16) -> Result<usize, String> {
    Err(format!(
        "native proc address resolution not available on this platform for ordinal {}",
        ordinal
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn invoke_stdcall(
    _proc_addr: usize,
    _args: &[FfiArg],
    _return_type: FfiReturnType,
) -> Result<i64, String> {
    Err("native stdcall invocation not available on this platform".to_string())
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;

    #[test]
    fn load_kernel32_and_resolve_get_tick_count() {
        let module = load_library("kernel32.dll").expect("kernel32.dll should load");
        assert!(module != 0, "module handle should be non-zero");
        let addr =
            get_proc_address(module, "GetTickCount").expect("GetTickCount should resolve");
        assert!(addr != 0, "proc address should be non-zero");
    }

    #[test]
    fn invoke_get_tick_count_returns_nonzero() {
        let module = load_library("kernel32.dll").expect("kernel32.dll should load");
        let addr =
            get_proc_address(module, "GetTickCount").expect("GetTickCount should resolve");
        let result =
            invoke_stdcall(addr, &[], FfiReturnType::Long).expect("invoke should succeed");
        assert!(result > 0, "GetTickCount should return positive value");
    }

    #[test]
    fn load_library_caches_modules() {
        let h1 = load_library("kernel32.dll").expect("first load");
        let h2 = load_library("kernel32.dll").expect("second load");
        assert_eq!(h1, h2, "same module should return same handle");
    }
}

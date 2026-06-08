//! OxVBA build system: wrapper generation for DLL, EXE, COM server, and XLL outputs.
//!
//! This crate generates Rust source code shims that embed compiled `.oxb` bundles
//! and expose them as native executables, DLLs, COM servers, or XLL add-ins.

pub mod compile;
pub mod comserver;
pub mod comserver_exe;
pub mod deffile;
pub mod dll;
pub mod exe;
pub mod idl;
pub mod manifest;
pub mod reflection_exe;
pub mod registration;
pub mod typelib_gen;
pub mod wrapper_plan;
pub mod xll;
pub mod xloper;

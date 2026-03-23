//! OxVBA build system: wrapper generation for DLL, EXE, COM server, and XLL outputs.
//!
//! This crate generates Rust source code shims that embed compiled `.oxb` bundles
//! and expose them as native executables, DLLs, COM servers, or XLL add-ins.

pub mod comserver;
pub mod comserver_exe;
pub mod compile;
pub mod deffile;
pub mod dll;
pub mod exe;
pub mod idl;
pub mod manifest;
pub mod registration;
pub mod xloper;
pub mod xll;

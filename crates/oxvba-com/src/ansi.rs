//! ANSI (system code page) ↔ Unicode conversion for byte-exact VBA string I/O.
//!
//! The codec itself now lives in `oxvba-runtime` (the value substrate shared by
//! both the `Declare` ANSI string marshalling here and the `Chr`/`Asc` built-ins
//! in `oxvba-lib`). This module re-exports it so existing `oxvba_com::ansi::…`
//! call sites keep working unchanged.

pub use oxvba_runtime::ansi::{ansi_decode, ansi_encode};

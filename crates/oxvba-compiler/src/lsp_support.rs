//! Re-exports of compiler internals needed by the language service.
//!
//! These functions are `pub(crate)` in `resolve.rs` and re-exported here
//! as `pub` so that `oxvba-languageservice` (and only it) can use them.

pub use crate::resolve::{
    IntrinsicSpec, IntrinsicSurface, ProcKind, detect_proc_kind, intrinsic_spec, normalize_ident,
    parse_proc_signature,
};

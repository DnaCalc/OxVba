//! The elaboration pass: lowering the binder's resolved Core IR tree into OxIR's
//! typed basic-block CFG.
//!
//! This is where the type information the binder recorded on the Core IR bindings
//! (`ty: VarTypeRef`) is recovered into OxIR's [`OxTy`] lattice, structured control
//! flow is lowered to basic blocks, and explicit effects / box-unbox / coercions are
//! inserted.
//!
//! [`OxTy`]: crate::ty::OxTy
//!
//! Landing incrementally:
//! - [`types`] — the `VarTypeRef → OxTy` lowering (object/UDT names resolved through a
//!   [`NameResolver`]).
//! - [`lower`] — the CFG-building spine ([`elaborate`]): the scalar / control-flow /
//!   `VbaProc`-call core, with explicit [`ElaborateError::Unimplemented`] for the rest.

pub mod lower;
pub mod types;

pub use lower::{ElaborateError, elaborate};
pub use types::{NameResolver, ResolvedTypeName, lower_var_type};

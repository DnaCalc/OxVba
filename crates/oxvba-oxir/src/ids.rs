//! Index newtypes for OxIR structural references.
//!
//! (The *type*-bearing ids — [`crate::ty::ClassId`], [`crate::ty::IfaceId`],
//! [`crate::ty::RecordLayoutId`] — live in [`crate::ty`] because they appear inside
//! [`crate::ty::OxTy`]. These are the *structural* ids of the IR graph.)

use serde::{Deserialize, Serialize};

/// Index into [`crate::program::OxProgram::funcs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FuncId(pub usize);

/// Index into an [`crate::program::OxFunc`]'s block list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub usize);

/// A frame-local typed variable (params first, then locals). Mutable; may be
/// aliased (ByRef) or address-taken, which gates whether a backend SSA-promotes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalId(pub usize);

/// A module-level typed global. Persists across calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GlobalId(pub usize);

/// A single-assignment, never-aliased result of a pure op (the SSA-friendly half of
/// the value model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TempId(pub usize);

/// Index into [`crate::program::OxProgram`]'s cross-bundle import table (resolved at
/// link time to a `(unit, target)` pair).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImportId(pub usize);

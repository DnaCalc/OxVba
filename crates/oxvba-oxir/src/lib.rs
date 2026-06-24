//! `oxvba-oxir` — OxIR, the typed, backend-neutral mid-level IR for OxVBA.
//!
//! OxIR is a **typed, basic-block CFG with typed locals/places** — MIR-like, *not*
//! authored SSA. It is produced from the resolved Core IR tree by an elaboration
//! pass, and it is the single canonical executable-semantic artifact: the new
//! interpreter `oxvba-vm3` executes it (and is its executable specification) and a
//! Cranelift backend lowers it to native code. The IR carries no backend types, so
//! future wasm / copy-and-patch / LLVM backends stay reachable behind one semantic
//! kernel.
//!
//! This crate is being built bottom-up. The first landed layer is the **type
//! lattice** ([`ty`]) — the static type each OxIR value/local/place carries, which
//! is exactly the information the legacy `linearize` pass discards. The op set,
//! CFG/block structure, the COM interface + typed-method descriptor tables, and the
//! elaboration pass land on top of it.

pub mod ty;

pub use ty::{ArrayShape, ClassId, IfaceId, ObjClass, OxTy, RecordLayoutId};

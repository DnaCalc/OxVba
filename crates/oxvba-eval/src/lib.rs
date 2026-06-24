//! `oxvba-eval` — the OxVBA **semantic kernel**.
//!
//! Each value-core op's meaning is defined **once** here, over a `ValueOps` /
//! `Builder` abstraction, so that the interpreters (`oxvba-vm2`, `oxvba-vm3`) and
//! the Cranelift JIT all consume one source of truth rather than three
//! hand-written copies that must be kept in sync:
//!
//! - an interpreter instantiates `ValueOps` **concretely** — operations compute on
//!   real `Variant`s / typed lanes; running the kernel *is* interpreting;
//! - the JIT instantiates it as **staging** — operations *emit code* instead of
//!   computing; running the kernel over a program *is* compiling it.
//!
//! This makes vm3↔JIT divergence structurally hard for the value core (arithmetic,
//! coercion, comparison, string ops, box/unbox), and makes a future
//! wasm / copy-and-patch backend a new `Builder` instantiation rather than a
//! re-implementation.
//!
//! This crate is being populated by extracting the pure value semantics and interop
//! helpers currently living in `oxvba-vm2` (`arith`, `collection`, the `declare`/COM
//! dispatch helpers, pointer helpers, array/record ops) into a single shared home.
//!
//! Landed so far: [`arith`] (the Variant arithmetic / comparison / boolean /
//! coercion value-core — the numeric-fidelity spec both interpreters and the JIT
//! must agree on) and [`collection`] (the built-in `VBA.Collection` data model). The
//! `ValueOps` staging abstraction and the `self`-coupled interop helpers follow as
//! vm3 and the JIT split materializes.

pub mod arith;
pub mod collection;

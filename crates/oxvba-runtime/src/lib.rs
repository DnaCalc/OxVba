//! oxvba-runtime: core Variant representation and runtime semantics scaffolding.

pub mod alloc;
pub mod arithmetic;
pub mod bstr;
pub mod builtins;
pub mod coerce;
pub mod decimal;
pub mod safe_array;
pub mod variant;

pub use variant::{VarType, Variant};

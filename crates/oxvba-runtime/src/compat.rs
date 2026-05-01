//! Compatibility surface for the legacy semantic `RuntimeValue` carrier.
//!
//! Retained runtime APIs should use [`crate::Variant`] and related typed
//! substrate values. Code that still needs pre-Variant projection semantics
//! imports from this module explicitly so broad execution contracts do not
//! accidentally depend on `RuntimeValue` through root re-exports.

pub use crate::runtime_value::RuntimeValue;

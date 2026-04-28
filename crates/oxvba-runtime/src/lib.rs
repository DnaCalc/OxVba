//! oxvba-runtime: core Variant representation and runtime semantics scaffolding.

pub mod alloc;
pub mod arithmetic;
pub mod bstr;
pub mod builtins;
pub mod coerce;
pub mod decimal;
pub mod object_ref;
pub mod pointer_helpers;
pub mod runtime_value;
pub mod safe_array;
pub mod variant;

pub use coerce::{runtime_value_to_vba_str, runtime_value_to_vba_string, variant_to_vba_string};
pub use decimal::Decimal96;
pub use object_ref::{ObjectRef, RawRuntimeIUnknown, RawRuntimeIUnknownVtbl, RuntimeInterfaceId};
pub use runtime_value::{
    BindingHandle, CurrencyValue, DynLinkSymbol, F64Subtype, F64Value, RuntimeValue,
};
pub use variant::{VarType, Variant, VariantCore};

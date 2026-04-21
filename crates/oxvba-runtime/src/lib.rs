//! oxvba-runtime: core Variant representation and runtime semantics scaffolding.

pub mod alloc;
pub mod arithmetic;
pub mod bstr;
pub mod builtins;
pub mod coerce;
pub mod decimal;
pub mod pointer_helpers;
pub mod runtime_value;
pub mod safe_array;
pub mod value_tags;
pub mod variant;

pub use coerce::{runtime_value_to_vba_str, runtime_value_to_vba_string};
pub use decimal::Decimal96;
pub use runtime_value::{
    BindingHandle, CurrencyValue, DynLinkSymbol, F64Subtype, F64Value, ObjectHandle, RuntimeValue,
};
pub use variant::{VarType, Variant, VariantCore};

//! oxvba-runtime: core Variant representation and runtime semantics scaffolding.

pub mod alloc;
pub mod arithmetic;
pub mod bstr;
pub mod builtins;
pub mod coerce;
pub mod decimal;
pub mod object_ref;
pub mod pointer_helpers;
pub mod safe_array;
pub mod value_types;
pub mod variant;

pub use coerce::variant_to_vba_string;
pub use decimal::Decimal96;
pub use object_ref::{
    ObjectRef, RawRuntimeIUnknown, RawRuntimeIUnknownVtbl, RuntimeClassDescriptor,
    RuntimeDispatchCacheKey, RuntimeDispatchPlan, RuntimeDispatchPlanCache,
    RuntimeInterfaceDescriptor, RuntimeInterfaceId, RuntimeMemberDescriptor,
    RuntimeMemberInvokeKind, RuntimeParamDescriptor, RuntimeValueType,
};
pub use value_types::{BindingHandle, CurrencyValue, DynLinkSymbol, F64Subtype, F64Value};
pub use variant::{VarType, Variant, VariantCore};

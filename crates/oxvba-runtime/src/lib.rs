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
    ObjectRef, RUNTIME_GUID_ICONNECTIONPOINT, RUNTIME_GUID_ICONNECTIONPOINTCONTAINER,
    RUNTIME_GUID_IDISPATCH, RUNTIME_GUID_IUNKNOWN, RUNTIME_ICONNECTIONPOINT_INTERFACE_IDENTITY,
    RUNTIME_ICONNECTIONPOINTCONTAINER_INTERFACE_IDENTITY, RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
    RUNTIME_IUNKNOWN_INTERFACE_IDENTITY, RawRuntimeIUnknown, RawRuntimeIUnknownVtbl,
    RuntimeClassDescriptor, RuntimeDispatchCacheKey, RuntimeDispatchPlan, RuntimeDispatchPlanCache,
    RuntimeGuid, RuntimeInterfaceDescriptor, RuntimeInterfaceId, RuntimeInterfaceIdentity,
    RuntimeInterfaceKind, RuntimeMemberDescriptor, RuntimeMemberInvokeKind, RuntimeParamDescriptor,
    RuntimeValueType,
};
pub use value_types::{BindingHandle, CurrencyValue, DynLinkSymbol, F64Subtype, F64Value};
pub use variant::{VarType, Variant, VariantCore};

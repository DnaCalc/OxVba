//! oxvba-runtime: core Variant representation and runtime semantics scaffolding.

pub mod alloc;
pub mod ansi;
pub mod arithmetic;
pub mod bstr;
pub mod builtins;
pub mod call_frame;
pub mod coerce;
pub mod com_record;
pub mod date;
pub mod decimal;
pub mod object_ref;
pub mod pointer_helpers;
pub mod safe_array;
pub mod value_types;
pub mod variant;
pub mod vba_record;

pub use call_frame::{
    RuntimeByRefSlot, RuntimeByRefWriteback, RuntimeCallArgument, RuntimeCallContext,
    RuntimeCallError, RuntimeCallFrame, RuntimeCallKind, RuntimeCallResult, RuntimeCallSelector,
    RuntimeCallSource, RuntimeNamedArgument,
};
pub use coerce::{print_display_text, variant_to_vba_string};
pub use com_record::{ComRecord, ComRecordCloneFn, ComRecordDestroyFn};
pub use decimal::Decimal96;
pub use object_ref::{
    ObjectRef, RUNTIME_GUID_ICONNECTIONPOINT, RUNTIME_GUID_ICONNECTIONPOINTCONTAINER,
    RUNTIME_GUID_IDISPATCH, RUNTIME_GUID_IUNKNOWN, RUNTIME_ICONNECTIONPOINT_INTERFACE_IDENTITY,
    RUNTIME_ICONNECTIONPOINTCONTAINER_INTERFACE_IDENTITY, RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
    RUNTIME_IUNKNOWN_INTERFACE_IDENTITY, RawRuntimeIUnknown, RawRuntimeIUnknownVtbl,
    RuntimeApartmentModel, RuntimeClassDescriptor, RuntimeDispatchCacheKey, RuntimeDispatchPlan,
    RuntimeDispatchPlanCache, RuntimeGuid, RuntimeInterfaceDescriptor, RuntimeInterfaceId,
    RuntimeInterfaceIdentity, RuntimeInterfaceKind, RuntimeInterfaceProjection,
    RuntimeLifetimePolicy, RuntimeMemberDescriptor, RuntimeMemberInvokeKind, RuntimeObjectIdentity,
    RuntimeParamDescriptor, RuntimeValueType,
};
pub use object_ref::{
    finish_pending_termination, has_pending_terminations, reset_pending_terminations,
    retained_parked_termination_object, take_pending_terminations,
};
pub use value_types::{BindingHandle, CurrencyValue, DynLinkSymbol, F64Subtype, F64Value};
pub use variant::{VarType, Variant, VariantCore};
pub use vba_record::{
    VbaRecord, VbaRecordFieldKind, VbaRecordFieldLayout, VbaRecordFieldSpec, VbaRecordLayout,
};

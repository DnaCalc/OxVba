//! oxvba-runtime: core Variant representation and runtime semantics scaffolding.

pub mod alloc;
pub mod ansi;
pub mod arithmetic;
pub mod bstr;
pub mod builtins;
pub mod call_frame;
pub mod callback_thunks;
pub mod coerce;
pub mod collection;
pub mod com_record;
pub mod date;
pub mod decimal;
pub mod live_counters;
pub mod object_ref;
pub mod pointer_helpers;
pub mod safe_array;
pub mod value_types;
pub mod variant;
pub mod vba_date;
pub mod vba_error;
pub mod vba_radix;
pub mod vba_record;

pub use call_frame::{
    RuntimeByRefSlot, RuntimeByRefWriteback, RuntimeCallArgument, RuntimeCallContext,
    RuntimeCallError, RuntimeCallFrame, RuntimeCallKind, RuntimeCallResult, RuntimeCallSelector,
    RuntimeCallSource, RuntimeNamedArgument,
};
pub use callback_thunks::{
    CallbackExecutor, CallbackRegistration, CallbackThunkError, register_callback,
};
pub use coerce::{print_display_text, variant_to_vba_string, write_display_text};
pub use com_record::{ComRecord, ComRecordCloneFn, ComRecordDestroyFn};
pub use decimal::Decimal96;
pub use live_counters::{HandleBalance, LiveHandleCounts, live_handle_counts};
pub use object_ref::{
    ObjectRef, RUNTIME_CLASS_LIFECYCLE_NONE, RUNTIME_GUID_ICONNECTIONPOINT,
    RUNTIME_GUID_ICONNECTIONPOINTCONTAINER, RUNTIME_GUID_IDISPATCH, RUNTIME_GUID_IUNKNOWN,
    RUNTIME_ICONNECTIONPOINT_INTERFACE_IDENTITY,
    RUNTIME_ICONNECTIONPOINTCONTAINER_INTERFACE_IDENTITY, RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
    RUNTIME_IUNKNOWN_INTERFACE_IDENTITY, RawRuntimeIUnknown, RawRuntimeIUnknownVtbl,
    RuntimeApartmentModel, RuntimeClassActivationDescriptor, RuntimeClassAsNewFieldDescriptor,
    RuntimeClassDescriptor, RuntimeClassFieldDescriptor, RuntimeClassLifecycleDescriptor,
    RuntimeDescriptorIssue, RuntimeDispatchCacheKey, RuntimeDispatchPlan, RuntimeDispatchPlanCache,
    RuntimeGuid, RuntimeInterfaceDescriptor, RuntimeInterfaceId, RuntimeInterfaceIdentity,
    RuntimeInterfaceKind, RuntimeInterfaceProjection, RuntimeLifetimePolicy,
    RuntimeMemberDescriptor, RuntimeMemberInvokeKind, RuntimeObjectIdentity,
    RuntimeParamDescriptor, RuntimeProjectClassIdentity, RuntimeValueType,
};
pub use object_ref::{
    finish_pending_termination, has_pending_terminations, reset_pending_terminations,
    retained_parked_termination_object, take_pending_terminations,
};
pub use value_types::{BindingHandle, CurrencyValue, DynLinkSymbol, F64Subtype, F64Value};
pub use variant::{VarType, Variant, VariantCore};
pub use vba_date::{
    VBA_EPOCH_DAYS_FROM_UNIX, civil_from_days, days_from_civil, format_general_date,
    format_write_date, serial_to_hms, serial_to_ymd, ymd_to_serial,
};
pub use vba_error::default_error_message;
pub use vba_radix::{
    VbaRadixWidth, parse_vba_radix, parse_vba_radix_with_width, vba_radix_signed_value,
    vba_radix_width,
};
pub use vba_record::{
    VbaRecord, VbaRecordFieldKind, VbaRecordFieldLayout, VbaRecordFieldSpec, VbaRecordLayout,
};

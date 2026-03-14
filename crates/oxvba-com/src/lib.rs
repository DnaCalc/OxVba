//! oxvba-com: COM abstraction scaffolding.

pub mod cycle_gc;
pub mod dispatch;
pub mod dynamic_object;
pub mod invoke_policy;
pub mod model;
pub mod platform;
pub mod refcount;
pub mod runtime_state;
pub mod typelib;
pub mod typelib_cache;
pub mod typelib_catalog;
#[cfg(target_os = "windows")]
pub mod windows_client;
#[cfg(target_os = "windows")]
pub mod windows_connection_point;
#[cfg(target_os = "windows")]
pub mod windows_invoke;
#[cfg(target_os = "windows")]
pub mod windows_runtime_state;
#[cfg(target_os = "windows")]
pub mod windows_test_dispatch;
#[cfg(target_os = "windows")]
pub mod windows_variant;

pub use dispatch::{ComDispatch, DispatchResult};
pub use dynamic_object::{
    DynamicCallArg, DynamicCallKind, DynamicCallRequest, DynamicCallbackToken, DynamicEventPayload,
    DynamicMemberSelector, DynamicObjectBridge, DynamicObjectToken, DynamicSubscriptionToken,
    DynamicValue,
};
pub use invoke_policy::{
    BoundRuntimeInvokePlan, UnboundRuntimeInvokePlan, canonicalize_member_known_args,
    legacy_runtime_arg_values, plan_bound_runtime_invoke, plan_unbound_runtime_invoke,
    validate_named_arg_order,
};

pub use model::{
    ComCallbackPayload, ComCallbackToken, ComInvokeArg, ComInvokeKind, ComInvokeRequest,
    ComMemberToken, ComObjectDescriptor, ComObjectToken, ComObjectTransportKind,
    ComSubscriptionToken, ComValue, DISPATCH_INVOKE_MISSING_ARG_TOKEN,
};
pub use refcount::RefCount;
pub use runtime_state::{
    ComBinding, ComDirectDispatchSpec, ComEventCallback, ComEventPath, ComEventSpec,
    ComEventSubscription, ComEventTriggerSpec, ComMemberSpec, ComRuntimeState,
    binding_from_typelib_metadata,
};
pub use typelib::{
    TypeLibCacheScope, TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind,
    TypeLibMemberMetadata, TypeLibMetadataBlob, TypeLibResolveRequest, TypeLibResolvedIdentity,
};
pub use typelib_cache::TypeLibMetadataCacheState;
pub use typelib_catalog::{
    build_typelib_metadata, event_spec_from_typelib_metadata,
    known_typelib_identity_for_prog_id_name, member_spec_from_typelib_metadata,
    resolve_known_typelib_identity, source_interface_event_spec_supported,
};
#[cfg(target_os = "windows")]
pub use windows_client::{
    COM_CONNECT_E_CANNOTCONNECT, COM_CONNECT_E_NOCONNECTION, COM_DISP_E_BADPARAMCOUNT,
    COM_DISP_E_EXCEPTION, COM_DISP_E_MEMBERNOTFOUND, COM_DISP_E_PARAMNOTFOUND,
    COM_DISP_E_TYPEMISMATCH, COM_DISP_E_UNKNOWNNAME, COM_DISPID_PROPERTYPUT, COM_E_INVALIDARG,
    COM_E_NOINTERFACE, COM_E_NOTIMPL, COM_S_OK, IID_ICONNECTIONPOINT,
    IID_ICONNECTIONPOINTCONTAINER, IID_IDISPATCH, IID_IUNKNOWN, IID_NULL, RawIConnectionPoint,
    RawIConnectionPointContainer, RawIConnectionPointContainerVtbl, RawIConnectionPointVtbl,
    RawIDispatch, RawIDispatchVtbl, RawIUnknown, RawIUnknownVtbl, activate_dispatch_by_prog_id,
    activate_runtime_dispatch, add_ref_dispatch, get_dispid_by_name, get_dispids_by_names,
    guid_equals, parse_guid_canonical, query_dispatch_from_unknown, release_connection_point,
    release_dispatch, release_unknown, resolve_named_argument_dispids,
};
#[cfg(target_os = "windows")]
pub use windows_connection_point::{
    DispatchEventSinkConfig, RawSingleI32SourceEvents, WindowsConnectionPointTransport,
    try_advise_dispatch_event_sink, try_advise_single_i32_source_interface_event_sink,
    unadvise_connection_point,
};
#[cfg(target_os = "windows")]
pub use windows_invoke::{
    ComInvokeExceptionInfo, ComInvokeFailure, invoke_direct_dispid_runtime_value,
    invoke_dispatch_runtime_value, invoke_member_spec_runtime_value, take_excepinfo,
};
#[cfg(target_os = "windows")]
pub use windows_runtime_state::{
    ReleasedWindowsComObject, WindowsComClientState, WindowsComSubscriptionTransport,
    advise_event_subscription, bind_native_dispatch_result, cache_member_dispid, callback_arg,
    callback_arity, callback_subscription_token, collect_stale_callbacks_for_subscription,
    event_callback_args_from_member_token, event_is_source_interface_only,
    event_signature_arity_for_binding, insert_bound_object_binding, release_callback,
    release_object_binding, release_subscription_transport, remove_subscription_callbacks,
    resolve_bound_native_dispatch, resolve_member_dispid_cached, resolve_subscription_transport,
    take_polled_callback_payload,
};
#[cfg(target_os = "windows")]
pub use windows_test_dispatch::{
    IID_OXVBA_TEST_DISPATCH_EVENTS, IID_OXVBA_TEST_DISPATCH_EVENTS_STR,
    IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS, IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR,
    OXVBA_TEST_DISPATCH_PROGID, TEST_DISPID_COUNT, TEST_DISPID_ECHO_VARIANT,
    TEST_DISPID_EXCEL_QUIT, TEST_DISPID_EXISTS, TEST_DISPID_FIRE_CHANGED,
    TEST_DISPID_FIRE_CHANGED_PAIR, TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE, TEST_DISPID_LOOKUP,
    TEST_DISPID_LOOKUP_PAIR, TEST_DISPID_PING, TEST_DISPID_RAISE_EXCEPTION,
    TEST_DISPID_RETURN_SMALLINT, TEST_DISPID_RETURN_UNSIGNED_WORD, TEST_DISPID_SET_INDEXED_VALUE,
    TEST_DISPID_SET_INDEXED_VALUE_REF, TEST_DISPID_SET_VALUE, TEST_DISPID_SET_VALUE_REF,
    TEST_DISPID_SUM_PAIR, TEST_DISPID_VALUE, TEST_EVENT_CHANGED, TEST_EVENT_CHANGED_PAIR,
    TEST_NAMED_DISPID_INDEX, TEST_NAMED_DISPID_LHS, TEST_NAMED_DISPID_RHS, TEST_NAMED_DISPID_VALUE,
    create_oxvba_test_dispatch, map_com_hresult_label, raw_oxvba_test_dispatch_vtable_invoke,
};
#[cfg(target_os = "windows")]
pub use windows_variant::{
    VariantResultValue, set_variant_from_com_value, take_variant_result_value, variant_to_com_value,
};

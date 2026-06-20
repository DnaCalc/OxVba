//! oxvba-com: Windows-first COM bridge, metadata facade, and wire translation boundary.

pub mod ansi;
pub mod compat;
pub mod cycle_gc;
pub mod dispatch;
pub mod dynamic_object;
#[cfg(any(test, feature = "fixture-typelibs"))]
#[path = "fixtures/typelib_catalog.rs"]
mod fixture_typelib_catalog;
pub mod invoke_policy;
pub mod miri_variant_mock;
pub mod model;
pub mod platform;
pub mod refcount;
pub mod runtime_state;
pub mod stub_bridge;
pub mod typelib;
pub mod typelib_cache;
pub mod typelib_catalog;
#[cfg(target_os = "windows")]
pub mod windows_bridge;
#[cfg(target_os = "windows")]
pub mod windows_call_frame;
pub mod windows_client;
#[cfg(target_os = "windows")]
pub mod windows_connection_point;
#[cfg(not(target_arch = "wasm32"))]
pub mod windows_ffi_bridge;
#[cfg(target_os = "windows")]
pub mod windows_invoke;
#[cfg(target_os = "windows")]
pub mod windows_runtime_state;
#[cfg(all(target_os = "windows", any(test, feature = "fixture-typelibs")))]
#[path = "fixtures/windows_test_dispatch.rs"]
pub mod windows_test_dispatch;
pub mod windows_typelib_loader;
#[cfg(target_os = "windows")]
pub mod windows_variant;
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub mod windows_vtable;

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
    ComCallbackPayload, ComCallbackToken, ComCallbackValue, ComInvokeArg, ComInvokeKind,
    ComInvokeRequest, ComMemberToken, ComObjectDescriptor, ComObjectToken, ComObjectTransportKind,
    ComSubscriptionToken, ComValue, DISPATCH_INVOKE_MISSING_ARG_TOKEN,
};
pub use platform::portable::PortableComProjection;
pub use refcount::RefCount;
pub use runtime_state::{
    ComBinding, ComDirectDispatchSpec, ComEventCallback, ComEventPath, ComEventSpec,
    ComEventSubscription, ComEventTriggerSpec, ComMemberSpec, ComRuntimeState,
    binding_from_typelib_metadata,
};
pub use typelib::{
    ComDefaultValue, ComInterfaceIid, OptionalParamDefault, SourceTypeKind, TypeLibCacheScope,
    TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibInterfaceMetadata,
    TypeLibMemberInvokeKind, TypeLibMemberMetadata, TypeLibMetadataBlob, TypeLibParamType,
    TypeLibRecordInfo, TypeLibResolveRequest, TypeLibResolvedIdentity, TypeLibWireType,
    runtime_class_descriptor_from_typelib_metadata,
};
pub use typelib_cache::TypeLibMetadataCacheState;
pub(crate) use typelib_catalog::map_member_metadata_to_spec;
pub use typelib_catalog::{
    TypeLibMemberLookupResult, activation_prog_id_from_typelib_metadata, build_typelib_metadata,
    event_spec_from_typelib_metadata, known_typelib_identity_for_prog_id_name,
    member_spec_from_typelib_metadata, member_token_and_spec_from_typelib_metadata_name,
    resolve_default_member_token_and_spec_from_typelib_metadata, resolve_known_typelib_identity,
    resolve_member_token_and_spec_from_typelib_metadata_name,
    resolve_typelib_identity_for_prog_id_name, resolve_typelib_interface_metadata,
    resolve_typelib_interface_metadata_from_identity, source_interface_event_spec_supported,
};
#[cfg(target_os = "windows")]
pub use windows_bridge::{WindowsComBridge, WindowsComBridgeDispatchError};
#[cfg(target_os = "windows")]
pub use windows_call_frame::{
    ComCallFrameMarshalError, apply_runtime_call_writebacks_to_disp_params,
    disp_params_to_runtime_call_frame, logical_arg_index_to_com_arg_index,
    runtime_call_error_to_excepinfo, runtime_call_result_to_variant,
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
    activate_runtime_binding, activate_runtime_dispatch, activate_runtime_object_binding_shared,
    add_ref_dispatch, get_dispid_by_name, get_dispids_by_names, guid_equals, parse_guid_canonical,
    query_dispatch_from_unknown, query_interface_pointer, query_unknown_from_dispatch,
    release_connection_point, release_dispatch, release_unknown, resolve_named_argument_dispids,
};
#[cfg(target_os = "windows")]
pub use windows_connection_point::{
    DispatchEventSinkConfig, RawSingleI32SourceEvents, WindowsConnectionPointTransport,
    try_advise_dispatch_event_sink, try_advise_single_i32_source_interface_event_sink,
    unadvise_connection_point,
};
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub use windows_invoke::try_vtable_member_spec_invoke_with_shared_state;
#[cfg(target_os = "windows")]
pub use windows_invoke::{
    COM_DISPID_NEWENUM, ComInvokeExceptionInfo, ComInvokeFailure, ComTransportCounters,
    enumerate_object_with_shared_state, execute_bound_runtime_call_result,
    execute_bound_runtime_call_result_with_shared_state, execute_bound_variant,
    execute_bound_variant_with_shared_state, invoke_bound_dispatch_legacy_i32_result,
    invoke_bound_dispatch_variant_with_shared_state, invoke_direct_dispid_variant,
    invoke_direct_dispid_variant_with_shared_state, invoke_dispatch_legacy_i32_result,
    invoke_dispatch_legacy_i32_result_positional, invoke_dispatch_variant,
    invoke_dispatch_variant_with_shared_state, invoke_member_spec_legacy_i32_result,
    invoke_member_spec_variant, invoke_member_spec_variant_with_shared_state,
    map_com_hresult_label, map_com_hresult_vba_number, take_excepinfo,
    vba_number_from_dispatch_codes,
};
#[cfg(target_os = "windows")]
pub use windows_runtime_state::{
    ReleasedWindowsComObject, WindowsComClientState, WindowsComSubscriptionTransport,
    advise_event_subscription, bind_host_dispatch_object, bind_host_dispatch_object_shared,
    bind_native_dispatch_result, bind_native_dispatch_result_shared, cache_member_dispid,
    callback_arg, callback_arity, callback_subscription_token,
    collect_stale_callbacks_for_subscription, event_callback_args_from_invoke_args,
    event_is_source_interface_only, event_signature_arity_for_binding,
    host_dispatch_object_for_prog_id, host_dispatch_object_for_prog_id_shared,
    insert_bound_object_binding, insert_bound_object_binding_at_handle,
    insert_bound_object_binding_at_handle_shared, insert_bound_object_binding_shared,
    mark_next_callback_pumped_shared, queue_projection_event_callbacks_shared, release_callback,
    release_object_binding, release_object_binding_shared, release_subscription_transport,
    remove_subscription_callbacks, resolve_bound_native_dispatch,
    resolve_bound_native_dispatch_shared, resolve_bound_runtime_object,
    resolve_bound_runtime_object_shared, resolve_member_dispid_cached,
    resolve_subscription_transport, subscribe_event_shared, take_polled_callback_payload,
    unsubscribe_event_shared,
};
#[cfg(all(
    target_os = "windows",
    target_arch = "x86_64",
    any(test, feature = "fixture-typelibs")
))]
pub use windows_test_dispatch::{
    DUAL_BYTE_VALUE, DUAL_CREATED_OLE_DATE, DUAL_DOUBLE_VALUE, DUAL_FIXTURE_INTERFACE_IID,
    DUAL_INTEGER_VALUE, DUAL_LONGLONG_VALUE, DUAL_PRICE_SCALED_I64, DUAL_RECORD_RETURN_VALUE,
    DUAL_SINGLE_VALUE, DUAL_SLOT_EXISTS, DUAL_SLOT_GET_BYTE_VALUE, DUAL_SLOT_GET_COUNT,
    DUAL_SLOT_GET_CREATED, DUAL_SLOT_GET_DOUBLE_VALUE, DUAL_SLOT_GET_I4_SAFEARRAY_VALUE,
    DUAL_SLOT_GET_INTEGER_VALUE, DUAL_SLOT_GET_LONGLONG_VALUE, DUAL_SLOT_GET_OWNER,
    DUAL_SLOT_GET_PRICE, DUAL_SLOT_GET_RECORD_VALUE, DUAL_SLOT_GET_SINGLE_VALUE,
    DUAL_SLOT_GET_TEXT_VALUE, DUAL_SLOT_GET_VARIANT_VALUE, DUAL_SLOT_LOOKUP, DUAL_SLOT_PUT_VALUE,
    DUAL_SLOT_PUTREF_LONG_VALUE, DUAL_SLOT_PUTREF_OBJECT_VALUE, DUAL_SLOT_RAISE_ERROR,
    DUAL_SLOT_VALIDATE_ALL_INPUTS, DUAL_SLOT_VALIDATE_I4_SAFEARRAY_VALUE, DUAL_TEXT_VALUE,
    DUAL_VARIANT_VALUE, IID_OXVBA_DUAL_FIXTURE, create_oxvba_dual_vtable_object,
};
#[cfg(all(target_os = "windows", any(test, feature = "fixture-typelibs")))]
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
    create_oxvba_test_dispatch, raw_oxvba_test_dispatch_vtable_invoke,
};
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub use windows_typelib_loader::live_member_metadata_from_dispatch;
#[cfg(target_os = "windows")]
pub use windows_typelib_loader::live_object_typeinfo_name;
#[cfg(target_os = "windows")]
pub use windows_variant::{
    VariantResultValue, set_variant_from_com_value, take_variant_result_value,
    take_variant_result_variant, variant_to_com_value, variant_to_variant_value,
};

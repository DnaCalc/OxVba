//! oxvba-com: COM abstraction scaffolding.

pub mod cycle_gc;
pub mod dispatch;
pub mod dynamic_object;
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
pub mod windows_variant;

pub use dispatch::{ComDispatch, DispatchResult};
pub use dynamic_object::{
    DynamicCallArg, DynamicCallKind, DynamicCallRequest, DynamicCallbackToken, DynamicEventPayload,
    DynamicMemberSelector, DynamicObjectBridge, DynamicObjectToken, DynamicSubscriptionToken,
    DynamicValue,
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
};
pub use typelib::{
    TypeLibCacheScope, TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind,
    TypeLibMemberMetadata, TypeLibMetadataBlob, TypeLibResolveRequest, TypeLibResolvedIdentity,
};
pub use typelib_cache::TypeLibMetadataCacheState;
pub use typelib_catalog::{
    build_typelib_metadata, known_typelib_identity_for_prog_id_name, resolve_known_typelib_identity,
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
    add_ref_dispatch, get_dispid_by_name, get_dispids_by_names, guid_equals, parse_guid_canonical,
    query_dispatch_from_unknown, release_connection_point, release_dispatch, release_unknown,
};
#[cfg(target_os = "windows")]
pub use windows_connection_point::{
    DispatchEventSinkConfig, WindowsConnectionPointTransport, try_advise_dispatch_event_sink,
    unadvise_connection_point,
};
#[cfg(target_os = "windows")]
pub use windows_invoke::{ComInvokeExceptionInfo, ComInvokeFailure, take_excepinfo};
#[cfg(target_os = "windows")]
pub use windows_variant::{
    VariantResultValue, set_variant_from_com_value, take_variant_result_value, variant_to_com_value,
};

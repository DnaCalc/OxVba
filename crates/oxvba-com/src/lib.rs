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
pub use windows_invoke::{ComInvokeExceptionInfo, ComInvokeFailure, take_excepinfo};
#[cfg(target_os = "windows")]
pub use windows_variant::{
    VariantResultValue, set_variant_from_com_value, take_variant_result_value, variant_to_com_value,
};

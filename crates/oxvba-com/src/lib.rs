//! oxvba-com: COM abstraction scaffolding.

pub mod cycle_gc;
pub mod dispatch;
pub mod model;
pub mod platform;
pub mod refcount;
pub mod typelib;
pub mod typelib_catalog;

pub use dispatch::{ComDispatch, DispatchResult};
pub use model::{
    ComCallbackPayload, ComCallbackToken, ComInvokeArg, ComInvokeKind, ComInvokeRequest,
    ComObjectDescriptor, ComObjectToken, ComObjectTransportKind, ComSubscriptionToken,
    DISPATCH_INVOKE_MISSING_ARG_TOKEN,
};
pub use refcount::RefCount;
pub use typelib::{
    TypeLibCacheScope, TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind,
    TypeLibMemberMetadata, TypeLibMetadataBlob, TypeLibResolveRequest, TypeLibResolvedIdentity,
};
pub use typelib_catalog::{
    build_typelib_metadata, known_typelib_identity_for_prog_id_name, resolve_known_typelib_identity,
};

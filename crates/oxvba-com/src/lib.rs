//! oxvba-com: COM abstraction scaffolding.

pub mod cycle_gc;
pub mod dispatch;
pub mod model;
pub mod platform;
pub mod refcount;

pub use dispatch::{ComDispatch, DispatchResult};
pub use model::{
    ComCallbackPayload, ComCallbackToken, ComObjectToken, ComSubscriptionToken,
    DISPATCH_INVOKE_MISSING_ARG_TOKEN,
};
pub use refcount::RefCount;

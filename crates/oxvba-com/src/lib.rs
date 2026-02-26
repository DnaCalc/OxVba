//! oxvba-com: COM abstraction scaffolding.

pub mod cycle_gc;
pub mod dispatch;
pub mod platform;
pub mod refcount;

pub use dispatch::{ComDispatch, DispatchResult};
pub use refcount::RefCount;

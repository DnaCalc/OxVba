//! Built-in `VBA.Collection` instance state.
//!
//! The data model now lives in `oxvba-runtime` (carried on the object box's
//! `native_state` slot) so vm2, vm3, and the JIT share ONE implementation. This module
//! re-exports it so the long-standing `oxvba_eval::collection::*` import path keeps
//! resolving. The keyed-method *dispatch* shim lives here in `oxvba-eval`
//! (`dispatch_collection`), beside the rest of the shared value/builtin kernel.

pub use oxvba_runtime::collection::{CollectionData, CollectionError, Selector};

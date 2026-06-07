use oxvba_debug::{DebugSessionCore, DebugSessionHandle};
use static_assertions::{assert_impl_all, assert_not_impl_any};

#[test]
fn debug_session_handle_is_send_sync_clone() {
    assert_impl_all!(DebugSessionHandle: Send, Sync, Clone);
}

#[test]
fn debug_session_core_is_not_send_or_sync() {
    assert_not_impl_any!(DebugSessionCore: Send, Sync);
}

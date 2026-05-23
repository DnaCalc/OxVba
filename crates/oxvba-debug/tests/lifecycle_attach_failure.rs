#[path = "support_handle/mod.rs"]
mod support_handle;

use std::sync::Arc;

use oxvba_debug::{DebugAttachConfig, DebugAttachError, attach_debug_session};
use oxvba_host::{Engine, HostConfig};

#[test]
fn bad_manifest_returns_attach_error_without_worker_leak() {
    let manifest = support_handle::make_manifest("Sub Main()\nDim x As UnknownType\nx =\nEnd Sub");
    let err = attach_debug_session(
        Arc::new(Engine::new(HostConfig::default())),
        manifest,
        DebugAttachConfig::default(),
    )
    .expect_err("bad manifest should fail attach");
    assert!(matches!(
        err,
        DebugAttachError::Prepare { .. } | DebugAttachError::Compile { .. }
    ));

    // A fresh attach after the failed prepare proves no poisoned global worker state remains.
    let attach = support_handle::attach(support_handle::call_manifest());
    attach.handle.detach().expect("fresh detach");
}

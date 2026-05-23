#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugAttachConfig, DebugComApartment, DebugError};

#[test]
fn com_uninitialize_runs_on_worker_shutdown() {
    let attach = support_handle::attach_with_config(
        support_handle::call_manifest(),
        DebugAttachConfig {
            com_apartment: DebugComApartment::None,
            ..DebugAttachConfig::default()
        },
    );
    let before = attach
        .handle
        .report_worker_apartment()
        .expect("before detach");
    assert_eq!(before.configured, DebugComApartment::None);
    attach.handle.detach().expect("detach");

    let attach = support_handle::attach_with_config(
        support_handle::call_manifest(),
        DebugAttachConfig {
            com_apartment: DebugComApartment::None,
            ..DebugAttachConfig::default()
        },
    );
    let clone = attach.handle.clone();
    let err = attach
        .handle
        .detach()
        .expect_err("outstanding clone blocks shutdown");
    assert!(matches!(err, DebugError::OutstandingHandles { .. }));
    clone
        .detach()
        .expect("clone can detach after original consumed");
}

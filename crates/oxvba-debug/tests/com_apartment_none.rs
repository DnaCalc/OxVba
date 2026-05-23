#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugAttachConfig, DebugComApartment, DebugWorkerApartmentKind};

#[test]
fn none_mode_does_not_initialize_com_and_runs_cross_platform() {
    let config = DebugAttachConfig {
        com_apartment: DebugComApartment::None,
        ..DebugAttachConfig::default()
    };
    let attach = support_handle::attach_with_config(support_handle::call_manifest(), config);
    let report = attach
        .handle
        .report_worker_apartment()
        .expect("apartment report");
    assert_eq!(report.configured, DebugComApartment::None);
    assert!(!report.initialized_by_worker);
    assert_eq!(report.observed, DebugWorkerApartmentKind::None);
    attach.handle.start().expect("debugger still runs");
    attach.handle.detach().expect("detach");
}

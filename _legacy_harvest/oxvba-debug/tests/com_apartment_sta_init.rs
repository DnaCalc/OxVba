#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugAttachConfig, DebugComApartment, DebugWorkerApartmentKind};

#[test]
fn worker_reports_sta_when_configured_sta() {
    let config = DebugAttachConfig {
        com_apartment: DebugComApartment::Sta,
        ..DebugAttachConfig::default()
    };
    let attach = support_handle::attach_with_config(support_handle::call_manifest(), config);
    let report = attach
        .handle
        .report_worker_apartment()
        .expect("apartment report");
    assert_eq!(report.configured, DebugComApartment::Sta);
    #[cfg(target_os = "windows")]
    {
        assert!(report.initialized_by_worker);
        assert_eq!(report.observed, DebugWorkerApartmentKind::Sta);
    }
    #[cfg(not(target_os = "windows"))]
    {
        assert!(!report.initialized_by_worker);
        assert_eq!(report.observed, DebugWorkerApartmentKind::None);
    }
    attach.handle.detach().expect("detach");
}

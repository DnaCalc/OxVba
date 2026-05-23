#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugAttachConfig, DebugComApartment};

#[test]
fn multiple_sessions_have_independent_apartments() {
    let first = support_handle::attach_with_config(
        support_handle::call_manifest(),
        DebugAttachConfig {
            com_apartment: DebugComApartment::None,
            ..DebugAttachConfig::default()
        },
    );
    let second = support_handle::attach_with_config(
        support_handle::call_manifest(),
        DebugAttachConfig {
            com_apartment: DebugComApartment::Mta,
            ..DebugAttachConfig::default()
        },
    );
    assert_eq!(
        first.handle.report_worker_apartment().unwrap().configured,
        DebugComApartment::None
    );
    assert_eq!(
        second.handle.report_worker_apartment().unwrap().configured,
        DebugComApartment::Mta
    );
    first.handle.detach().expect("detach first");
    second.handle.detach().expect("detach second");
}

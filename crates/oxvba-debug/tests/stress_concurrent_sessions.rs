#[path = "support_handle/mod.rs"]
mod support_handle;

use std::{sync::mpsc, time::Duration};

#[test]
fn one_hundred_concurrent_sessions_complete_without_crosstalk_or_leak() {
    let (tx, rx) = mpsc::channel();
    for index in 0..100 {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| {
                let handle = support_handle::attach(support_handle::multi_module_manifest()).handle;
                let session_id = handle.session_id().clone();
                let pause = handle.start().expect("start");
                let breakpoints = handle.breakpoints().expect("breakpoints");
                handle.detach().expect("detach");
                (index, session_id, pause, breakpoints.len())
            });
            let _ = tx.send(result);
        });
    }
    drop(tx);

    let mut session_ids = std::collections::BTreeSet::new();
    for _ in 0..100 {
        let (index, session_id, _pause, breakpoint_count) = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("concurrent session should not deadlock")
            .expect("concurrent session should not panic");
        assert_eq!(
            breakpoint_count, 0,
            "session {index} should not see cross-talk breakpoints"
        );
        assert!(
            session_ids.insert(session_id),
            "session ids should be unique"
        );
    }
}

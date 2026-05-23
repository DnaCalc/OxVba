#[path = "support_handle/mod.rs"]
mod support_handle;

use std::time::{Duration, Instant};

#[test]
fn bench_step_into_round_trip() {
    let handle = support_handle::attach(support_handle::multi_module_manifest()).handle;
    let _ = handle.start().expect("start");
    let start = Instant::now();
    let _ = handle.step_into().expect("step into");
    assert!(start.elapsed() < Duration::from_secs(1));
    let _ = handle.detach();
}

#[test]
fn bench_set_source_breakpoint_round_trip() {
    let handle = support_handle::attach(support_handle::multi_module_manifest()).handle;
    let start = Instant::now();
    let bp = handle
        .set_source_breakpoint("Module1", 2, true)
        .expect("set breakpoint");
    assert!(start.elapsed() < Duration::from_secs(1));
    let id = oxvba_host::DirectHostBreakpointId::new(bp.id);
    handle.clear_source_breakpoint(&id).expect("clear");
    handle.detach().expect("detach");
}

#[test]
fn bench_evaluate_watches_round_trip() {
    let handle = support_handle::attach(support_handle::multi_module_manifest()).handle;
    let _ = handle.start().expect("start");
    let watch = handle.add_watch("y").expect("watch");
    let start = Instant::now();
    let _ = handle.evaluate_watches().expect("evaluate watches");
    assert!(start.elapsed() < Duration::from_secs(1));
    let id = oxvba_host::DirectHostWatchId::new(watch.id);
    handle.remove_watch(&id).expect("remove watch");
    handle.detach().expect("detach");
}

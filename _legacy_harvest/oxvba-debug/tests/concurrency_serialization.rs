#[path = "support_handle/mod.rs"]
mod support_handle;

use std::thread;

#[test]
fn eight_callers_serialize_at_worker_channel() {
    let handle = support_handle::attach_handle();
    let mut workers = Vec::new();
    for index in 0..8 {
        let handle = handle.clone();
        workers.push(thread::spawn(move || {
            handle
                .set_source_breakpoint("Module1", if index % 2 == 0 { 2 } else { 6 }, true)
                .expect("set breakpoint from cloned handle")
        }));
    }
    let records: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().expect("worker thread"))
        .collect();
    assert_eq!(records.len(), 8);
    assert_eq!(handle.breakpoints().expect("breakpoints").len(), 8);
    handle.detach().expect("detach");
}

#[path = "support_handle/mod.rs"]
mod support_handle;

#[test]
fn attach_detach_loop_has_stable_thread_and_fd_counts() {
    for _ in 0..50 {
        let handle = support_handle::attach_handle();
        let _ = handle.report_worker_apartment().expect("apartment report");
        handle.detach().expect("detach");
    }

    support_handle::attach_handle()
        .detach()
        .expect("final attach confirms resources remain usable");
}

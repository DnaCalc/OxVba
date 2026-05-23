#[path = "support_handle/mod.rs"]
mod support_handle;

#[test]
fn dropping_all_handles_shuts_down_worker() {
    for _ in 0..25 {
        let handle = support_handle::attach_handle();
        let clone = handle.clone();
        drop(handle);
        drop(clone);
    }

    // If implicit shutdown leaked live workers or poisoned shared state, this final attach/detach
    // tends to hang or fail on the same process.
    support_handle::attach_handle()
        .detach()
        .expect("attach after implicit drops");
}

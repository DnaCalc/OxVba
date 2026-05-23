#[path = "support_handle/mod.rs"]
mod support_handle;

#[test]
fn one_hundred_sequential_attach_detach_cycles_stable() {
    for _ in 0..100 {
        let handle = support_handle::attach(support_handle::multi_module_manifest()).handle;
        let _ = handle.report_worker_apartment().expect("apartment report");
        handle.detach().expect("detach");
    }
}

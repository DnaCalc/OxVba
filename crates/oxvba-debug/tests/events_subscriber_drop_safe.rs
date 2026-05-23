#[path = "support_handle/mod.rs"]
mod support_handle;

#[test]
fn dropping_subscriber_does_not_poison_worker() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let subscriber = attach.handle.subscribe();
    drop(subscriber);
    let breakpoint = attach
        .handle
        .set_source_breakpoint("Module1", 5, true)
        .expect("set breakpoint after subscriber drop");
    assert_eq!(breakpoint.file_line, 5);
    attach.handle.detach().expect("detach");
}

#[path = "support_handle/mod.rs"]
mod support_handle;

#[test]
fn attach_returns_handle_and_initial_receiver() {
    let attach = support_handle::attach(support_handle::call_manifest());
    assert!(
        attach
            .handle
            .session_id()
            .as_str()
            .contains("DebugHandleCatalog")
    );
    let _events = attach.events;
    attach.handle.detach().expect("detach cleanly");
}

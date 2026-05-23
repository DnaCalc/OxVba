#[path = "support_handle/mod.rs"]
mod support_handle;

#[test]
fn reattach_after_detach_has_fresh_session_id_and_state() {
    let first = support_handle::attach_handle();
    let first_id = first.session_id().clone();
    first.detach().expect("first detach");

    let second = support_handle::attach_handle();
    let second_id = second.session_id().clone();
    assert_ne!(
        first_id, second_id,
        "reattach should allocate a fresh debug session id"
    );
    assert!(second.current_pause().expect("pause query").is_none());
    second.detach().expect("second detach");
}

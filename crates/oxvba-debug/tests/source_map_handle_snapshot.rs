#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugRunResultView;

#[test]
fn thin_slice_file_lines_bind_through_handle() {
    let manifest = support_handle::make_manifest(
        "Attribute VB_Name = \"Module1\"\n\
         Sub Main()\n\
         Call Foo(4)\n\
         End Sub\n\
         \n\
         Sub Foo(ByVal y As Long)\n\
         Dim z As Long\n\
         z = y + 1\n\
         End Sub",
    );
    let attach = support_handle::attach(manifest);
    let breakpoint = attach
        .handle
        .set_source_breakpoint("Module1", 6, true)
        .expect("set breakpoint on editor line");
    assert_eq!(breakpoint.file_line, 6);
    let _ = attach.handle.start().expect("start");
    let result = attach.handle.continue_execution().expect("continue");
    let DebugRunResultView::Paused(pause) = result else {
        panic!("expected breakpoint pause");
    };
    assert_eq!(pause.current_location.expect("location").file_line, 6);
    attach.handle.detach().expect("detach");
}

//! M3-7 — native `Declare` lane on **vm3** (the sole runtime).
//!
//! The companion `native_declare_lane.rs` proves vm3 drives a real `Declare Lib` call
//! through `LoadLibrary`/`GetProcAddress` under `interactive_dev` (native mode) across the
//! scalar/ByRef/string/pointer/UDT/vtable shapes. This file pins the `AddressOf` callback
//! thunk shape that real VBA code uses when passing a procedure pointer into synchronous
//! native callback APIs such as `CallWindowProcW`.
#![cfg(target_os = "windows")]

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig, Vm3Snapshot};

fn engine() -> Engine {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
}

#[test]
fn vm3_invokes_address_of_native_callback_shape() {
    // Riff-style code passes `AddressOf` as a `LongPtr` callback pointer. `CallWindowProcW`
    // gives us a synchronous four-argument Windows callback probe without installing a
    // timer or retaining the thunk after the Declare call returns.
    let source = "Private Declare PtrSafe Function RiffCallPtr4 Lib \"user32\" Alias \"CallWindowProcW\" (ByVal lpPrevWndFunc As LongPtr, ByVal a0 As LongPtr, ByVal a1 As LongPtr, ByVal a2 As LongPtr, ByVal a3 As LongPtr) As LongPtr\n\
         Public CallbackHwnd As LongLong\n\
         Public CallbackMsg As Long\n\
         Public CallbackId As LongLong\n\
         Public CallbackTime As Long\n\
         Sub Main()\n\
         Dim ignored As LongPtr\n\
         ignored = RiffCallPtr4(AddressOf RiffTimerLikeCallback, 11, 22, 33, 44)\n\
         End Sub\n\
         Sub RiffTimerLikeCallback(ByVal hWnd As LongPtr, ByVal uMsg As Long, ByVal idEvent As LongPtr, ByVal dwTime As Long)\n\
         CallbackHwnd = hWnd\n\
         CallbackMsg = uMsg\n\
         CallbackId = idEvent\n\
         CallbackTime = dwTime\n\
         End Sub";
    match engine().execute_source_with_variant_snapshot_vm3(source) {
        Vm3Snapshot::Ran(snapshot) => {
            assert!(
                snapshot.iter().any(|v| v.as_i64() == Some(11)),
                "expected CallWindowProcW to pass hwnd/a0 to the AddressOf callback: {snapshot:?}"
            );
            assert!(
                snapshot.iter().any(|v| v.as_i32() == Some(22)),
                "expected CallWindowProcW to pass msg/a1 to the AddressOf callback: {snapshot:?}"
            );
            assert!(
                snapshot.iter().any(|v| v.as_i64() == Some(33)),
                "expected CallWindowProcW to pass id/a2 to the AddressOf callback: {snapshot:?}"
            );
            assert!(
                snapshot.iter().any(|v| v.as_i32() == Some(44)),
                "expected CallWindowProcW to invoke the AddressOf callback with four native args: {snapshot:?}"
            );
        }
        Vm3Snapshot::Unsupported(what) => {
            panic!("vm3 should run the AddressOf native-callback shape, got unsupported {what:?}")
        }
        Vm3Snapshot::Failed(msg) => {
            panic!("vm3 should run the AddressOf native-callback shape, failed with {msg:?}")
        }
    }
}

#[test]
fn vm3_declines_address_of_non_callback_declare_shape_before_ffi() {
    let source = "Private Declare PtrSafe Function NotACallback Lib \"kernel32\" Alias \"GetTickCount64\" (ByVal callback As LongPtr) As LongLong\n\
         Sub Main()\n\
         Dim ignored As LongLong\n\
         ignored = NotACallback(AddressOf RiffTimerLikeCallback)\n\
         End Sub\n\
         Sub RiffTimerLikeCallback(ByVal hWnd As LongPtr, ByVal uMsg As Long, ByVal idEvent As LongPtr, ByVal dwTime As Long)\n\
         End Sub";
    match engine().execute_source_with_variant_snapshot_vm3(source) {
        Vm3Snapshot::Unsupported(what) => assert!(
            what.contains("AddressOf proc passed to a non-synchronous Declare callback parameter"),
            "expected non-callback Declare to reject AddressOf before FFI, got {what:?}"
        ),
        other => {
            panic!("vm3 should reject the non-callback AddressOf Declare shape, got {other:?}")
        }
    }
}

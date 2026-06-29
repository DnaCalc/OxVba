//! M3-7 — native `Declare` lane on **vm3** (the sole runtime).
//!
//! The companion `native_declare_lane.rs` proves vm3 drives a real `Declare Lib` call
//! through `LoadLibrary`/`GetProcAddress` under `interactive_dev` (native mode) across the
//! scalar/ByRef/string/pointer/UDT/vtable shapes. This file pins the one shape M3-7 leaves
//! to a follow-up: an `AddressOf` proc marshaled into a native callback slot, which vm3
//! **honestly declines** (a named `Unsupported`) rather than mis-marshaling the proc
//! reference as a bogus integer. (vm2, now retired, fully supported this shape; it is a
//! deliberate, documented vm3-only residual that needs a VM-bound thread-local callback-thunk
//! table.)
#![cfg(target_os = "windows")]

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig, Vm3Snapshot};

fn engine() -> Engine {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
}

#[test]
fn vm3_declines_address_of_native_callback_shape() {
    // An `AddressOf` proc marshaled into a native callback slot (a `LongPtr` parameter)
    // needs a thread-local callback-thunk table bound to the VM — platform machinery
    // M3-7 leaves to a follow-up. vm3 must decline it *honestly* (a named Unsupported),
    // never marshal the proc reference as a bogus integer.
    let source =
        "Private Declare PtrSafe Function RiffCallPtr4 Lib \"user32\" Alias \"CallWindowProcW\" (ByVal lpPrevWndFunc As LongPtr, ByVal a0 As LongPtr, ByVal a1 As LongPtr, ByVal a2 As LongPtr, ByVal a3 As LongPtr) As LongPtr\n\
         Public CallbackHwnd As LongLong\n\
         Sub Main()\n\
         Dim ignored As LongPtr\n\
         ignored = RiffCallPtr4(AddressOf RiffTimerLikeCallback, 11, 22, 33, 44)\n\
         End Sub\n\
         Sub RiffTimerLikeCallback(ByVal hWnd As LongPtr, ByVal uMsg As Long, ByVal idEvent As LongPtr, ByVal dwTime As Long)\n\
         CallbackHwnd = hWnd\n\
         End Sub";
    match engine().execute_source_with_variant_snapshot_vm3(source) {
        Vm3Snapshot::Unsupported(what) => assert!(
            what.contains("AddressOf proc passed to a Declare callback parameter"),
            "expected the named callback-shape decline, got {what:?}"
        ),
        other => panic!("vm3 should decline the AddressOf native-callback shape, got {other:?}"),
    }
}

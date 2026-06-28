//! M3-7 — native `Declare` lane on **vm3**, differentialed against vm2.
//!
//! The companion `native_declare_lane.rs` proves the vm2 backend drives a real
//! `Declare Lib` call through `LoadLibrary`/`GetProcAddress` under `interactive_dev`
//! (native mode). This file proves vm3 produces the **same** observable on that same
//! native lane — a scalar return, a ByRef numeric write-back, a `ByVal As String` (ANSI)
//! argument, and a `VarPtr` pointer-helper round-trip — and that vm3 **honestly declines**
//! the one shape M3-7 leaves to a follow-up (an `AddressOf` proc marshaled into a native
//! callback slot) rather than mis-marshaling it.
//!
//! Both backends run through the one shared dynamic-link HAL contract, so the As-Any/UDT/
//! Variant marshalling shapes the vm2 file covers are inherited by vm3 unchanged; these
//! representative cases gate the vm3 wiring (argument marshalling, write-back routing,
//! `LastDllError`, pointer pins) that is new in M3-7.
#![cfg(target_os = "windows")]

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig, Vm3Snapshot};
use oxvba_runtime::Variant;

fn engine() -> Engine {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
}

/// Run `source` under vm2 (the golden oracle) on the native-dynamic-link policy.
fn run_vm2(source: &str) -> Vec<Variant> {
    engine()
        .execute_source_with_variant_snapshot_clean(source)
        .expect("native declare probe should execute on vm2")
}

/// Run `source` under vm3 on the same policy, requiring it to complete.
fn run_vm3(source: &str) -> Vec<Variant> {
    match engine().execute_source_with_variant_snapshot_vm3(source) {
        Vm3Snapshot::Ran(values) => values,
        other => panic!("native declare probe should run on vm3, got {other:?}"),
    }
}

/// Assert both backends agree that some snapshot slot satisfies `pred` — the
/// pointer-agnostic differential check (raw pointer slots differ run-to-run; the
/// meaningful native results do not).
fn both_have(source: &str, pred: impl Fn(&Variant) -> bool, what: &str) {
    let vm2 = run_vm2(source);
    let vm3 = run_vm3(source);
    assert!(
        vm2.iter().any(&pred),
        "vm2 missing {what} in {vm2:?}"
    );
    assert!(
        vm3.iter().any(&pred),
        "vm3 missing {what} in {vm3:?} (vm2 had it: {vm2:?})"
    );
}

#[test]
fn vm3_scalar_double_declare_round_trips_through_native_ffi() {
    // msvcrt `sqrt` — a pure ByVal Double in / Double out scalar call.
    both_have(
        "Private Declare PtrSafe Function NativeSqrt Lib \"msvcrt\" Alias \"sqrt\" (ByVal x As Double) As Double\n\
         Sub Main()\n\
         Dim result As Double\n\
         result = NativeSqrt(156.25)\n\
         End Sub",
        |v| v.as_f64().is_some_and(|n| (n - 12.5).abs() < 1e-9),
        "sqrt(156.25)=12.5",
    );
}

#[test]
fn vm3_byref_numeric_declare_writes_back() {
    // oleaut32 `VarR8FromI4(Long in, ByRef Double out)` — ByRef numeric write-back.
    both_have(
        "Private Declare PtrSafe Function VarR8FromI4 Lib \"oleaut32\" (ByVal inVal As Long, ByRef outVal As Double) As Long\n\
         Sub Main()\n\
         Dim outVal As Double\n\
         Dim status As Long\n\
         status = VarR8FromI4(42, outVal)\n\
         End Sub",
        |v| v.as_f64().is_some_and(|n| (n - 42.0).abs() < 1e-9),
        "VarR8FromI4 ByRef Double=42.0",
    );
}

#[test]
fn vm3_byval_ansi_string_declare_loads_library() {
    // kernel32 `LoadLibraryA(ByVal As String) As LongPtr` — ByVal ANSI string in, a
    // non-zero module handle out (then released). kernel32's handle is process-stable,
    // so both backends observe the same handle.
    both_have(
        "Private Declare PtrSafe Function NativeLoad Lib \"kernel32\" Alias \"LoadLibraryA\" (ByVal name As String) As LongPtr\n\
         Private Declare PtrSafe Function NativeFree Lib \"kernel32\" Alias \"FreeLibrary\" (ByVal hModule As LongPtr) As Long\n\
         Sub Main()\n\
         Dim handle As LongPtr\n\
         handle = NativeLoad(\"kernel32\")\n\
         Dim freed As Long\n\
         freed = NativeFree(handle)\n\
         End Sub",
        |v| v.as_i64().is_some_and(|n| n != 0),
        "non-zero module handle from LoadLibraryA",
    );
}

#[test]
fn vm3_varptr_memmove_round_trips_scalars() {
    // C runtime `memmove` via `VarPtr` pins — exercises the pointer-helper register +
    // free path and ByVal LongPtr marshalling on vm3.
    both_have(
        "Private Declare PtrSafe Function MemMove Lib \"msvcrt\" Alias \"memmove\" (ByVal Destination As LongPtr, ByVal Source As LongPtr, ByVal Length As LongPtr) As LongPtr\n\
         Sub Main()\n\
         Dim src As Long\n\
         Dim copied As Long\n\
         Dim ret As LongPtr\n\
         src = &H11223344\n\
         ret = MemMove(VarPtr(copied), VarPtr(src), 4)\n\
         End Sub",
        |v| v.as_i32() == Some(0x11223344),
        "memmove copied scalar through VarPtr pins",
    );
}

#[test]
fn vm3_strptr_writeback_round_trips_through_native_call() {
    // `StrPtr` over an l-value, consumed by a native call that mutates the buffer in
    // place: `CharUpperBuffW(StrPtr(s), Len(s))` upper-cases the string's wide buffer,
    // and the pointer-helper write-back reads the mutation back into `s`. Proves the
    // `Ptr { kind: Str }` pin + the `StrPtr` read-back path on vm3.
    both_have(
        "Private Declare PtrSafe Function CharUpperBuffW Lib \"user32\" (ByVal lpsz As LongPtr, ByVal cchLength As Long) As Long\n\
         Sub Main()\n\
         Dim s As String\n\
         Dim changed As Long\n\
         Dim isUpper As Boolean\n\
         s = \"abc\"\n\
         changed = CharUpperBuffW(StrPtr(s), Len(s))\n\
         isUpper = (s = \"ABC\")\n\
         End Sub",
        |v| v.as_bool() == Some(true),
        "StrPtr buffer upper-cased and read back (s = \"ABC\")",
    );
}

#[test]
fn vm3_declines_address_of_native_callback_shape() {
    // An `AddressOf` proc marshaled into a native callback slot (a `LongPtr` parameter)
    // needs a thread-local callback-thunk table bound to the VM — platform machinery
    // M3-7 leaves to a follow-up. vm3 must decline it *honestly* (a named Unsupported),
    // never marshal the proc reference as a bogus integer. vm2 fully supports this shape,
    // so this is a deliberate, documented vm3-only residual.
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
    // vm2 supports it (sanity: the shape is valid and runs on the oracle).
    let _ = run_vm2(source);
    match engine().execute_source_with_variant_snapshot_vm3(source) {
        Vm3Snapshot::Unsupported(what) => assert!(
            what.contains("AddressOf proc passed to a Declare callback parameter"),
            "expected the named callback-shape decline, got {what:?}"
        ),
        other => panic!("vm3 should decline the AddressOf native-callback shape, got {other:?}"),
    }
}

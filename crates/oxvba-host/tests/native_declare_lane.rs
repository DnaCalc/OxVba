//! L1 — native `Declare` lane routing. With the binder emitting `marshal_lane =
//! "m1-native-ffi"` for real libraries, a `Declare Lib` call reaches the real
//! `LoadLibrary`/`GetProcAddress` FFI path under `interactive_dev` (native mode) on
//! Windows. Covers the non-pointer cases L1 unblocks: a scalar return, a ByRef
//! numeric write-back, and a `ByVal As String` (ANSI) argument. (Pointer-helper
//! arguments + write-back are L2/L3; full coverage is the re-instated
//! `native_declare_string_marshalling_end_to_end` + SQLiteForExcel suites.)
#![cfg(target_os = "windows")]

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};
use oxvba_runtime::{VarType, Variant};

/// Run a source module on the VM backend under the interactive-dev policy (which
/// enables native dynamic linking). The snapshot is the module globals + `Main`'s
/// locals.
fn run(source: &str) -> Vec<Variant> {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_source_with_variant_snapshot_clean(source)
        .expect("native declare probe should execute on the VM backend")
}

fn any_double_near(snapshot: &[Variant], expected: f64) -> bool {
    snapshot
        .iter()
        .filter(|v| matches!(v.vtype(), VarType::Double))
        .any(|v| (v.as_f64().unwrap_or(0.0) - expected).abs() < 1e-9)
}

fn any_nonzero_i64(snapshot: &[Variant]) -> bool {
    snapshot.iter().any(|v| v.as_i64().is_some_and(|n| n != 0))
}

#[test]
fn scalar_double_declare_round_trips_through_native_ffi() {
    // msvcrt `sqrt` — a pure ByVal Double in / Double out scalar call.
    let snapshot = run(
        "Private Declare PtrSafe Function NativeSqrt Lib \"msvcrt\" Alias \"sqrt\" (ByVal x As Double) As Double\n\
         Sub Main()\n\
         Dim result As Double\n\
         result = NativeSqrt(156.25)\n\
         End Sub",
    );
    assert!(
        any_double_near(&snapshot, 12.5),
        "expected sqrt(156.25)=12.5 in {snapshot:?}"
    );
}

#[test]
fn byref_numeric_declare_writes_back() {
    // oleaut32 `VarR8FromI4(Long in, ByRef Double out)` — exercises ByRef numeric
    // write-back through the native FFI lane.
    let snapshot = run(
        "Private Declare PtrSafe Function VarR8FromI4 Lib \"oleaut32\" (ByVal inVal As Long, ByRef outVal As Double) As Long\n\
         Sub Main()\n\
         Dim outVal As Double\n\
         Dim status As Long\n\
         status = VarR8FromI4(42, outVal)\n\
         End Sub",
    );
    assert!(
        any_double_near(&snapshot, 42.0),
        "expected VarR8FromI4 to write 42.0 into the ByRef Double in {snapshot:?}"
    );
}

#[test]
fn byval_ansi_string_declare_loads_library() {
    // kernel32 `LoadLibraryA(ByVal As String) As LongPtr` — ByVal ANSI string in,
    // a non-zero module handle out (then released).
    let snapshot = run(
        "Private Declare PtrSafe Function NativeLoad Lib \"kernel32\" Alias \"LoadLibraryA\" (ByVal name As String) As LongPtr\n\
         Private Declare PtrSafe Function NativeFree Lib \"kernel32\" Alias \"FreeLibrary\" (ByVal hModule As LongPtr) As Long\n\
         Sub Main()\n\
         Dim handle As LongPtr\n\
         handle = NativeLoad(\"kernel32\")\n\
         Dim freed As Long\n\
         freed = NativeFree(handle)\n\
         End Sub",
    );
    assert!(
        any_nonzero_i64(&snapshot),
        "expected a non-zero module handle from LoadLibraryA in {snapshot:?}"
    );
}

#[test]
fn native_declare_rejects_jit_without_falling_back() {
    let mut engine = Engine::new(HostConfig { enable_jit: true });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let err = engine
        .execute_source_with_variant_snapshot_clean(
            "Private Declare PtrSafe Function NativeSqrt Lib \"msvcrt\" Alias \"sqrt\" (ByVal x As Double) As Double\n\
             Sub Main()\n\
             Dim result As Double\n\
             result = NativeSqrt(4)\n\
             End Sub",
        )
        .expect_err("JIT execution is not implemented; it must not silently fall back");
    assert!(
        err.message().contains("JIT execution"),
        "unexpected diagnostic: {err}"
    );
}

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
fn riff_shaped_memmove_round_trips_varptr_scalars() {
    // Riff uses RtlMoveMemory for this shape. Use the C runtime's `memmove` here:
    // the pointer-helper and native-FFI requirements are the same, and the export
    // has a conventional C ABI so this test isolates OxVBA's pointer marshalling.
    let snapshot = run(
        "Private Declare PtrSafe Function MemMove Lib \"msvcrt\" Alias \"memmove\" (ByVal Destination As LongPtr, ByVal Source As LongPtr, ByVal Length As LongPtr) As LongPtr\n\
         Sub Main()\n\
         Dim src As Long\n\
         Dim copied As Long\n\
         Dim ret As LongPtr\n\
         src = &H11223344\n\
         ret = MemMove(VarPtr(copied), VarPtr(src), 4)\n\
         End Sub",
    );
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(0x11223344)),
        "expected RtlMoveMemory to copy scalar bytes through VarPtr pins: {snapshot:?}"
    );
}

#[test]
fn riff_exact_kernel32_memory_lane_handles_raw_pointer_offsets_and_zeroing() {
    // Riff allocates raw buffers, writes through pointer arithmetic (`ByVal (p+n)`),
    // reads back into scalar locals, then zeroes/frees the memory. This keeps the
    // native side effect bounded to one private allocation.
    let snapshot = run("Private Const MEM_COMMIT As Long = &H1000\n\
         Private Const MEM_RESERVE As Long = &H2000\n\
         Private Const MEM_RELEASE As Long = &H8000\n\
         Private Const PAGE_READWRITE As Long = &H4\n\
         Private Declare PtrSafe Function VirtualAlloc Lib \"kernel32\" (ByVal lpAddress As LongPtr, ByVal dwSize As LongPtr, ByVal flAllocationType As Long, ByVal flProtect As Long) As LongPtr\n\
         Private Declare PtrSafe Function VirtualFree Lib \"kernel32\" (ByVal lpAddress As LongPtr, ByVal dwSize As LongPtr, ByVal dwFreeType As Long) As Long\n\
         Private Declare PtrSafe Sub RtlMoveMemory Lib \"kernel32\" (ByVal Destination As LongPtr, ByVal Source As LongPtr, ByVal Length As LongPtr)\n\
         Private Declare PtrSafe Sub RtlZeroMemory Lib \"kernel32\" (ByVal Destination As LongPtr, ByVal Length As LongPtr)\n\
         Sub Main()\n\
         Dim mem As LongPtr\n\
         Dim src As Long\n\
         Dim copied As Long\n\
         Dim zeroed As Long\n\
         Dim zeroProof As Long\n\
         Dim freed As Long\n\
         mem = VirtualAlloc(0, 16, MEM_COMMIT Or MEM_RESERVE, PAGE_READWRITE)\n\
         If mem = 0 Then Err.Raise 700, \"RiffNative\", \"VirtualAlloc returned null\"\n\
         src = &H11223344\n\
         RtlMoveMemory ByVal (mem + 4), VarPtr(src), 4\n\
         RtlMoveMemory VarPtr(copied), ByVal (mem + 4), 4\n\
         RtlZeroMemory ByVal (mem + 4), 4\n\
         RtlMoveMemory VarPtr(zeroed), ByVal (mem + 4), 4\n\
         If zeroed = 0 Then zeroProof = &H556677\n\
         freed = VirtualFree(mem, 0, MEM_RELEASE)\n\
         If freed = 0 Then Err.Raise 701, \"RiffNative\", \"VirtualFree failed\"\n\
         End Sub");
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(0x11223344)),
        "expected RtlMoveMemory to copy through a raw pointer offset: {snapshot:?}"
    );
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(0x556677)),
        "expected RtlZeroMemory to clear the raw pointer offset and set zeroProof: {snapshot:?}"
    );
    assert!(
        snapshot.iter().any(|v| v.as_i64().is_some_and(|n| n != 0)),
        "expected a non-zero VirtualAlloc pointer in {snapshot:?}"
    );
}

#[test]
fn riff_exact_rtlmovememory_typed_byref_destinations_write_back() {
    // Riff declares helper aliases with a typed ByRef destination and a raw source
    // pointer. These must use ordinary Declare ByRef writeback, not VarPtr
    // expression-shape writeback.
    let snapshot = run(
        "Private Declare PtrSafe Sub RtlMoveMemoryToSingle Lib \"kernel32\" Alias \"RtlMoveMemory\" (ByRef Destination As Single, ByVal Source As LongPtr, ByVal Length As LongPtr)\n\
         Private Declare PtrSafe Sub RtlMoveMemoryToInteger Lib \"kernel32\" Alias \"RtlMoveMemory\" (ByRef Destination As Integer, ByVal Source As LongPtr, ByVal Length As LongPtr)\n\
         Sub Main()\n\
         Dim srcSingle As Single\n\
         Dim copiedSingle As Single\n\
         Dim srcInt As Integer\n\
         Dim copiedInt As Integer\n\
         srcSingle = 12.5!\n\
         srcInt = 1234\n\
         RtlMoveMemoryToSingle copiedSingle, VarPtr(srcSingle), 4\n\
         RtlMoveMemoryToInteger copiedInt, VarPtr(srcInt), 2\n\
         End Sub",
    );
    assert!(
        snapshot
            .iter()
            .any(|v| v.as_f32().is_some_and(|n| (n - 12.5).abs() < f32::EPSILON)),
        "expected typed Single destination writeback through RtlMoveMemory: {snapshot:?}"
    );
    assert!(
        snapshot.iter().any(|v| v.as_i16() == Some(1234)),
        "expected typed Integer destination writeback through RtlMoveMemory: {snapshot:?}"
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

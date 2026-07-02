//! L1 — native `Declare` lane routing on **vm3** (the sole runtime). With the binder
//! emitting `marshal_lane = "m1-native-ffi"` for real libraries, a `Declare Lib` call
//! reaches the real `LoadLibrary`/`GetProcAddress` FFI path under `interactive_dev`
//! (native mode) on Windows. Covers the non-pointer cases L1 unblocks: a scalar return,
//! a ByRef numeric write-back, and a `ByVal As String` (ANSI) argument, plus the riff-
//! shaped pointer/UDT/vtable marshalling shapes. (Pointer-helper arguments + write-back
//! are L2/L3; full coverage is the re-instated `native_declare_string_marshalling_end_to_end`
//! + SQLiteForExcel suites.)
//!
//! The `AddressOf` callback slot shape is covered with a synchronous `CallWindowProcW`
//! probe so vm3 proves the same native callback entry path real VBA code uses.
#![cfg(target_os = "windows")]

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};
use oxvba_runtime::{VarType, Variant};

/// Run a source module on the vm3 backend under the interactive-dev policy (which
/// enables native dynamic linking). The snapshot is the module globals + `Main`'s
/// locals.
fn run(source: &str) -> Vec<Variant> {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_source_with_variant_snapshot_clean(source)
        .expect("native declare probe should execute on the VM backend")
}

fn run_err(source: &str) -> String {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    match engine.execute_source_with_variant_snapshot_clean(source) {
        Ok(snapshot) => panic!("native declare probe should have failed, got {snapshot:?}"),
        Err(err) => format!("{err:?}"),
    }
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
         Private Const MEM_RELEASE As Long = &H8000&\n\
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
fn riff_shaped_callwindowproc_invokes_address_of_callback() {
    // Riff uses AddressOf plus native thunks to bridge into timer callbacks. This
    // bounded probe calls the VM callback thunk synchronously through
    // CallWindowProcW's four-argument callback ABI, without installing a timer or
    // touching executable memory.
    let snapshot = run(
        "Private Declare PtrSafe Function RiffCallPtr4 Lib \"user32\" Alias \"CallWindowProcW\" (ByVal lpPrevWndFunc As LongPtr, ByVal a0 As LongPtr, ByVal a1 As LongPtr, ByVal a2 As LongPtr, ByVal a3 As LongPtr) As LongPtr\n\
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
         End Sub",
    );
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

#[test]
fn riff_exact_vtableproc_reads_synthetic_vtable_slot() {
    // Riff's manual COM path first dereferences the object's vtable pointer, then
    // reads a procedure pointer by slot index. Use private synthetic memory so
    // the exact VTableProc byte-copy shape is covered without activating WASAPI
    // or calling an arbitrary native function pointer.
    let snapshot = run("Private Const MEM_COMMIT As Long = &H1000\n\
         Private Const MEM_RESERVE As Long = &H2000\n\
         Private Const MEM_RELEASE As Long = &H8000&\n\
         Private Const PAGE_READWRITE As Long = &H4\n\
         Private Declare PtrSafe Function VirtualAlloc Lib \"kernel32\" (ByVal lpAddress As LongPtr, ByVal dwSize As LongPtr, ByVal flAllocationType As Long, ByVal flProtect As Long) As LongPtr\n\
         Private Declare PtrSafe Function VirtualFree Lib \"kernel32\" (ByVal lpAddress As LongPtr, ByVal dwSize As LongPtr, ByVal dwFreeType As Long) As Long\n\
         Private Declare PtrSafe Sub RtlMoveMemory Lib \"kernel32\" (ByVal Destination As LongPtr, ByVal Source As LongPtr, ByVal Length As LongPtr)\n\
         Public SlotPtr As LongPtr\n\
         Private Function VTableProc(ByVal pUnk As LongPtr, ByVal vTableIndex As Long) As LongPtr\n\
         Dim pVtbl As LongPtr\n\
         RtlMoveMemory VarPtr(pVtbl), ByVal pUnk, LenB(pVtbl)\n\
         RtlMoveMemory VarPtr(VTableProc), ByVal (pVtbl + (vTableIndex * LenB(pVtbl))), LenB(pVtbl)\n\
         End Function\n\
         Sub Main()\n\
         Dim obj As LongPtr\n\
         Dim vt As LongPtr\n\
         Dim slotValue As LongPtr\n\
         Dim freedObj As Long\n\
         Dim freedVt As Long\n\
         obj = VirtualAlloc(0, 8, MEM_COMMIT Or MEM_RESERVE, PAGE_READWRITE)\n\
         vt = VirtualAlloc(0, 24, MEM_COMMIT Or MEM_RESERVE, PAGE_READWRITE)\n\
         If obj = 0 Or vt = 0 Then Err.Raise 710, \"RiffNative\", \"VirtualAlloc returned null\"\n\
         slotValue = &H12345678\n\
         RtlMoveMemory ByVal obj, VarPtr(vt), LenB(vt)\n\
         RtlMoveMemory ByVal (vt + (2 * LenB(vt))), VarPtr(slotValue), LenB(slotValue)\n\
         SlotPtr = VTableProc(obj, 2)\n\
         freedVt = VirtualFree(vt, 0, MEM_RELEASE)\n\
         freedObj = VirtualFree(obj, 0, MEM_RELEASE)\n\
         If freedVt = 0 Or freedObj = 0 Then Err.Raise 711, \"RiffNative\", \"VirtualFree failed\"\n\
         End Sub");
    assert!(
        snapshot.iter().any(|v| v.as_i64() == Some(0x12345678)),
        "expected exact Riff VTableProc shape to read slot 2 from the synthetic vtable: {snapshot:?}"
    );
}

#[test]
fn riff_exact_iidfromstring_writes_guid_udt_through_as_any() {
    // Riff uses `IIDFromString StrPtr("{...}"), guid` where `guid` is a UDT:
    // Long, Integer, Integer, Byte(0 To 7). The native `As Any` ByRef lane must
    // expose that record as the 16-byte GUID ABI layout and copy native writes
    // back into the record fields.
    let snapshot = run("Private Type GUID\n\
         Data1 As Long\n\
         Data2 As Integer\n\
         Data3 As Integer\n\
         Data4(0 To 7) As Byte\n\
         End Type\n\
         Private Declare PtrSafe Function IIDFromString Lib \"ole32\" (ByVal lpsz As LongPtr, ByRef lpiid As Any) As Long\n\
         Public Hr As Long\n\
         Public Data4First As Long\n\
         Public Data4Last As Long\n\
         Public Checksum As Long\n\
         Sub Main()\n\
         Dim iid As GUID\n\
         Hr = IIDFromString(StrPtr(\"{00000000-0000-0000-C000-000000000046}\"), iid)\n\
         Data4First = iid.Data4(0)\n\
         Data4Last = iid.Data4(7)\n\
         Checksum = iid.Data1 + iid.Data2 + iid.Data3 + iid.Data4(0) + iid.Data4(7)\n\
         End Sub");
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(0)),
        "expected IIDFromString to return S_OK in {snapshot:?}"
    );
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(0xC0)),
        "expected GUID Data4(0)=&HC0 after native UDT writeback: {snapshot:?}"
    );
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(0x46)),
        "expected GUID Data4(7)=&H46 after native UDT writeback: {snapshot:?}"
    );
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(0x106)),
        "expected checksum over GUID fields to include native writeback bytes: {snapshot:?}"
    );
}

#[test]
fn byref_as_any_copies_general_nested_udt_records() {
    let snapshot = run("Private Const MEM_COMMIT As Long = &H1000\n\
         Private Const MEM_RESERVE As Long = &H2000\n\
         Private Const MEM_RELEASE As Long = &H8000&\n\
         Private Const PAGE_READWRITE As Long = &H4\n\
         Private Type Inner\n\
         Flag As Boolean\n\
         Value As Long\n\
         End Type\n\
         Private Type Packet\n\
         Tag As Integer\n\
         Inner As Inner\n\
         Tail(0 To 3) As Byte\n\
         End Type\n\
         Private Declare PtrSafe Function VirtualAlloc Lib \"kernel32\" (ByVal lpAddress As LongPtr, ByVal dwSize As LongPtr, ByVal flAllocationType As Long, ByVal flProtect As Long) As LongPtr\n\
         Private Declare PtrSafe Function VirtualFree Lib \"kernel32\" (ByVal lpAddress As LongPtr, ByVal dwSize As LongPtr, ByVal dwFreeType As Long) As Long\n\
         Private Declare PtrSafe Sub RtlMoveMemoryRaw Lib \"kernel32\" Alias \"RtlMoveMemory\" (ByVal Destination As LongPtr, ByVal Source As LongPtr, ByVal Length As LongPtr)\n\
         Private Declare PtrSafe Sub RtlMoveMemoryAny Lib \"kernel32\" Alias \"RtlMoveMemory\" (ByRef Destination As Any, ByVal Source As LongPtr, ByVal Length As LongPtr)\n\
         Public CopiedTag As Long\n\
         Public CopiedFlag As Boolean\n\
         Public CopiedValue As Long\n\
         Public CopiedTail0 As Long\n\
         Public CopiedTail3 As Long\n\
         Sub Main()\n\
         Dim dst As Packet\n\
         Dim mem As LongPtr\n\
         Dim tag As Integer\n\
         Dim flag As Boolean\n\
         Dim value As Long\n\
         Dim tail0 As Byte\n\
         Dim tail3 As Byte\n\
         Dim freed As Long\n\
         mem = VirtualAlloc(0, 32, MEM_COMMIT Or MEM_RESERVE, PAGE_READWRITE)\n\
         If mem = 0 Then Err.Raise 720, \"NativeUdt\", \"VirtualAlloc returned null\"\n\
         tag = 1234\n\
         flag = True\n\
         value = &H11223344\n\
         tail0 = &HAB\n\
         tail3 = &HCD\n\
         RtlMoveMemoryRaw ByVal mem, VarPtr(tag), 2\n\
         RtlMoveMemoryRaw ByVal (mem + 4), VarPtr(flag), 2\n\
         RtlMoveMemoryRaw ByVal (mem + 8), VarPtr(value), 4\n\
         RtlMoveMemoryRaw ByVal (mem + 12), VarPtr(tail0), 1\n\
         RtlMoveMemoryRaw ByVal (mem + 15), VarPtr(tail3), 1\n\
         RtlMoveMemoryAny dst, ByVal mem, 16\n\
         CopiedTag = dst.Tag\n\
         CopiedFlag = dst.Inner.Flag\n\
         CopiedValue = dst.Inner.Value\n\
         CopiedTail0 = dst.Tail(0)\n\
         CopiedTail3 = dst.Tail(3)\n\
         freed = VirtualFree(mem, 0, MEM_RELEASE)\n\
         If freed = 0 Then Err.Raise 721, \"NativeUdt\", \"VirtualFree failed\"\n\
         End Sub");
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(1234)),
        "expected RtlMoveMemory to copy the outer Integer field through UDT As Any: {snapshot:?}"
    );
    assert!(
        snapshot.iter().any(|v| v.as_bool() == Some(true)),
        "expected RtlMoveMemory to copy the nested Boolean field through UDT As Any: {snapshot:?}"
    );
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(0x11223344)),
        "expected RtlMoveMemory to copy the nested Long field through UDT As Any: {snapshot:?}"
    );
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(0xAB)),
        "expected RtlMoveMemory to copy fixed-array byte 0 through UDT As Any: {snapshot:?}"
    );
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(0xCD)),
        "expected RtlMoveMemory to copy fixed-array byte 3 through UDT As Any: {snapshot:?}"
    );
}

#[test]
fn byref_as_any_declines_udt_records_with_owning_fields() {
    let error = run_err(
        "Private Type Packet\n\
         Text As String\n\
         End Type\n\
         Private Declare PtrSafe Sub RtlMoveMemoryAny Lib \"kernel32\" Alias \"RtlMoveMemory\" (ByRef Destination As Any, ByVal Source As LongPtr, ByVal Length As LongPtr)\n\
         Sub Main()\n\
         Dim dst As Packet\n\
         RtlMoveMemoryAny dst, 0, 0\n\
         End Sub",
    );
    assert!(
        error.contains("native ByRef As Any record marshaling is not supported")
            && error.contains("String fields"),
        "expected deterministic ByRef As Any decline for String-containing UDT, got {error}"
    );
}

#[test]
fn riff_shaped_as_any_scalar_byref_writes_back() {
    // Riff's DispCallFunc wrapper passes `vTypes(0)` and `pArgs(0)` to ByRef
    // `As Any` parameters. Those are scalar array elements, so `As Any` must
    // expose a native-width cell, not only GUID-shaped records.
    let snapshot = run(
        "Private Declare PtrSafe Function RtlMoveMemoryAny Lib \"kernel32\" Alias \"RtlMoveMemory\" (ByRef Destination As Any, ByVal Source As LongPtr, ByVal Length As LongPtr) As LongPtr\n\
         Public CopiedInteger As Long\n\
         Public CopiedLongPtr As LongLong\n\
         Public CopiedArrayInteger As Long\n\
         Public CopiedArrayLongPtr As LongLong\n\
         Sub Main()\n\
         Dim srcInt As Integer\n\
         Dim dstInt As Integer\n\
         Dim srcPtr As LongPtr\n\
         Dim dstPtr As LongPtr\n\
         Dim dstInts(0 To 0) As Integer\n\
         Dim dstPtrs(0 To 0) As LongPtr\n\
         srcInt = 1234\n\
         srcPtr = &H12345678\n\
         dstInts(0) = 0\n\
         dstPtrs(0) = 0\n\
         RtlMoveMemoryAny dstInt, VarPtr(srcInt), LenB(dstInt)\n\
         RtlMoveMemoryAny dstPtr, VarPtr(srcPtr), LenB(dstPtr)\n\
         RtlMoveMemoryAny dstInts(0), VarPtr(srcInt), LenB(dstInts(0))\n\
         RtlMoveMemoryAny dstPtrs(0), VarPtr(srcPtr), LenB(dstPtrs(0))\n\
         CopiedInteger = dstInt\n\
         CopiedLongPtr = dstPtr\n\
         CopiedArrayInteger = dstInts(0)\n\
         CopiedArrayLongPtr = dstPtrs(0)\n\
         End Sub",
    );
    let integer_hits = snapshot.iter().filter(|v| v.as_i32() == Some(1234)).count();
    assert!(
        integer_hits >= 2,
        "expected ByRef As Any Integer writeback for local and array element: {snapshot:?}"
    );
    let longptr_hits = snapshot
        .iter()
        .filter(|v| v.as_i64() == Some(0x12345678))
        .count();
    assert!(
        longptr_hits >= 2,
        "expected ByRef As Any LongPtr writeback for local and array element: {snapshot:?}"
    );
}

#[cfg(target_arch = "x86_64")]
#[test]
fn riff_shaped_dispcallfunc_vtable_call_writes_variant_result() {
    // Riff's vCall wrapper invokes `oleaut32!DispCallFunc` with a COM instance
    // pointer, a byte offset into the vtable, null argument tables for the zero-arg
    // case, and a ByRef Variant result. Use a private synthetic vtable whose slot
    // points at a harmless kernel32 no-arg export, avoiding real WASAPI activation.
    let snapshot = run("Private Const MEM_COMMIT As Long = &H1000\n\
         Private Const MEM_RESERVE As Long = &H2000\n\
         Private Const MEM_RELEASE As Long = &H8000&\n\
         Private Const PAGE_READWRITE As Long = &H4\n\
         Private Const CC_STDCALL As Long = 4\n\
         Private Const vbLong As Integer = 3\n\
         Private Declare PtrSafe Function VirtualAlloc Lib \"kernel32\" (ByVal lpAddress As LongPtr, ByVal dwSize As LongPtr, ByVal flAllocationType As Long, ByVal flProtect As Long) As LongPtr\n\
         Private Declare PtrSafe Function VirtualFree Lib \"kernel32\" (ByVal lpAddress As LongPtr, ByVal dwSize As LongPtr, ByVal dwFreeType As Long) As Long\n\
         Private Declare PtrSafe Sub RtlMoveMemory Lib \"kernel32\" (ByVal Destination As LongPtr, ByVal Source As LongPtr, ByVal Length As LongPtr)\n\
         Private Declare PtrSafe Function GetModuleHandleA Lib \"kernel32\" (ByVal lpModuleName As String) As LongPtr\n\
         Private Declare PtrSafe Function GetProcAddress Lib \"kernel32\" (ByVal hModule As LongPtr, ByVal lpProcName As String) As LongPtr\n\
         Private Declare PtrSafe Function DispCallFunc Lib \"oleaut32\" (ByVal pvInstance As LongPtr, ByVal oVft As LongPtr, ByVal cc As Long, ByVal vtReturn As Integer, ByVal cActuals As Long, ByRef prgvt As Any, ByRef prgpvarg As Any, ByRef pvargResult As Variant) As Long\n\
         Public HrInvoke As Long\n\
         Public SlotResult As Long\n\
         Public DispCallFuncProof As Long\n\
         Sub Main()\n\
         Dim obj As LongPtr\n\
         Dim vt As LongPtr\n\
         Dim proc As LongPtr\n\
         Dim hKernel As LongPtr\n\
         Dim vRet As Variant\n\
         Dim freedObj As Long\n\
         Dim freedVt As Long\n\
         obj = VirtualAlloc(0, 8, MEM_COMMIT Or MEM_RESERVE, PAGE_READWRITE)\n\
         vt = VirtualAlloc(0, 8, MEM_COMMIT Or MEM_RESERVE, PAGE_READWRITE)\n\
         If obj = 0 Or vt = 0 Then Err.Raise 720, \"RiffNative\", \"VirtualAlloc returned null\"\n\
         hKernel = GetModuleHandleA(\"kernel32.dll\")\n\
         proc = GetProcAddress(hKernel, \"GetTickCount\")\n\
         If proc = 0 Then Err.Raise 721, \"RiffNative\", \"GetProcAddress failed\"\n\
         RtlMoveMemory ByVal obj, VarPtr(vt), LenB(vt)\n\
         RtlMoveMemory ByVal vt, VarPtr(proc), LenB(proc)\n\
         HrInvoke = DispCallFunc(obj, 0, CC_STDCALL, vbLong, 0, ByVal 0&, ByVal 0&, vRet)\n\
         If HrInvoke = 0 Then SlotResult = CLng(vRet)\n\
         If SlotResult > 0 Then DispCallFuncProof = &H51512\n\
         freedVt = VirtualFree(vt, 0, MEM_RELEASE)\n\
         freedObj = VirtualFree(obj, 0, MEM_RELEASE)\n\
         If freedVt = 0 Or freedObj = 0 Then Err.Raise 722, \"RiffNative\", \"VirtualFree failed\"\n\
         End Sub");
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(0)),
        "expected DispCallFunc to return S_OK in {snapshot:?}"
    );
    assert!(
        snapshot.iter().any(|v| v.as_i32() == Some(0x51512)),
        "expected the vtable slot result Variant to set the positive-result proof marker: {snapshot:?}"
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

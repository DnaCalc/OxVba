# Pointer Helpers Status

Date: 2026-04-04; updated 2026-06-22
Owner: Codex
Status: in-progress

This note records the exact current support split for the `StrPtr` / `VarPtr` /
`ObjPtr` execution lane.

## Working Now

- `StrPtr`
  - compiler recognition, typing, VM lowering, JIT lowering, and native-interop
    lane selection are live
  - focused host evidence:
    - `cargo test -p oxvba-host --test pointer_helpers_end_to_end windows_pointer_helper_e2e::strptr_supports_wide_native_call_in_vm_and_jit -- --exact --nocapture`
  - current result: passes in both VM and JIT using CRT `wcslen`

- `ObjPtr`
  - stable identity is now preserved for the same runtime object handle
  - `Nothing` is surfaced as `0` for the current direct host-backed object lane
  - focused host evidence:
    - `cargo test -p oxvba-host --test pointer_helpers_end_to_end windows_pointer_helper_e2e::objptr_is_stable_for_same_object_in_vm_and_jit -- --exact --nocapture`
    - `cargo test -p oxvba-host --test pointer_helpers_end_to_end windows_pointer_helper_e2e::objptr_returns_zero_for_runtime_nothing_after_failed_createobject -- --exact --nocapture`

- bounded `VarPtr`
  - compiler recognition and typed runtime production are live for scalar
    values and canonical zero-based byte-buffer shapes such as
    `VarPtr(buf(0))`
  - native writeback is now live for declared-width scalar l-values
    (`Boolean`, `Byte`, `Integer`, `Long`, `LongLong`/`LongPtr`, `Single`,
    `Double`, `Currency`, and `Date`) and for the existing string/byte-buffer
    lanes
  - focused host evidence:
    - `cargo test -p oxvba-host --test pointer_helpers_end_to_end -- --nocapture`
    - `cargo test -p oxvba-host --test pointer_helpers_end_to_end windows_pointer_helper_e2e::varptr_supports_byte_buffer_native_read_in_vm_and_jit -- --exact --nocapture`
    - `cargo test -p oxvba-host --test native_declare_lane riff_shaped_memmove_round_trips_varptr_scalars -- --nocapture`
    - `cargo test -p oxvba-host --test native_declare_lane riff_exact_kernel32_memory_lane_handles_raw_pointer_offsets_and_zeroing -- --nocapture`
    - `cargo test -p oxvba-host --test native_declare_lane riff_exact_rtlmovememory_typed_byref_destinations_write_back -- --nocapture`
  - current result:
    - non-zero scalar pointer-like production in both VM and JIT
    - byte-buffer registry materialization in both VM and JIT
    - real native byte-buffer read through `ucrtbase!strlen` in both VM and JIT
    - real native scalar memory copy through `msvcrt!memmove` in the VM, with
      the copied bytes written back into the original `Long` variable
    - exact Riff-shaped `kernel32!RtlMoveMemory` / `RtlZeroMemory` over raw
      pointer offsets, plus typed `RtlMoveMemory` aliases with ByRef `Single`
      and `Integer` destinations

## Still Open

- runtime-sized dynamic arrays remain a later delivery area
- the SQLite core fixture now stops at a narrower compile-time boundary around
  expression-bounded `ReDim`, not at the pointer-helper lane
- fixed-string, `Variant` cell mutation, and UDT/record-address writeback lanes
  need separate parity work before this note claims them broadly

## SQLite Movement

- `Core64Normalized` no longer stops at `call to unknown procedure: strptr`
- current CLI/host boundary:
  - `PMR-E-BACKEND-COMPILE: type error: unsupported statement: ReDim with runtime expression bounds is not yet supported for array `buf`: buf(length - 1)`
- evidence:
  - `cargo run -p oxvba-cli -- run-project .external\sqliteforexcel\fixtures\Core64Normalized\SQLiteForExcelCore64Normalized.basproj`

## Reference Posture

- classicvb.net, "Unofficial Documentation for VarPtr, StrPtr, and ObjPtr":
  https://classicvb.net/tips/varptr/
- Stack Overflow, "What are the benefits and risks of using the StrPtr function
  in VBA?":
  https://stackoverflow.com/questions/42015700/what-are-the-benefits-and-risks-of-using-the-strptr-function-in-vba

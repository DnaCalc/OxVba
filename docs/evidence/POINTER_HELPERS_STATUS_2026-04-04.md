# Pointer Helpers Status

Date: 2026-04-04
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
  - compiler recognition and typed runtime production are live for scalar values
  - focused host evidence:
    - `cargo test -p oxvba-host --test pointer_helpers_end_to_end -- --nocapture`
  - current automated proof is bounded to non-zero pointer-like production for a
    scalar variable in both VM and JIT

## Still Open

- `VarPtr(buf(0))` native dereference for array-buffer interop is not yet
  trustworthy
- the exact direct probe
  - `cargo test -p oxvba-host --test pointer_helpers_end_to_end windows_pointer_helper_e2e::varptr_supports_byte_buffer_native_read_in_vm_and_jit -- --exact --nocapture`
  currently terminates with:
  - `exit code: 0xc0000005 (STATUS_ACCESS_VIOLATION)`
- this means the later SQLite-style array-buffer lane remains open even though
  the helper family is now recognized and the core SQLite fixture has moved past
  the old `StrPtr` frontier

## SQLite Movement

- `Core64Normalized` no longer stops at `call to unknown procedure: strptr`
- current CLI/host boundary:
  - `PMR-E-BACKEND-COMPILE: type error: unsupported statement: ReDim buf(length - 1)`
- evidence:
  - `cargo run -p oxvba-cli -- run-project .external\sqliteforexcel\fixtures\Core64Normalized\SQLiteForExcelCore64Normalized.basproj`

## Reference Posture

- classicvb.net, "Unofficial Documentation for VarPtr, StrPtr, and ObjPtr":
  https://classicvb.net/tips/varptr/
- Stack Overflow, "What are the benefits and risks of using the StrPtr function
  in VBA?":
  https://stackoverflow.com/questions/42015700/what-are-the-benefits-and-risks-of-using-the-strptr-function-in-vba

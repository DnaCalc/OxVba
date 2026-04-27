# SQLite Runtime-Sized ReDim Base Array Evidence

Date: 2026-04-27

Bead: `bd-sql1.16.2`

## Result

The bounded one-dimensional non-`Preserve` runtime-sized `ReDim` substrate
required by the SQLite UTF-8 helper lane is present and validated in the current
codebase.

The implementation path is:

- `crates/oxvba-compiler/src/resolve.rs` parses dynamic-array runtime
  expression bounds into `BoundStmt::ReDimRuntime`.
- `crates/oxvba-compiler/src/emit.rs` lowers runtime `ReDim` into
  `Instruction::IntrinsicArrayResize` against the base array slot.
- `crates/oxvba-vm/src/interpreter.rs` materializes typed `SAFEARRAY` payloads
  for runtime array resize.
- `crates/oxvba-jit/src/runtime_helpers.rs` exposes the matching JIT helper
  path while preserving retained `Variant` array carriers.

## Validation

Commands:

```powershell
cargo test -p oxvba-compiler compile_runtime_redim_expression_bounds_on_dynamic_array_emits_resize_instruction -- --nocapture
cargo test -p oxvba-compiler resolve_runtime_redim_expression_bounds_on_dynamic_array -- --nocapture
cargo test -p oxvba-vm intrinsic_array_resize_1d_materializes_zeroed_byte_payload --lib -- --nocapture
cargo test -p oxvba-jit runtime_array_resize_paths_preserve_variant_slot_carriers --lib -- --nocapture
cargo test -p oxvba-host --test sqliteforexcel_declare_integration sqliteforexcel_sqlite3_module_source_direct_compile_moves_past_pointer_and_redim_boundaries -- --nocapture
```

Results:

| Check | Result |
| --- | --- |
| Compiler emit runtime `ReDim` instruction | pass |
| Compiler resolve runtime expression bounds | pass |
| VM runtime array resize materialization | pass |
| JIT runtime array resize helper carrier preservation | pass |
| SQLite direct compile moves past pointer and `ReDim` boundaries | pass |

## Residual Boundary

This closes only `bd-sql1.16.2`: one-dimensional runtime-sized non-`Preserve`
`ReDim` allocation into base array slots.

The remaining SQLite runtime buffer lane continues under:

- `bd-sql1.16.3`: bridge `VarPtr(buf(0))` and array return over runtime-sized
  byte buffers.
- `bd-sql1.16.4`: rerun and publish the SQLite fixture matrix after the bridge
  work.

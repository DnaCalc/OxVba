# SQLite Runtime Byte Buffer Bridge Evidence

Date: 2026-04-27

Bead: `bd-sql1.16.3`

## Result

The bounded runtime-sized byte-buffer bridge required after runtime `ReDim`
allocation is present and validated.

Covered behavior:

- `VarPtr(dst(0))` over a runtime-sized byte array can be passed to a native
  writeback call.
- Indexed reads from the runtime-sized byte buffer work after native writeback.
- `LBound`/`UBound` iteration over the runtime-sized buffer works.
- Dynamic byte-array function return assignment preserves byte values in VM and
  JIT.

## Validation

Commands:

```powershell
cargo test -p oxvba-host --test pointer_helpers_end_to_end runtime_sized_byte_array_native_writeback_and_index_reads_work_in_vm_and_jit -- --nocapture
cargo test -p oxvba-host --test pointer_helpers_end_to_end dynamic_byte_array_function_return_assignment_preserves_byte_values_in_vm_and_jit -- --nocapture
cargo test -p oxvba-compiler compile_dynamic_byte_array_function_return_emits_all_index_reads -- --nocapture
cargo test -p oxvba-compiler resolve_and_typecheck_dynamic_byte_array_function_return_call -- --nocapture
```

Results:

| Check | Result |
| --- | --- |
| Runtime-sized byte buffer native writeback and indexed reads in VM/JIT | pass |
| Dynamic byte-array function return assignment in VM/JIT | pass |
| Compiler emits indexed reads for dynamic byte-array function return | pass |
| Compiler resolves and typechecks dynamic byte-array function return call | pass |

## Residual Boundary

This closes only `bd-sql1.16.3`. The remaining SQLite lane work is
`bd-sql1.16.4`: rerun raw and normalized SQLite fixture rows and publish the
moved boundary or first successful native execution evidence.

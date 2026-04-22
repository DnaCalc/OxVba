## Native Declare Writeback Reconciliation

Date: `2026-04-22`
Bead: `bd-t8rr.7.3` / `vmm-g2`

## Outcome

The migrated value model remains compatible with the currently supported native
declare/writeback subset. No additional runtime adapter rewrite was required in
this bead; the real delivery was tightening executable coverage until the
supported subset was explicit and verified under the migrated substrate.

## Supported Writeback Subset Verified In This Bead

Host-backed Windows declare probes now explicitly verify:

1. `ByRef LongPtr`
2. `ByRef Boolean`
3. `ByRef Integer`
4. `ByRef Single`
5. `ByRef Double`
6. `ByRef LongLong`
7. `ByRef Currency`
8. `ByRef Date`
9. pointer-driven string payload writeback through `StrPtr(...)`
10. pointer-driven string-slot writeback through `VarPtr(s As String)`
11. pointer-driven byte-buffer writeback through `VarPtr(buf(0))`

The additional executable probes landed in:

- [native_declare_string_marshalling_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/native_declare_string_marshalling_end_to_end.rs)

## Verification

Executed and passing:

1. `cargo test -p oxvba-host --test native_declare_string_marshalling_end_to_end -- --test-threads=1 --nocapture`

The added probes use real Windows APIs:

1. `VarBoolFromI4`
2. `VarR8FromI4`
3. `VarR4FromI4`
4. `VarI2FromI4`
5. `GetDiskFreeSpaceExW`
6. existing `VarCyFromI4`
7. existing `VarDateFromStr`
8. existing string/pointer/writeback APIs

## Interpretation

1. the standard dynlink adapter's supported `ByRef` scalar subset remains
   correct after the value-model migration work already landed in the runtime
   and pointer-helper layers
2. the current supported writeback subset is wider than the old bead baseline
   explicitly proved
3. the remaining ABI/layout lane should now treat broader UDT/layout-sensitive
   cases as the open work, not ordinary scalar/native writeback for the covered
   types above

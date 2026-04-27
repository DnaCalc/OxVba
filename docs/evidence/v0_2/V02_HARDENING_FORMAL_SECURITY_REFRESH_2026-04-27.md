# V0.2 VM/JIT Hardening Formal and Security Refresh

- Bead: `bd-bqm8.6.5`
- Parent lane: `bd-bqm8.6`
- Date: 2026-04-27
- Status: evidence bead complete; lane remains in-progress until final checklist

## Commands

```powershell
cargo test -p oxvba-runtime variant_ --lib
cargo test -p oxvba-jit slot_abi --lib
cargo test -p oxvba-vm hardening_rejects --lib
cargo test -p oxvba-vm rejects_invalid_jump_target --lib
cargo check -p oxvba-runtime -p oxvba-vm -p oxvba-jit
rg -n "unsafe|from_raw|from_wire_bytes|try_to_runtime_value|slot out of range|malformed|invalid .*payload" crates/oxvba-runtime/src/variant.rs crates/oxvba-runtime/src/pointer_helpers.rs crates/oxvba-jit/src/slot_abi.rs crates/oxvba-vm/src/interpreter.rs crates/oxvba-vm/src/semantics.rs -g "*.rs"
rg -n "HARD-VMJIT|bd-bqm8\.6|hardening|malformed" docs/evidence/v0_2 docs/worksets/WORKSET_2026-04-06_V0_2_SCOPE_ROUNDOUT_EXECUTION.md
```

## Results

| Command | Result | Notes |
| --- | --- | --- |
| `cargo test -p oxvba-runtime variant_ --lib` | pass | 31 passed, 0 failed. Covers retained `Variant`, pointer helper, SAFEARRAY, wire parsing, and malformed pointer-payload tests selected by the matrix. |
| `cargo test -p oxvba-jit slot_abi --lib` | pass | 8 passed, 0 failed. Covers retained JIT slot layout plus malformed pointer slot checked projection. |
| `cargo test -p oxvba-vm hardening_rejects --lib` | pass | 3 passed, 0 failed. Covers malformed VM bytecode slots and malformed runtime payload in arithmetic semantics. |
| `cargo test -p oxvba-vm rejects_invalid_jump_target --lib` | pass | 1 passed, 0 failed. Keeps invalid control-flow target handling pinned. |
| `cargo check -p oxvba-runtime -p oxvba-vm -p oxvba-jit` | pass | Runtime, VM, and JIT hardening packages compile cleanly. |
| safety-surface scan | pass, residuals classified | Raw/unsafe hits remain concentrated in retained boundary materialization code: BSTR/SAFEARRAY/object pointer ownership, Windows VARIANT pointer helpers, and tests that inspect native layout. |
| evidence trace scan | pass | `bd-bqm8.6.1` through `bd-bqm8.6.4` evidence artifacts and `HARD-VMJIT-*` matrix rows are traceable. |

## Residual Classification

| Row | Residual | Classification | Follow-up |
| --- | --- | --- | --- |
| `HARD-VMJIT-001` | Retained `Variant` wire parsing can still only validate representable malformed shapes safely. Arbitrary invalid heap pointers cannot be dereferenced safely for proof-by-test. | accepted V0.2 boundary residual | Keep invalid heap pointer fuzzing out of safe unit tests; cover null/zero and malformed metadata shapes instead. |
| `HARD-VMJIT-002` | JIT slot malformed pointer payloads now produce checked diagnostics, but raw layout remains ABI-sensitive. | accepted V0.2 boundary residual | Keep `slot_abi` layout tests and checked projection tests in the final checklist. |
| `HARD-VMJIT-003` | Windows pointer helper ownership code necessarily uses raw pointer and FFI APIs for BSTR, VARIANT, SAFEARRAY, and object references. | accepted V0.2 boundary residual | Treat pointer helper raw sections as boundary code requiring review, not general runtime representation. |
| `HARD-VMJIT-004` | VM semantic payload validation is covered for selected malformed inputs; exhaustive hostile payload fuzzing is not part of V0.2. | non-blocking residual | Defer broad fuzzing to a later hardening lane if required. |
| `HARD-VMJIT-005` | Invalid jump and slot-shape regressions are pinned; full bytecode validator is not introduced in this bead. | non-blocking residual | Consider a central bytecode validator if future serialized bytecode ingestion becomes a production API. |
| `HARD-VMJIT-008` | Kani/formal lanes were not expanded in this bead; current policy treats formal failures/skips as non-blocking when documented. | non-blocking formal residual | Final checklist must cite this refresh and keep formal expansion as follow-up rather than lane blocker. |

## Next Step

`bd-bqm8.6` remains in-progress. The next bead is `bd-bqm8.6.6`, the final
VM/JIT hardening checklist.


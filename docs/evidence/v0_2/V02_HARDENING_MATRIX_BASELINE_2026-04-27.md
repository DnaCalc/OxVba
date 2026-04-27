# V0.2 VM/JIT Hardening Matrix Baseline

- Bead: `bd-bqm8.6.2`
- Parent lane: `bd-bqm8.6`
- Date: 2026-04-27
- Status: matrix baseline complete; lane remains in-progress

## Scope

This artifact establishes the V0.2 VM/JIT hardening matrix and scan baseline.
It does not claim the hardening lane is complete. Follow-on delivery remains
owned by `bd-bqm8.6.3`, `bd-bqm8.6.4`, `bd-bqm8.6.5`, and `bd-bqm8.6.6`.

## Baseline Commands

```powershell
rg -n "panic!|unwrap\(|expect\(|unsafe|from_raw|from_bytes|data_bytes|Variant::zeroed|to_runtime_value\(|variant_cell_pointer|payload_u64|bytecode|malformed|invalid" crates/oxvba-runtime/src crates/oxvba-vm/src crates/oxvba-jit/src crates/oxvba-host/src -g "*.rs"
rg -n "no_panic|malformed|invalid|hardening|formal_v|slot_abi|variant_.*rejects|boundary" crates/oxvba-runtime/src crates/oxvba-vm/src crates/oxvba-jit/src crates/oxvba-host/src docs/evidence/formal docs/evidence/profiles -g "*.rs" -g "*.md"
cargo test -p oxvba-runtime variant_compat_slot_boundary --lib
cargo test -p oxvba-jit slot_abi --lib
```

## Baseline Results

| Command | Result | Notes |
| --- | --- | --- |
| runtime/JIT/VM/host panic and raw-boundary scan | pass, findings classified | Most high-volume `expect` hits are test assertions. Production findings cluster around bytecode bounds, retained `Variant` projection, VM coercion payload validation, pointer helpers, and JIT slot ABI boundaries. |
| malformed/boundary/formal evidence scan | pass, findings classified | Existing invalid payload diagnostics are present in `variant.rs`, `coerce.rs`, `semantics.rs`, and `interpreter.rs`; follow-up rows below require focused regression hardening. |
| `cargo test -p oxvba-runtime variant_compat_slot_boundary --lib` | pass | 2 passed, 0 failed. Covers supported legacy subset roundtrip and non-legacy carrier rejection at the compat-slot boundary. |
| `cargo test -p oxvba-jit slot_abi --lib` | pass | 7 passed, 0 failed. Covers slot layout, scalar/string projection, binding handle projection, and slot storage pointer exposure. |

## Hardening Matrix

| Row | Surface | Current Baseline | Required Follow-up | Owner |
| --- | --- | --- | --- | --- |
| `HARD-VMJIT-001` | Retained `Variant` compat-slot projection | `Variant::to_compat_slot` rejects non-legacy carriers; `Variant::to_runtime_value` emits invalid payload errors for malformed payload bytes. | Add focused malformed retained-Variant boundary tests and tighten diagnostics where the selected malformed shape can be represented without unsafe construction. | `bd-bqm8.6.3` |
| `HARD-VMJIT-002` | JIT `RtSlot` boundary projection | `slot_abi` tests cover layout, scalar/string payloads, binding handles, and storage pointer behavior. | Add deterministic rejection for malformed/unknown JIT slot tags or malformed heap payloads selected from the matrix. | `bd-bqm8.6.3` |
| `HARD-VMJIT-003` | Pointer-helper cells for BSTR, VARIANT, SAFEARRAY, and object handles | Pointer helper tests already cover typed cell roundtrips and boundary pointer storage. | Add hostile/null/mismatched pointer-cell regressions where public helper APIs can express them safely. | `bd-bqm8.6.3` |
| `HARD-VMJIT-004` | VM runtime payload coercion | VM semantics and runtime coercion paths already return invalid payload diagnostics for String, Decimal, numeric, Boolean, Error, Object, and SAFEARRAY payloads. | Add bytecode/runtime-input regressions that assert deterministic errors instead of panics for selected malformed operands. | `bd-bqm8.6.4` |
| `HARD-VMJIT-005` | Bytecode control-flow validity | Interpreter rejects invalid jump targets; JIT compilation rejects invalid bytecode segment bounds and invalid jump patch targets. | Add a malformed bytecode regression set for invalid control-flow and stack/input shapes not already pinned by tests. | `bd-bqm8.6.4` |
| `HARD-VMJIT-006` | Host/project/source malformed inputs | Host runner and project graph paths have explicit invalid-config and invalid-project diagnostics; parser/engine formal fixtures already include no-panic coverage for selected language constructs. | Add selected malformed source/project regressions that exercise host-to-VM diagnostics without relying on test-only `expect` paths. | `bd-bqm8.6.4` |
| `HARD-VMJIT-007` | Raw/unsafe representation assumptions | Raw representation hits are concentrated in runtime pointer helpers, BSTR/SAFEARRAY owners, JIT context, and slot ABI code. | Refresh safety review evidence and classify any raw/unsafe assumption that cannot be converted to a checked API in V0.2. | `bd-bqm8.6.5` |
| `HARD-VMJIT-008` | Formal/security evidence | Existing formal evidence register contains historical hardening rows and non-blocking formal skips. | Run focused formal/security evidence refresh for the selected hardening rows; record unresolved formal failures as explicit non-blocking residuals. | `bd-bqm8.6.5` |

## Delivery Rules

- `bd-bqm8.6.2` closes only the matrix and scan baseline.
- `bd-bqm8.6` remains in-progress until delivery, regression, evidence, and final checklist beads complete.
- The next ready delivery bead is `bd-bqm8.6.3`, focused on malformed retained-Variant and JIT-slot boundary handling.


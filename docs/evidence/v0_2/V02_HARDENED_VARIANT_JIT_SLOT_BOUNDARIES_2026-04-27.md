# V0.2 Hardened Variant and JIT Slot Boundaries

- Bead: `bd-bqm8.6.3`
- Parent lane: `bd-bqm8.6`
- Date: 2026-04-27
- Status: delivery bead complete; lane remains in-progress

## Delivered Hardening

- `VariantCore::from_wire_bytes` now rejects non-decimal `VARIANT` wire bytes
  with non-zero reserved words before materializing a retained `Variant`.
- Decimal wire parsing remains allowed to use the reserved-word fields because
  the current retained decimal carrier stores `scale_sign` and high payload
  words there.
- `RtSlot` now exposes `try_to_runtime_value()` for checked JIT slot projection
  with deterministic malformed-slot diagnostics.
- The existing panicking JIT test compatibility bridge now delegates to the
  checked projection path and is explicitly documented as test compatibility.

## Regression Coverage

```powershell
cargo test -p oxvba-runtime variant_wire --lib
cargo test -p oxvba-runtime variant_runtime_projection_rejects_malformed_pointer_payloads --lib
cargo test -p oxvba-jit malformed_pointer_slot_projects_to_deterministic_error --lib
cargo test -p oxvba-jit slot_abi --lib
```

## Results

| Command | Result | Notes |
| --- | --- | --- |
| `cargo test -p oxvba-runtime variant_wire --lib` | pass | 4 passed, 0 failed. Includes non-decimal reserved-word rejection and decimal reserved-payload acceptance. |
| `cargo test -p oxvba-runtime variant_runtime_projection_rejects_malformed_pointer_payloads --lib` | pass | 1 passed, 0 failed. Covers zero object and SAFEARRAY pointer payload rejection. |
| `cargo test -p oxvba-jit malformed_pointer_slot_projects_to_deterministic_error --lib` | pass | 1 passed, 0 failed. Covers checked JIT slot diagnostics for malformed object and SAFEARRAY pointer payloads. |
| `cargo test -p oxvba-jit slot_abi --lib` | pass | 8 passed, 0 failed. Existing slot ABI suite remains green with the new checked projection path. |

## Remaining Lane Work

`bd-bqm8.6` remains in-progress. The next delivery bead is `bd-bqm8.6.4`,
covering malformed bytecode, project/source, and runtime-input regressions from
the hardening matrix.


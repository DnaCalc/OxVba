# V0.2 VM/JIT Hardening Final Checklist

- Bead: `bd-bqm8.6.6`
- Parent lane: `bd-bqm8.6`
- Date: 2026-04-27
- Status: complete

## Checklist

| Gate | Result | Evidence |
| --- | --- | --- |
| Child bead rollout exists | pass | [V02_HARDENING_ROLLOUT_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_HARDENING_ROLLOUT_2026-04-27.md) |
| Hardening matrix exists and assigns follow-up rows | pass | [V02_HARDENING_MATRIX_BASELINE_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_HARDENING_MATRIX_BASELINE_2026-04-27.md) |
| Retained `Variant` and JIT slot malformed boundary handling is hardened | pass | [V02_HARDENED_VARIANT_JIT_SLOT_BOUNDARIES_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_HARDENED_VARIANT_JIT_SLOT_BOUNDARIES_2026-04-27.md) |
| Malformed VM bytecode/runtime-input regressions exist | pass | [V02_MALFORMED_VM_INPUT_REGRESSIONS_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_MALFORMED_VM_INPUT_REGRESSIONS_2026-04-27.md) |
| Formal/security residuals are explicit | pass | [V02_HARDENING_FORMAL_SECURITY_REFRESH_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_HARDENING_FORMAL_SECURITY_REFRESH_2026-04-27.md) |
| Parent lane has no support-only closure | pass | `bd-bqm8.6.3` and `bd-bqm8.6.4` delivered code/test changes; support/evidence beads do not stand alone. |

## Final Validation Commands

```powershell
cargo test -p oxvba-runtime variant_ --lib
cargo test -p oxvba-jit slot_abi --lib
cargo test -p oxvba-vm hardening_rejects --lib
cargo test -p oxvba-vm rejects_invalid_jump_target --lib
cargo check -p oxvba-runtime -p oxvba-vm -p oxvba-jit
cargo fmt --check
git diff --check
rg -n "bd-bqm8\.6\.[1-5].*complete|bd-bqm8\.6 remains|HARD-VMJIT-00[1-8]|residual" docs/evidence/v0_2 docs/worksets/WORKSET_2026-04-06_V0_2_SCOPE_ROUNDOUT_EXECUTION.md
```

## Final Validation Results

| Command | Result |
| --- | --- |
| `cargo test -p oxvba-runtime variant_ --lib` | pass, 31 passed, 0 failed |
| `cargo test -p oxvba-jit slot_abi --lib` | pass, 8 passed, 0 failed |
| `cargo test -p oxvba-vm hardening_rejects --lib` | pass, 3 passed, 0 failed |
| `cargo test -p oxvba-vm rejects_invalid_jump_target --lib` | pass, 1 passed, 0 failed |
| `cargo check -p oxvba-runtime -p oxvba-vm -p oxvba-jit` | pass |
| `cargo fmt --check` | pass |
| `git diff --check` | pass |
| trace scan | pass |

## Closure Basis

`bd-bqm8.6` is complete for the bounded V0.2 VM/JIT hardening and security
pass. The lane delivered an explicit matrix, checked retained-Variant/JIT-slot
diagnostics, malformed VM input regressions, and residual classification. Raw
pointer/unsafe boundary code remains an accepted V0.2 residual because it is the
necessary representation boundary for BSTR, SAFEARRAY, object references, and
Windows VARIANT materialization; it is not a blocker for this lane because it is
documented, scoped, and covered by focused tests where safely representable.


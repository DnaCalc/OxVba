# V0.2 Malformed VM Input Regressions

- Bead: `bd-bqm8.6.4`
- Parent lane: `bd-bqm8.6`
- Date: 2026-04-27
- Status: delivery bead complete; lane remains in-progress

## Delivered Regressions

- Added a malformed bytecode regression for an out-of-range destination/write
  slot beyond the VM's default register preallocation.
- Added a malformed bytecode regression for an out-of-range operand/read slot.
- Added a runtime-input regression for a malformed retained `Variant` object
  payload reaching a numeric VM semantic operation.
- Kept the existing invalid jump-target regression as the control-flow baseline.

## Regression Commands

```powershell
cargo test -p oxvba-vm hardening_rejects --lib
cargo test -p oxvba-vm rejects_invalid_jump_target --lib
```

## Results

| Command | Result | Notes |
| --- | --- | --- |
| `cargo test -p oxvba-vm hardening_rejects --lib` | pass | 3 passed, 0 failed. Covers out-of-range bytecode write slot, out-of-range bytecode operand slot, and malformed runtime payload in arithmetic semantics. |
| `cargo test -p oxvba-vm rejects_invalid_jump_target --lib` | pass | 1 passed, 0 failed. Keeps invalid control-flow target handling pinned. |

## Remaining Lane Work

`bd-bqm8.6` remains in-progress. The next bead is `bd-bqm8.6.5`, covering
formal/security evidence refresh and residual classification for the hardening
matrix rows.


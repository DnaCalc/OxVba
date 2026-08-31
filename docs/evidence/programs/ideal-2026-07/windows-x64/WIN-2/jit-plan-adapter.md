# WIN-2 JIT Verified-Plan Adapter

Date: 2026-08-23
Bead: `bd-59co.3.3.4`
Status: in-progress delivery evidence. This does not close `WAC-VERIFIED-INTEROP-PLAN`.

## Outcome

JIT foreign `ComCallLate` and `CallNative(Declare)` execute the same verified
plan identity VM3 uses. Project-instance member dispatch is unchanged.
Pointer-helper Declare writebacks still decline to `bd-59co.3.3.5`. Whole-image
Declare admission is removed. Two fail-closed VM3/JIT fixtures live in
`crates/oxvba-differential/tests/jit_windows_vm3_parity.rs`.

## Commands

- `cargo test -p oxvba-differential --test jit_windows_vm3_parity -- --nocapture`
- `cargo test -p oxvba-differential --test jit_portable_vm3_parity -- --nocapture`

## Residual

Excel rows stay planned under WIN-14. Remaining WIN-2 session/cache/early/event
serving work stays with `bd-59co.3.3.5`. WIN-3 scalar late COM continues as
`bd-59co.3.4.2`. WIN-9 scalar Declare continues as `bd-59co.3.10.2`.

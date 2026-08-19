# CORE-7 Portable Instruction Declines

Date: 2026-08-18
Bead: `bd-59co.2.9.4`
Status: in-progress delivery evidence. This does not close `CORE-JIT-LOWERING`.

## Outcome

JIT admission no longer whole-image-declines a program merely because its
`external_calls` or `com_interfaces` tables are nonempty. Decline happens only
when the image actually lowers `CallNative(Declare)`, `ComCallEarly`,
`ComCallLate`, or `Ptr`.

Unused `Declare` metadata therefore JIT-executes the portable path. A used
Declare/COM/pointer instruction still declines before codegen with the existing
`native/COM calls start in M4-9` message.

## Commands

- `cargo test -p oxvba-differential --test jit_portable_vm3_parity -- --nocapture`
- `cargo test -p oxvba-differential --test jit_linux_safe_scope -- --nocapture`
- `cargo test -p oxvba-jit -- --nocapture`

## Residual

Remaining portable call/aggregate/library gaps stay with `.5` through `.7`.
Windows Declare/COM/pointer execution stays outside this tranche.
`bd-59co.2.9.9` still owns the later architecture.

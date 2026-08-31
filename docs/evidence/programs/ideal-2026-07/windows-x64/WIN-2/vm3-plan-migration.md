# WIN-2 VM3 Plan Migration

Date: 2026-08-23
Bead: `bd-59co.3.3.3`
Status: in-progress delivery evidence. This does not close `WAC-VERIFIED-INTEROP-PLAN`.

## Outcome

VM3 late-bound foreign COM dispatch and `CallNative(Declare)` elaborate and
verify a shared `VerifiedInteropPlan` before the existing host COM/dynlink
paths. Marshalling stays in `oxvba-com` and HAL dynlink. There is no second
marshaller.

## Commands

- `cargo test -p oxvba-runtime interop_plan -- --nocapture`
- `cargo test -p oxvba-vm3 interop_plan -- --nocapture`

## Residual

JIT adapter is `bd-59co.3.3.4`. Remaining WIN-2 work is `bd-59co.3.3.5`.

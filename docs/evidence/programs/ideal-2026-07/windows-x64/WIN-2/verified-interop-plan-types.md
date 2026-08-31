# WIN-2 Verified Interop Plan Types

Date: 2026-08-23
Bead: `bd-59co.3.3.2`
Status: in-progress delivery evidence. This does not close `WAC-VERIFIED-INTEROP-PLAN`.

## Outcome

`oxvba-runtime` now owns backend-neutral first-slice plan types and a fail-closed
verifier for late IDispatch and x64 `Declare`. VM3 and JIT consume the same
canonical identity string. x86, empty member/library, missing LastDllError
capture, IDispatch fallback on Declare, and tampered identity fail closed.

## Commands

- `cargo test -p oxvba-runtime interop_plan -- --nocapture`

## Residual

VM3 migration is `bd-59co.3.3.3`. JIT adapter is `bd-59co.3.3.4`. Remaining
WIN-2 session/cache/early/event/serving plan kinds stay with `bd-59co.3.3.5`.
Excel certification stays with WIN-14.

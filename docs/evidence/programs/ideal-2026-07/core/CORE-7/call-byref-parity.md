# CORE-7 Portable Call, ByRef, Optional and ParamArray

Date: 2026-08-18
Bead: `bd-59co.2.9.5`
Status: in-progress delivery evidence. This does not close `CORE-JIT-LOWERING`.

## Outcome

The portable-basics harness now locks VM3/JIT matches for:

- static ByVal function
- ByRef writeback
- omitted Optional Long
- omitted Optional Variant / `IsMissing`
- ParamArray sum

No additional JIT lowering change was required for this slice. The earlier
M4-era omitted-Optional/ParamArray declines are not hit by the current
elaborated portable shapes.

## Command

`cargo test -p oxvba-differential --test jit_portable_vm3_parity portable_basics_call -- --nocapture`

Result: passed.

## Residual

Windows/COM/native calls remain declined. Remaining CORE-7 architecture remains
`bd-59co.2.9.9`.

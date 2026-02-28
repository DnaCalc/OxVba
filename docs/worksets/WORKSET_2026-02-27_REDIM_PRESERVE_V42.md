# WORKSET_2026-02-27_REDIM_PRESERVE_V42.md

## Purpose
Execute profile `v42` (`mvp-lang-redim-preserve-v42`) for dynamic array reshaping subset.

## Scope
- Parse and bind `ReDim a(n)` and `ReDim Preserve a(n)` (1D literal bound subset).
- Update bound model for post-`ReDim` index validation.
- Preserve existing values when `Preserve` is used.
- Reinitialize array slots for non-preserve reshapes.

## Implementation Notes
- This subset uses static slot allocation with shape updates tracked in resolver state.
- Literal index/bound subset only; dynamic bounds are deferred.
- Shrink + out-of-bounds access is rejected deterministically.

## Validation Commands
```powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-lang-redim-preserve-v42
```

## Completion Signal
`v42` closes when ReDim conformance fixtures and `FO-V42-*` obligations are green (or formally logged under non-blocking policy).

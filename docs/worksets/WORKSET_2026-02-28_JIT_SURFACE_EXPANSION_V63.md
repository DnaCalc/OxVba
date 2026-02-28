# WORKSET_2026-02-28_JIT_SURFACE_EXPANSION_V63

## Profile
- ID: `mvp-jit-surface-expansion-v63`
- Ladder step: `v63`

## Scope
- Expand Cranelift-supported bytecode surface while preserving VM fallback parity.
- Add support for intrinsic integer math subset on JIT path.

## Implementation Tasks
- Extend Cranelift support checks and translation for selected intrinsic ops.
- Add JIT-vs-VM parity tests for intrinsic subset programs.
- Add formal obligations for JIT support/pairwise equivalence.

## Gate Commands
- `cargo test -p oxvba-jit`
- `cargo test -p oxvba-host --lib`
- `./scripts/run-formal.ps1 -ProfileScope mvp-jit-surface-expansion-v63`
- `./scripts/run-matrix.ps1 -ProfileScope mvp-jit-surface-expansion-v63 -OutputDir docs/evidence/profiles/v63`

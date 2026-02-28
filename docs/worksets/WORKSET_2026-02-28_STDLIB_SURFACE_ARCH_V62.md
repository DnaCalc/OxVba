# WORKSET_2026-02-28_STDLIB_SURFACE_ARCH_V62

## Profile
- ID: `mvp-stdlib-surface-architecture-v62`
- Ladder step: `v62`

## Scope
- Split intrinsic surface into deterministic-core vs host-sensitive capability classes.
- Centralize intrinsic arity/surface registry at resolver boundary.
- Add evidence mapping for intrinsic surface inventory.

## Implementation Tasks
- Add `IntrinsicSurface` classification in compiler resolver.
- Refactor intrinsic call validation to use centralized spec metadata.
- Add intrinsic surface evidence CSV and structural validator script.

## Gate Commands
- `cargo test -p oxvba-compiler`
- `cargo test -p oxvba-host --lib`
- `./scripts/run-formal.ps1 -ProfileScope mvp-stdlib-surface-architecture-v62`
- `./scripts/run-matrix.ps1 -ProfileScope mvp-stdlib-surface-architecture-v62 -OutputDir docs/evidence/profiles/v62`

# PROFILE_STATUS_V62.md

## Profile
- ID: mvp-stdlib-surface-architecture-v62
- Ladder step: v62

## Scope Summary
- Split intrinsic surface into deterministic-core and host-sensitive capabilities, with centralized arity/surface metadata and evidence mapping.

## Gate Artifacts
- crates/oxvba-compiler/src/resolve.rs
- docs/evidence/runtime/INTRINSIC_SURFACE.csv
- scripts/validate-intrinsic-surface.ps1
- docs/evidence/profiles/v62/matrix_latest.csv
- docs/evidence/profiles/v62/gate_report.md
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile v62 is complete when FO-V62-* obligations are pass and required matrix cells for `v62` are green.

# PROFILE_STATUS_V81.md

## Profile
- ID: mvp-array-bounds-and-indexing-v81
- Ladder step: v81

## Scope Summary
- Added lower-bound-aware array declaration/reference parsing, including `Option Base` defaults.
- Added multi-dimensional bounds parsing and index linearization for current executable subset.
- Kept bound descriptor metadata (`rank`, `bounds`) aligned with declaration/reference alias mapping.

## Gate Artifacts
- scripts/run-formal.ps1
- scripts/run-matrix.ps1
- docs/worksets/WORKSET_2026-02-28_ARRAY_BOUNDS_INDEXING_V81.md
- docs/evidence/profiles/v81/matrix_latest.csv
- docs/evidence/profiles/v81/gate_report.md
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile `v81` is complete when FO-V81-* obligations are pass, required VM/JIT matrix cells are green for profile scope, and strict async Kani run `v81-kani` is started and tracked as `DG-V81-001`.

## Gate Result (2026-02-28)
- `FO-V81-001..003`: pass (`docs/evidence/formal/latest_run.md`).
- Matrix gate: pass (`docs/evidence/profiles/v81/gate_report.md`, required cells `2/2` green).
- Deferred strict formal lane: started (`v81-kani`), register entry `DG-V81-001` (`docs/evidence/formal/DEFERRED_GATES.md`).

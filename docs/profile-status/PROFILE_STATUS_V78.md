# PROFILE_STATUS_V78.md

## Profile
- ID: mvp-string-compare-search-v78
- Ladder step: v78

## Scope Summary
- Add `Option Compare` mode capture (`Binary`/`Text`/`Database`) in resolver output.
- Extend compare/search subset with executable `InStrRev` and `Like` lowering/runtime paths.
- Encode compare mode in string compare/search bytecode instructions (`InStr`, `InStrRev`, `StrComp`, `Like`).
- Add conformance fixtures for mode-scoped compare behavior and `InStrRev`/`Like` coverage.

## Gate Artifacts
- scripts/run-formal.ps1
- scripts/run-matrix.ps1
- docs/worksets/WORKSET_2026-02-28_STRING_COMPARE_SEARCH_V78.md
- docs/evidence/profiles/v78/matrix_latest.csv
- docs/evidence/profiles/v78/gate_report.md
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile `v78` is complete when FO-V78-* obligations are pass, required VM/JIT matrix cells are green for profile scope, and deferred strict Kani run `v78-kani` is started and registered as `DG-V78-001`.

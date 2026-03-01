# WORKSET_2026-03-01_NON_HAL_GAP_BASELINE_LOCK_V147.md

## Objective

Execute and stabilize profile scope `v147`: freeze the non-HAL completion baseline by extracting all `partial/planned` rows from:
- `docs/evidence/SPEC_CHECKLIST.md`
- `docs/evidence/language/COVERAGE_INDEX.csv`
- `docs/evidence/runtime/LIBRARY_CHECKLIST.csv`

and classifying each row as:
- `non-hal` (in-scope for `v147..v166`)
- `hal-adjacent` (excluded from current ladder scope)

## Deliverables

1. Reproducible baseline generator script:
- `scripts/build-non-hal-gap-baseline.ps1`

2. Frozen baseline artifacts:
- `docs/evidence/profiles/v147/non_hal_gap_baseline.csv`
- `docs/evidence/profiles/v147/non_hal_gap_baseline.md`

3. Profile status contract:
- `docs/profile-status/PROFILE_STATUS_V147.md`

## Execution Notes

- Baseline extraction is data-driven from current evidence/checklist files.
- Classification is heuristic but explicit and deterministic in script logic.
- This profile does not attempt semantic implementation changes; it locks scope for `v148+`.

## Closure Conditions

Profile `v147` is complete when:
1. baseline script executes successfully,
2. baseline artifacts are present and updated,
3. all `partial/planned` rows are classified into `non-hal` vs `hal-adjacent`,
4. profile status doc is published.

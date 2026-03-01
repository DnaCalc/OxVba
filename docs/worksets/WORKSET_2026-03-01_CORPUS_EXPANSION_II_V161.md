# WORKSET_2026-03-01_CORPUS_EXPANSION_II_V161.md

## Objective

Execute profile scope `v161`: expand and refresh conformance coverage for financial algorithm behavior and tolerance/error modes introduced in `v154..v156`.

## Scope

In scope for `v161`:
- add dedicated conformance fixtures for:
  - `NPV`/`IRR`/`MIRR` algorithmic subset outputs,
  - `Rate`/`NPer` algorithmic subset outputs,
  - mixed tolerance/error mode behavior combining non-convergent and convergent calls;
- add host-formal checks for fixture execution and expected output values.

Out of scope:
- exact Excel parity of financial conventions/rounding/iteration thresholds;
- broader financial function suite not yet implemented.

## Deliverables

- Conformance fixtures:
  - `conformance/tests/financial_algorithm_npv_irr_mirr_subset.bas`
  - `conformance/tests/financial_algorithm_rate_nper_subset.bas`
  - `conformance/tests/financial_tolerance_mixed_modes.bas`
  - updated `conformance/golden/smoke.csv`
- Host formal checks:
  - `crates/oxvba-host/src/engine.rs`
- Evidence/docs:
  - `docs/evidence/formal/obligations.csv`
  - `docs/evidence/language/COVERAGE_INDEX.csv`
  - `docs/evidence/runtime/LIBRARY_CHECKLIST.csv`
  - profile gate artifacts under `docs/evidence/profiles/v161/`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V161.md`

## Closure Conditions

Profile `v161` is complete when:
1. the financial fixture corpus is expanded with deterministic expected outputs,
2. host-formal checks validate algorithm and mixed-mode tolerance behavior,
3. VM/JIT conformance gates remain green with refreshed corpus.

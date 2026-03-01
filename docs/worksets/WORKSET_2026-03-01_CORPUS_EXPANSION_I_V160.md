# WORKSET_2026-03-01_CORPUS_EXPANSION_I_V160.md

## Objective

Execute profile scope `v160`: expand the conformance corpus across non-HAL areas targeted by recent profiles, specifically `Err` lifecycle/reset behavior, string-sentinel flows, UDT whole-value copy flows, and coercion edge normalization.

## Scope

In scope for `v160`:
- add new conformance fixtures and golden expectations for:
  - `Err.Clear` full-surface reset behavior,
  - `vbNullString` predicate behavior in value flow,
  - repeated whole-UDT overwrite/copy semantics,
  - `CVErr` normalization edge (`CVErr(-n)` vs `CVErr(n)`);
- add host-formal execution checks for these fixtures.

Out of scope:
- host-oracle parity closure for these semantics;
- HAL-adjacent behavior.

## Deliverables

- Conformance fixtures:
  - `conformance/tests/err_clear_full_surface_reset.bas`
  - `conformance/tests/string_vbnullstring_predicates.bas`
  - `conformance/tests/udt_whole_assignment_overwrite.bas`
  - `conformance/tests/coercion_cverr_abs_normalization.bas`
  - updated `conformance/golden/smoke.csv`
- Host formal checks:
  - `crates/oxvba-host/src/engine.rs`
- Evidence/docs:
  - `docs/evidence/formal/obligations.csv`
  - `docs/evidence/language/COVERAGE_INDEX.csv`
  - profile gate artifacts under `docs/evidence/profiles/v160/`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V160.md`

## Closure Conditions

Profile `v160` is complete when:
1. corpus fixtures land with deterministic golden outputs,
2. host-formal checks validate execution for each targeted area,
3. profile evidence is published with green VM/JIT conformance gates.

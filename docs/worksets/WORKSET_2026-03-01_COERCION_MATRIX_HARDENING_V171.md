# WORKSET_2026-03-01_COERCION_MATRIX_HARDENING_V171.md

## Objective

Execute profile scope `v171`: grow coercion-matrix regression coverage for edge `CVErr` range/normalization and predicate behavior.

## Scope

In scope for `v171`:
- add conformance fixture for `CVErr` range boundaries and predicate/type outcomes;
- update formal checks and profile status docs.

Out of scope:
- broad coercion-table redesign.

## Deliverables

- Conformance fixture + golden row:
  - `conformance/tests/coercion_cverr_range_predicates.bas`
  - `conformance/golden/smoke.csv`
- Formal checks:
  - `docs/evidence/formal/obligations.csv`
  - `crates/oxvba-host/src/engine.rs`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V171.md`

## Closure Conditions

Profile `v171` is complete when coercion-edge fixture runs green on VM/JIT conformance lanes and formal checks pass.

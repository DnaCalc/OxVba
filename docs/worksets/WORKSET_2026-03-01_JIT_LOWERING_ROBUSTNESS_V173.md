# WORKSET_2026-03-01_JIT_LOWERING_ROBUSTNESS_V173.md

## Objective

Execute profile scope `v173`: improve JIT lowering/fallback robustness coverage for newly hardened non-HAL semantics.

## Scope

In scope for `v173`:
- add JIT regression tests that prove fallback parity for new coercion/error-model fixtures;
- synchronize formal checks and profile status docs.

Out of scope:
- expanding Cranelift supported-op surface.

## Deliverables

- JIT tests:
  - `crates/oxvba-jit/src/lib.rs`
- Formal checks:
  - `docs/evidence/formal/obligations.csv`
  - `crates/oxvba-host/src/engine.rs`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V173.md`

## Closure Conditions

Profile `v173` is complete when new coercion/error regressions are covered by JIT fallback parity tests and validation lanes remain green.

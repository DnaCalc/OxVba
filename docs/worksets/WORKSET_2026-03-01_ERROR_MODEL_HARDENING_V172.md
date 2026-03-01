# WORKSET_2026-03-01_ERROR_MODEL_HARDENING_V172.md

## Objective

Execute profile scope `v172`: stress nested error-mode transitions and `Err` state interactions across `Resume Next` and `On Error GoTo` mode switching.

## Scope

In scope for `v172`:
- add conformance fixture covering nested error-mode transition sequence;
- wire formal checks and profile status records.

Out of scope:
- new host/HAL error integration.

## Deliverables

- Conformance fixture + golden row:
  - `conformance/tests/error_nested_mode_transitions.bas`
  - `conformance/golden/smoke.csv`
- Formal checks:
  - `docs/evidence/formal/obligations.csv`
  - `crates/oxvba-host/src/engine.rs`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V172.md`

## Closure Conditions

Profile `v172` is complete when nested error-mode transition fixture remains green on VM/JIT conformance lanes and formal checks pass.

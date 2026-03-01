# WORKSET_2026-03-01_FINANCIAL_TOLERANCE_MODEL_V156.md

## Objective

Execute profile scope `v156`: define and apply an explicit deterministic tolerance/iteration policy for financial solvers and return stable error-tag signals for non-convergence or invalid solve domains.

## Scope

In scope for `v156`:
- formalize bounded iteration/tolerance constants for `Rate` solving;
- convert non-convergence and invalid-domain branches in `Rate`/`NPer` into deterministic error-tag outputs;
- add conformance + host-formal coverage for non-converging/invalid financial calls.

Out of scope:
- Excel-oracle parity tuning for exact error-code mapping and edge-case sign conventions;
- generalized solver infrastructure beyond current financial intrinsics.

## Deliverables

- Runtime/host updates:
  - `crates/oxvba-vm/src/interpreter.rs`
  - `crates/oxvba-host/src/engine.rs`
- Conformance:
  - `conformance/tests/financial_tolerance_non_convergence.bas`
  - updated `conformance/golden/smoke.csv`
- Evidence/docs:
  - `docs/evidence/formal/obligations.csv`
  - `docs/evidence/runtime/LIBRARY_CHECKLIST.csv`
  - `docs/evidence/language/NON_HAL_COMPLETION_BACKLOG_2026-03-01.md`
  - profile gate artifacts under `docs/evidence/profiles/v156/`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V156.md`

## Closure Conditions

Profile `v156` is complete when:
1. financial solver non-convergence/invalid-domain paths return deterministic error tags,
2. conformance and formal checks assert this behavior directly,
3. profile/evidence artifacts are updated with the tolerance model status.

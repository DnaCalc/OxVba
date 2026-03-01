# WORKSET_2026-03-01_DIAGNOSTICS_TIMING_PASS_V157.md

## Objective

Execute profile scope `v157`: harden compile-time vs runtime diagnostic timing behavior for in-scope non-HAL language/runtime constructs and make phase classification directly testable.

## Scope

In scope for `v157`:
- add explicit phase-classified execution diagnostics in host execution flow;
- verify compile-time diagnostics preempt runtime execution when both are present in source;
- verify runtime diagnostics are raised only after successful compile/typecheck;
- add conformance fixture plus formal obligations for phase timing coverage.

Out of scope:
- exact VBA text parity for diagnostic messages;
- full oracle-validated compile/runtime timing matrix across all unsupported/host-sensitive paths.

## Deliverables

- Host diagnostic phase model and tests:
  - `crates/oxvba-host/src/engine.rs`
- Conformance fixture:
  - `conformance/tests/diagnostic_phase_compile_wins.bas`
  - updated `conformance/golden/smoke.csv`
- Evidence/docs:
  - `docs/evidence/formal/obligations.csv`
  - `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`
  - `docs/evidence/language/COVERAGE_INDEX.csv`
  - `docs/evidence/language/NON_HAL_COMPLETION_BACKLOG_2026-03-01.md`
  - profile gate artifacts under `docs/evidence/profiles/v157/`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V157.md`

## Closure Conditions

Profile `v157` is complete when:
1. compile-time and runtime diagnostic phases are distinguishable in host execution API/tests,
2. compile-time failures deterministically short-circuit runtime execution,
3. profile evidence and obligations are updated with passing gate artifacts.

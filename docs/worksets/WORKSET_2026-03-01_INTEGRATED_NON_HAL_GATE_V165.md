# WORKSET_2026-03-01_INTEGRATED_NON_HAL_GATE_V165.md

## Objective

Execute profile scope `v165`: run and publish the integrated non-HAL completion gate (formal + matrix + bench) for the `v147..v166` ladder.

## Scope

In scope for `v165`:
- run integrated gate lanes for profile scope `mvp-profile-v165`;
- capture matrix, conformance, benchmark, and integrated rollup artifacts under `docs/evidence/profiles/v165/`;
- wire formal checks for artifact and status-document presence.

Out of scope:
- new semantic feature work;
- HAL-oracle closure work.

## Deliverables

- Profile artifacts:
  - `docs/evidence/profiles/v165/integrated_gate.md`
  - `docs/evidence/profiles/v165/gate_report.md`
  - `docs/evidence/profiles/v165/matrix_latest.csv`
  - `docs/evidence/profiles/v165/benchmark_latest.md`
  - `docs/evidence/profiles/v165/benchmark_latest.csv`
- Formal obligations/tests:
  - `docs/evidence/formal/obligations.csv`
  - `crates/oxvba-host/src/engine.rs`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V165.md`

## Closure Conditions

Profile `v165` is complete when:
1. integrated formal/matrix/bench artifacts are present,
2. matrix required cells are green,
3. profile status and obligations are synchronized.

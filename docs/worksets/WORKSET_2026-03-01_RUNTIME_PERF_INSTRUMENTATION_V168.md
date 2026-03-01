# WORKSET_2026-03-01_RUNTIME_PERF_INSTRUMENTATION_V168.md

## Objective

Execute profile scope `v168`: expand runtime performance instrumentation around newly concrete non-HAL Err/string/financial paths.

## Scope

In scope for `v168`:
- extend conformance runner to support filtered workload selection;
- extend benchmark lane with focused subset workloads (`Err`/string/financial + coercion edges);
- publish profile status and formal checks for new instrumentation hooks.

Out of scope:
- semantic changes to language/runtime behavior.

## Deliverables

- Script updates:
  - `scripts/run-conformance.ps1`
  - `scripts/run-bench.ps1`
- Formal checks:
  - `docs/evidence/formal/obligations.csv`
  - `crates/oxvba-host/src/engine.rs`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V168.md`

## Closure Conditions

Profile `v168` is complete when:
1. conformance runner supports include-pattern filtering,
2. benchmark runner includes focused non-HAL subset workloads,
3. profile status and obligations are synchronized.

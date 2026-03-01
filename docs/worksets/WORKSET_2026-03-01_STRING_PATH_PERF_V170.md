# WORKSET_2026-03-01_STRING_PATH_PERF_V170.md

## Objective

Execute profile scope `v170`: optimize string-digit intrinsic execution paths without semantic drift.

## Scope

In scope for `v170`:
- replace avoidable char-vector materialization in slice/right/mid-statement paths with byte-slice operations over deterministic ASCII numeric text;
- keep existing return/error behavior intact;
- add formal checks asserting optimized path structure is present.

Out of scope:
- Unicode-aware semantics expansion;
- boundary marshalling/string storage redesign.

## Deliverables

- VM optimization:
  - `crates/oxvba-vm/src/interpreter.rs`
- Formal checks:
  - `docs/evidence/formal/obligations.csv`
  - `crates/oxvba-host/src/engine.rs`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V170.md`

## Closure Conditions

Profile `v170` is complete when optimized string-digit paths are active and full validation remains green.

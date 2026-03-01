# WORKSET_2026-03-01_ERR_LIFECYCLE_TRANSITIONS_V149.md

## Objective

Execute profile scope `v149`: implement deterministic `Err` lifecycle transitions at procedure boundaries and `Resume*` success-path clearing points.

## Scope

In scope for `v149`:
- VM lifecycle updates:
  - clear `Err` state on `Resume Next`, `Resume`, and `Resume <label>` successful paths;
  - centralize `Err` state reset in interpreter to avoid drift between instructions.
- Emitter lifecycle guards:
  - insert `ClearErr` at procedure entry and procedure exit boundaries in emitted bytecode;
  - preserve existing `GoSub`/label semantics by applying guards only at procedure boundaries.
- Coverage and regression:
  - VM unit tests for resume-clearing behavior;
  - compiler regression asserting procedure-boundary clear guards exist;
  - host/formal tests validating observable lifecycle behavior.
- Conformance fixtures:
  - `conformance/tests/err_resume_next_clears.bas`
  - `conformance/tests/err_proc_call_boundary_clears.bas`
  - expected outputs in `conformance/golden/smoke.csv`.

Out of scope:
- full VBA parity for all `Err` fields and all host-dependent lifecycle nuances;
- oracle-driven final lifecycle table closure (tracked in deferred oracle topics).

## Deliverables

- Runtime/compiler changes:
  - `crates/oxvba-vm/src/interpreter.rs`
  - `crates/oxvba-compiler/src/emit.rs`
  - `crates/oxvba-compiler/src/lib.rs`
  - `crates/oxvba-host/src/engine.rs`
- Conformance additions:
  - `conformance/tests/err_resume_next_clears.bas`
  - `conformance/tests/err_proc_call_boundary_clears.bas`
  - `conformance/golden/smoke.csv`
- Evidence/checklist updates:
  - `docs/evidence/language/COVERAGE_INDEX.csv`
  - `docs/evidence/SPEC_CHECKLIST.md`
  - `docs/evidence/runtime/LIBRARY_CHECKLIST.csv`
  - `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`
  - `docs/evidence/formal/obligations.csv`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V149.md`

## Closure Conditions

Profile `v149` is complete when:
1. resume-success and procedure-boundary lifecycle clears execute deterministically in VM/JIT conformance lanes,
2. new lifecycle fixtures are green in matrix artifacts,
3. formal obligations and profile status are published with updated evidence notes.

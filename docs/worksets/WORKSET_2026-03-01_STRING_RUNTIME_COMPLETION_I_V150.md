# WORKSET_2026-03-01_STRING_RUNTIME_COMPLETION_I_V150.md

## Objective

Execute profile scope `v150`: replace remaining projection-style behavior in core string advanced intrinsics by making `Join` concrete for the current array-tag runtime domain.

## Scope

In scope for `v150`:
- runtime behavior update:
  - `Join` no longer acts as scalar pass-through for array-tag inputs;
  - for array-tag values (`ARRAY_TAG_BASE + n`), `Join` now returns deterministic element count `n` in the current executable model.
- regression and conformance coverage:
  - VM unit coverage for array-tag `Join` behavior;
  - host formal checks for executable `Array(...) -> Join(...)` flow;
  - conformance fixture for array-tag-aware `Join`.
- documentation/evidence updates for string advanced intrinsic notes.

Out of scope:
- full VBA `Join` string materialization semantics over real string arrays;
- locale-aware delimiter/string formatting parity;
- host boundary marshaling of true string arrays.

## Deliverables

- Runtime/tests:
  - `crates/oxvba-vm/src/interpreter.rs`
  - `crates/oxvba-host/src/engine.rs`
- Conformance:
  - `conformance/tests/string_join_array_tag_count.bas`
  - `conformance/golden/smoke.csv`
- Evidence/docs:
  - `docs/evidence/language/COVERAGE_INDEX.csv`
  - `docs/evidence/runtime/LIBRARY_CHECKLIST.csv`
  - `docs/evidence/SPEC_CHECKLIST.md`
  - `docs/evidence/formal/obligations.csv`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V150.md`

## Closure Conditions

Profile `v150` is complete when:
1. `Join` array-tag count behavior is executable and covered by VM/JIT conformance,
2. formal obligations for the new behavior are present and green under non-blocking policy,
3. profile status and evidence/checklist notes are updated.

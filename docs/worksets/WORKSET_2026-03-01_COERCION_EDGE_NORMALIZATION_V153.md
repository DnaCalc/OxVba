# WORKSET_2026-03-01_COERCION_EDGE_NORMALIZATION_V153.md

## Objective

Execute profile scope `v153`: normalize deterministic coercion/introspection handling for `Null`/`Empty`/`Error` value paths in the non-HAL runtime subset.

## Scope

In scope for `v153`:
- introduce explicit runtime tag helpers for `Empty`, `Null`, and `CVErr`-encoded error values;
- preserve `CVErr(...)` as an intrinsic operation in binding and lower it into deterministic error-tag encoding;
- normalize predicate behavior:
  - `IsError` recognizes only encoded error-tag range;
  - `IsNumeric` rejects `Null`/`Empty`/`Error` sentinel tags;
  - `VarType` reports distinct tags for `Empty`/`Null`/`Error` in deterministic subset.
- refresh compiler/host/conformance/formal coverage for these behaviors.

Out of scope:
- full VBA oracle parity for arithmetic/assignment propagation of `Null`/`Empty`/`Error`;
- full COM boundary `VARIANT` subtype roundtrip for `CVErr` across host interop edges.

## Deliverables

- Runtime/compiler updates:
  - `crates/oxvba-runtime/src/value_tags.rs`
  - `crates/oxvba-runtime/src/lib.rs`
  - `crates/oxvba-compiler/src/resolve.rs`
  - `crates/oxvba-compiler/src/emit.rs`
  - `crates/oxvba-vm/src/interpreter.rs`
  - `crates/oxvba-host/src/engine.rs`
- Conformance:
  - `conformance/tests/coercion_null_empty_error_predicates.bas`
  - updates to existing `CVErr`/introspection fixtures and `conformance/golden/smoke.csv`
- Evidence/docs:
  - `docs/evidence/formal/obligations.csv`
  - `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`
  - `docs/evidence/language/NON_HAL_COMPLETION_BACKLOG_2026-03-01.md`
  - profile gate artifacts under `docs/evidence/profiles/v153/`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V153.md`

## Closure Conditions

Profile `v153` is complete when:
1. `Null`/`Empty`/`CVErr` deterministic tags are distinct and consumed consistently by coercion/introspection predicates,
2. VM/JIT conformance remains green with updated fixtures,
3. formal obligations and profile evidence/status artifacts are updated.

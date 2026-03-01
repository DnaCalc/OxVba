# WORKSET_2026-03-01_STRING_SENTINEL_TIGHTENING_V151.md

## Objective

Execute profile scope `v151`: tighten `vbNullString` non-boundary semantics in compile-time typing flow by rejecting numeric-target assignment and argument routes.

## Scope

In scope for `v151`:
- typecheck tightening:
  - reject `vbNullString` assignment into non-`String`/`Variant` targets;
  - reject passing `vbNullString` into typed parameters outside `String`/`Variant`.
- regression and conformance coverage:
  - compiler regression tests for assignment and call-argument rejection;
  - host/formal test for executable compile-time rejection path;
  - conformance error fixture for numeric assignment misuse.
- evidence updates for string/null semantics tracking.

Out of scope:
- full runtime sentinel representation split between `vbNullString` and empty-string payload;
- boundary/interop parity for BSTR null-pointer semantics;
- Excel oracle closure for full coercion matrix.

## Deliverables

- Compiler/host updates:
  - `crates/oxvba-compiler/src/typecheck.rs`
  - `crates/oxvba-compiler/src/lib.rs`
  - `crates/oxvba-host/src/engine.rs`
- Conformance:
  - `conformance/tests/string_vbnullstring_long_error.bas`
  - `conformance/golden/smoke.csv`
- Evidence/docs:
  - `docs/evidence/formal/obligations.csv`
  - `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`
  - `docs/evidence/SPEC_CHECKLIST.md`
  - `docs/evidence/language/COVERAGE_INDEX.csv`
  - `docs/evidence/language/NON_HAL_COMPLETION_BACKLOG_2026-03-01.md`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V151.md`

## Closure Conditions

Profile `v151` is complete when:
1. `vbNullString` numeric-target misuse is rejected with stable diagnostics in compiler/host/conformance lanes,
2. formal obligations for this guard are present and green (non-blocking policy unchanged),
3. profile/evidence updates record the tightened subset and remaining oracle-dependent parity work.

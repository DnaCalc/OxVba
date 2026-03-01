# WORKSET_2026-03-01_UDT_VALUE_SEMANTICS_V152.md

## Objective

Execute profile scope `v152`: harden deterministic UDT value semantics by supporting whole-value assignment (`b = a`) through explicit field-alias copy lowering.

## Scope

In scope for `v152`:
- resolver/typecheck/emitter support for whole-UDT assignment when both sides are compatible flattened UDT aliases;
- deterministic lowering of whole-value assignment into per-field slot copies;
- regression and conformance coverage for whole-value UDT copy semantics;
- formal obligations and profile/evidence updates.

Out of scope:
- full VBA UDT initialization-order parity across all declaration contexts;
- deep-copy/reference-field semantics beyond current flattened alias runtime model;
- oracle-dependent edge behavior that requires Excel differential validation.

## Deliverables

- Compiler/host updates:
  - `crates/oxvba-compiler/src/resolve.rs`
  - `crates/oxvba-compiler/src/typecheck.rs`
  - `crates/oxvba-compiler/src/emit.rs`
  - `crates/oxvba-compiler/src/lib.rs`
  - `crates/oxvba-host/src/engine.rs`
- Conformance:
  - `conformance/tests/udt_whole_assignment_copy.bas`
  - `conformance/golden/smoke.csv`
- Evidence/docs:
  - `docs/evidence/formal/obligations.csv`
  - `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`
  - `docs/evidence/SPEC_CHECKLIST.md`
  - `docs/evidence/language/COVERAGE_INDEX.csv`
  - `docs/evidence/language/NON_HAL_COMPLETION_BACKLOG_2026-03-01.md`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V152.md`

## Closure Conditions

Profile `v152` is complete when:
1. whole-UDT assignment lowers deterministically to field copy semantics and executes in VM/JIT conformance lanes,
2. formal obligations for UDT copy semantics are present and green under non-blocking formal policy,
3. profile/evidence artifacts record delivered behavior and remaining oracle-dependent parity topics.

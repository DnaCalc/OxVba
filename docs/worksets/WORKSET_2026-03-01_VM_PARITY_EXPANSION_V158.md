# WORKSET_2026-03-01_VM_PARITY_EXPANSION_V158.md

## Objective

Execute profile scope `v158`: expand interpreter-level parity coverage for recently concretized runtime behavior (`v153..v157`), with direct VM tests and source-level fixtures validating equivalent outcomes.

## Scope

In scope for `v158`:
- extend VM unit coverage for:
  - algorithmic financial `Rate`/`NPer` instruction execution outputs,
  - financial non-convergence error-tag signaling,
  - `VarType`/`IsNumeric` handling across `Empty`/`Null`/`CVErr`/array tags;
- add a focused conformance fixture for `VarType`/`IsNumeric` sentinel-tag behavior;
- add host-formal checks proving VM-path execution parity for these behaviors.

Out of scope:
- JIT support-surface expansion (covered by `v159`);
- oracle parity closure for financial and coercion edge semantics.

## Deliverables

- VM/host updates:
  - `crates/oxvba-vm/src/interpreter.rs`
  - `crates/oxvba-host/src/engine.rs`
- Conformance:
  - `conformance/tests/introspection_vartype_isnumeric_tags.bas`
  - updated `conformance/golden/smoke.csv`
- Evidence/docs:
  - `docs/evidence/formal/obligations.csv`
  - `docs/evidence/language/COVERAGE_INDEX.csv`
  - profile gate artifacts under `docs/evidence/profiles/v158/`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V158.md`

## Closure Conditions

Profile `v158` is complete when:
1. newly concrete runtime paths have direct VM unit coverage,
2. source-level conformance and host-formal checks reflect the same outcomes,
3. profile evidence is published with passing gates.

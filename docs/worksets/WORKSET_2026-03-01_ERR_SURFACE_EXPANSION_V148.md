# WORKSET_2026-03-01_ERR_SURFACE_EXPANSION_V148.md

## Objective

Execute and stabilize profile scope `v148`: expand executable `Err` member surface in the current deterministic runtime model.

## Scope

In scope for `v148`:
- parser/resolver aliasing for additional `Err` members:
  - `Err.Description`
  - `Err.Source`
  - `Err.HelpContext`
  - `Err.HelpFile`
  - `Err.LastDllError`
- typecheck routing so these members remain valid under `Option Explicit`.
- emission/runtime mapping in current integer-slot VM model:
  - `Err.Number` and `Err.Description` map to current error code value.
  - `Err.Source`, `Err.HelpContext`, `Err.HelpFile`, `Err.LastDllError` map to deterministic `0` subset values.
- conformance fixture and golden expectation for this member-read subset.

Out of scope:
- Full text/value parity for string-oriented members.
- Full lifecycle parity and host-specific semantics (tracked in deferred oracle gates).

## Deliverables

- Compiler updates in `resolve.rs`, `typecheck.rs`, and `emit.rs`.
- Conformance fixture:
  - `conformance/tests/err_surface_fields_subset.bas`
  - `conformance/golden/smoke.csv` update
- Coverage/checklist note updates:
  - `docs/evidence/language/COVERAGE_INDEX.csv`
  - `docs/evidence/SPEC_CHECKLIST.md`
  - `docs/evidence/runtime/LIBRARY_CHECKLIST.csv`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V148.md`

## Closure Conditions

Profile `v148` is complete when:
1. compiler accepts expanded `Err` member reads under `Option Explicit`,
2. VM/JIT conformance fixture for member-read subset is green,
3. profile status and evidence notes reflect the expanded-but-partial state.

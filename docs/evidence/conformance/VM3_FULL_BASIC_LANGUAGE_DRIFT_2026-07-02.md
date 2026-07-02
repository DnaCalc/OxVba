# VM3 Full Basic-Language Drift Reconciliation

Date: 2026-07-02
Bead: `bd-5kqj`
Split from: `bd-9sed.18`

## Context

After the W10 pending-row oracle refresh promoted all 29 originally pending
fixtures, the broader default conformance gate still failed:

- Command: `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language`
- Starting result: 70 mismatches over 192 files.

This drift predates the final W10 pending-row promotions and is not being
treated as a VBA compatibility target or as accepted legacy behavior. It is now
tracked explicitly in `bd-5kqj`.

## Reconciliation Performed

Two live Excel oracle passes were captured with PID-scoped dialog handling:

- `docs/evidence/conformance/oracle_captures/conformance_oracle_basic_scalar_drift_20260702/`
  - 42 scalar-looking drift rows.
  - 41 `ok`, 1 `error`, 0 skipped.
  - 42 old-golden mismatches; 38 rows now pass after oracle-backed golden
    refresh and one vm3 `Round` behavior fix.
- `docs/evidence/conformance/oracle_captures/conformance_oracle_basic_remaining_drift_20260702/`
  - 31 remaining drift rows after the scalar refresh.
  - 20 `ok`, 8 `error`, 3 skipped synthetic collection rows.
  - The VBE modal handler caught compile-error dialogs and restarted Excel
    sessions where needed; no owned Excel process was left running after the
    capture.

The scalar refresh updated `conformance/golden/values.csv` to use Excel's
retained subtype shape (`i16`, `bool`, `u8`, `currency`, `decimal`, and `f64`
where Excel reports those tags) rather than the old broad `i32` fallback
expectations. Rows where vm3 still differs from Excel were also updated to the
Excel target so the remaining failures are true vm3/VBA gaps.

The remaining-row refresh updated additional stale goldens for enum/module
constant/error-surface/property/array-observable rows where the Excel capture
provided a valid target. UDT rows were not blindly rewritten to the generic
oracle error shape because the current `EncodeValue(ByVal Variant)` harness
cannot encode private UDT locals cleanly; that is tracked as composite
retained-value follow-up work. Synthetic `CollectionAdd`/`CollectionItem`/
`CollectionCount`/`CollectionRemove` fixtures were left as follow-up work
because they are not real VBA language fixtures.

The starting 70-mismatch surface was captured after the earlier W10 `Rnd` and
`Single` retained-value formatter fixes; this bead reconciles that post-W10
baseline.

This pass also removed a legacy vm3 behavior for `Round`: live Excel reports
runtime error 5 for `Round(19, -1)`, while vm3 previously accepted negative
decimal-place counts. `Round` now rejects negative `numdecimalplaces`; the old
differential regression that protected negative shifted rounding was replaced
with an error-5 regression.

## Current Gate

After the oracle-backed golden refreshes and `Round` fix:

- Command: `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language`
- Current result: 27 mismatches over 192 files.
- Log: `target/conformance_basic_language_vm_after_remaining_oracle_20260702.log`

The 27 remaining mismatches are classified and split as follows:

- `bd-5kqj.1` composite retained-value dump shape:
  array/`ReDim`/UDT rows where vm3 currently dumps structural SAFEARRAY/record
  debug values while the VBA-observable fixture target should be scalar or the
  oracle encoder must be extended.
- `bd-5kqj.2` CVErr and error-state edge semantics:
  `coercion_cverr_abs_normalization.bas`,
  `error_nested_mode_transitions.bas`,
  `regression_cverr_error_resume_bridge.bas`.
- `bd-5kqj.3` compile diagnostics:
  `default_type_param_defobj_error.bas`,
  `function_return_explicit_as_precedence_error.bas`.
- `bd-5kqj.4` module-level `Const` parsing/execution:
  `module_const_basic.bas`.
- `bd-5kqj.5` non-VBA synthetic collection fixtures:
  `object_collection_add_item.bas`,
  `object_collection_count_chain.bas`,
  `object_collection_remove_count.bas`.
- `bd-5kqj.6` `Err.Source` default naming:
  `err_surface_fields_subset.bas` reports `VBAProject` in Excel and `Main` in
  vm3.
- `bd-5kqj.7` Variant small-integer arithmetic subtype:
  `for_exit_for_basic.bas`.
- `bd-5kqj.8` `For Each` array loop-variable final state:
  `for_each_array_literal_basic.bas` and the loop-variable part of
  `for_each_array_variable_basic.bas`.

## Checks

- `cargo test -q -p oxvba-differential --test round_negative_digits_vm3`
- `cargo test -q -p oxvba-lib round`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern stdlib_math_primitives.bas`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern <42 scalar oracle rows>`
  - Current expected result: 3 mismatches, all split to follow-up beads.
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language`
  - Current expected result: 27 mismatches over 192 files, all classified above.

## Fresh-Eyes Review

Reviewed the updated golden rows against both raw Excel captures and current
vm3 outputs. Rows were only refreshed to real Excel targets when the generic
oracle capture was a valid observation for that fixture shape. UDT and synthetic
collection rows were explicitly split instead of being treated as VBA parity.
The `Round(19, -1)` change was backed by the Excel runtime error 5 capture and
by a focused differential regression. No remaining mismatch is left only in
chat; every residual category has an open bead.

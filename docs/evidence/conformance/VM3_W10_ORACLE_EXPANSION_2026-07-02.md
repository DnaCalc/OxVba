# VM3 W10 Oracle Expansion Progress

Date: 2026-07-02
Bead: `bd-9sed.11`
Residual bead: `bd-9sed.18`
Plan: `docs/VM3_COMPLETION_AND_VM2_RETIREMENT_PLAN.md#W10`

## Outcome

This records completion of the W10 pending-row oracle refresh. It is not an
umbrella-workset or IP gate closure: the broader full `basic-language`
retained-value drift is split to `bd-5kqj`.

The pending conformance oracle pass ran 29 `value-oracle-pending` fixtures
against live Excel 16.0 with the dialog guardian enabled:

- Capture directory:
  `docs/evidence/conformance/oracle_captures/conformance_oracle_w10_pending_20260702/`
- Oracle summary: 29 total, 23 `ok`, 6 `error`, 0 skipped.
- PID-scoped UIA handling was active through
  `_vba_dialog_handler.ps1`, `vba_dialog_handler.log`, and
  `excel_dialog_guardian.log`.

Fifteen rows were promoted from `value-oracle-pending` to `basic-language`
because live Excel and vm3 agree under the existing committed retained-value
comparison shape:

- `financial_tolerance_mixed_modes.bas`
- `financial_tolerance_non_convergence.bas`
- `stdlib_advanced_replace_trim.bas`
- `stdlib_advanced_split_join.bas`
- `stdlib_date_add_diff.bas`
- `stdlib_date_string_policy.bas`
- `stdlib_datetime_expansion.bas`
- `stdlib_format_core.bas`
- `stdlib_instr_case_ops.bas`
- `stdlib_slice_ops.bas`
- `stdlib_string_expansion_core.bas`
- `string_compare_option_binary.bas`
- `string_compare_option_text.bas`
- `string_mid_statement_mutation.bas`
- `string_slice_ops_dollar.bas`

A follow-up typed scalar oracle pass then reran the remaining 14 pending rows
after `scripts/run-conformance-oracle.ps1` was updated to preserve scalar
subtypes instead of coercing them through `CLng`. Capture directory:

- `docs/evidence/conformance/oracle_captures/conformance_oracle_w10_typed_pending_20260702/`

That pass promoted four more exact scalar rows:

- `jit_intrinsic_math_subset.bas`
- `stdlib_date_serial_value.bas`
- `stdlib_financial_zero_rate.bas`
- `stdlib_math_transcendental_identity.bas`

The conformance runner now compares `f64:` tokens numerically with a tight
tolerance, so Excel oracle values can keep their VBA-rendered decimal text
without failing on harmless last-bit or decimal-format differences. That allowed
two additional typed scalar rows to move into `basic-language`:

- `financial_algorithm_rate_nper_subset.bas`
- `stdlib_time_serial_value.bas`

Five pending fixtures were then reshaped so `Main` retains only scalar
observables and helper procedures keep array/object locals out of the retained
value dump. Live Excel recapture:

- `docs/evidence/conformance/oracle_captures/conformance_oracle_w10_reshaped_scalar_20260702/`

These rows now match vm3 exactly and were promoted:

- `object_identity_is_nothing.bas`
- `object_identity_is_same_and_different.bas`
- `stdlib_array_introspection_bounds.bas`
- `stdlib_array_introspection_types.bas`
- `string_join_array_tag_count.bas`

Two financial fixtures were then promoted as compile/error-shape rows after the
legacy scalar/vararg financial fallback was removed. The typed live-Excel pass
reported both rows as dialog/compile errors (`0x800A9C68`), and vm3 now rejects
the same fixed-signature calls during binding instead of accepting scalar
cash-flow arguments:

- `financial_algorithm_npv_irr_mirr_subset.bas`
- `stdlib_random_financial_expansion.bas`

The final pending row, `stdlib_rnd_isolated.bas`, was reshaped to the
documented repeatable-sequence form (`Rnd -1` immediately before `Randomize 1`)
and promoted after vm3's `Rnd`/`Randomize` seed transforms were corrected from
live Excel probes:

- `docs/evidence/conformance/oracle_captures/conformance_oracle_w10_rnd_repeatable_20260702/`
- `docs/evidence/conformance/oracle_captures/conformance_oracle_w10_rnd_cdbl_probe_20260702/`
- `docs/evidence/conformance/oracle_captures/conformance_oracle_w10_rnd_seed_matrix_20260702/`

Fresh-eyes review of those probes also exposed a retained-value formatter bug:
`Single` variants were being printed through the `Double` accessor as `f64:0`.
The CLI formatter now reads `Single` payloads through `as_f32()` before widening
for the `f64:` text surface.

W5/W6 fresh-host COM evidence already exists and remains part of the W10
regression-net story:

- `docs/evidence/com/VM3_W5_COM_FOREIGN_LEGS_2026-07-02.md`
- `docs/evidence/com/VM3_W6_GETOBJECT_2026-07-02.md`

During the pass, the CLI retained-value formatter was corrected so `Date`
variants use the date payload accessor instead of the numeric `Double` accessor.
This keeps `DateSerial`/`TimeSerial` retained values from dumping as `f64:0`.

## Residual

The original 29 W10 `value-oracle-pending` rows are all reconciled and promoted
to `basic-language`.

Open items:

- The Excel oracle harness still has a legacy retained-value encoder that
  collapses arrays/objects; scalar subtype preservation is now in place for new
  captures.
- A full default conformance run still reports pre-existing drift:
  `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language` failed with
  70 mismatches over 192 files. This is split to `bd-5kqj` with evidence in
  `docs/evidence/conformance/VM3_FULL_BASIC_LANGUAGE_DRIFT_2026-07-02.md`.

## Checks

- `cargo test -p oxvba-cli format_variant_value_preserves_date_payloads -- --nocapture`
- `cargo run -q -p oxvba-cli --bin oxvba-cli -- run conformance/tests/stdlib_date_serial_value.bas --dump-values`
- `cargo run -q -p oxvba-cli --bin oxvba-cli -- run conformance/tests/stdlib_time_serial_value.bas --dump-values`
- `cargo run -q -p oxvba-cli --bin oxvba-cli -- run conformance/tests/stdlib_rnd_isolated.bas --dump-values`
- `cargo run -q -p oxvba-cli --bin oxvba-cli -- run conformance/tests/financial_algorithm_rate_nper_subset.bas --dump-values`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern <15 promoted rows>`
  - Passed: `conformance run: ok (15 files, backend=vm, suite=basic-language)`.
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern <4 typed scalar promoted rows>`
  - Passed: `conformance run: ok (4 files, backend=vm, suite=basic-language)`.
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern <2 f64-tolerant promoted rows>`
  - Passed: `conformance run: ok (2 files, backend=vm, suite=basic-language)`.
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern <5 reshaped scalar promoted rows>`
  - Passed: `conformance run: ok (5 files, backend=vm, suite=basic-language)`.
- `cargo test -q -p oxvba-bind --test bind_roundtrip migrated_vba`
  - Passed: fixed-arity migrated VBA intrinsics reject surplus positional
    arguments.
- `cargo test -q -p oxvba-symbol surface_preserves_paramarray_metadata`
  - Passed: cross-project export surfaces keep true `ParamArray` metadata
    separate from fixed-arity calls.
- `cargo test -q -p oxvba-lib financial_cashflow_functions_reject_scalar_values`
  - Passed: `NPV`/`IRR`/`MIRR` cash-flow helpers reject scalar cash-flow values.
- `cargo test -q -p oxvba-bind --test cross_project cross_bundle_free_function`
  - Passed: adjacent cross-bundle free-function named/optional/default binding
    still works.
- `cargo test -q -p oxvba-bind --test bind_roundtrip paramarray`
  - Passed: local `ParamArray` packing/regression tests still work.
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern financial_algorithm_npv_irr_mirr_subset.bas,stdlib_random_financial_expansion.bas`
  - Passed: `conformance run: ok (2 files, backend=vm, suite=basic-language)`.
- `cargo test -q -p oxvba-lib rnd_`
  - Passed: exact Excel-observed default, negative seed, and
    `Rnd -1`/`Randomize 1` seed-state regressions.
- `cargo test -q -p oxvba-cli format_variant_value_preserves -- --nocapture`
  - Passed: retained `Date` and `Single` formatter payload regressions.
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern stdlib_rnd_isolated.bas`
  - Passed: `conformance run: ok (1 files, backend=vm, suite=basic-language)`.
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern <29 W10 promoted rows>`
  - Passed: `conformance run: ok (29 files, backend=vm, suite=basic-language)`.
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern conversion_cint_basic.bas`
  - Expected current state: fails, proving the pre-existing typed retained-value
    baseline drift that `bd-5kqj` must resolve.
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language`
  - Current state after W10 pending-row promotion: fails with 70 mismatches over
    192 files; split to `bd-5kqj`.

## Fresh-Eyes Review

Reviewed the promoted set against the raw Excel captures and scratch vm3 output.
Rows with compile-vs-runtime error-shape differences were only promoted after
vm3 moved to the Excel compile/bind error shape. The Rnd row was only promoted
after live Excel seed probes matched vm3's exact retained values. The fresh pass
also caught and fixed the `Single` retained-value formatter bug before commit.
The broader full-gate drift is explicitly split rather than treated as accepted
legacy behavior.

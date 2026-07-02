# VM3 W10 Oracle Expansion Progress

Date: 2026-07-02
Bead: `bd-9sed.11`
Residual bead: `bd-9sed.18`
Plan: `docs/VM3_COMPLETION_AND_VM2_RETIREMENT_PLAN.md#W10`

## Outcome

This is a W10 progress record, not W10 closure.

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

W5/W6 fresh-host COM evidence already exists and remains part of the W10
regression-net story:

- `docs/evidence/com/VM3_W5_COM_FOREIGN_LEGS_2026-07-02.md`
- `docs/evidence/com/VM3_W6_GETOBJECT_2026-07-02.md`

During the pass, the CLI retained-value formatter was corrected so `Date`
variants use the date payload accessor instead of the numeric `Double` accessor.
This keeps `DateSerial`/`TimeSerial` retained values from dumping as `f64:0`.

## Residual

The remaining W10 work is not being treated as a legacy compatibility target.
It is split to `bd-9sed.18` and W10 remains in progress.

Open items:

- The Excel oracle harness still has a legacy retained-value encoder that
  collapses arrays/objects; scalar subtype preservation is now in place for new
  captures.
- A full default conformance run reported pre-existing drift:
  `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language` failed with
  70 mismatches over the prior 163-file basic suite before the newly promoted
  rows can be called a clean full-gate expansion.
- `financial_algorithm_npv_irr_mirr_subset.bas` produced an RPC failure after
  the watchdog killed Excel on deadline; that result is unstable and was not
  promoted.
- The remaining pending rows need typed live-Excel recapture, vm3 fixes, or an
  explicit split:
  `financial_algorithm_npv_irr_mirr_subset.bas`,
  `financial_algorithm_rate_nper_subset.bas`,
  `object_identity_is_nothing.bas`,
  `object_identity_is_same_and_different.bas`,
  `stdlib_array_introspection_bounds.bas`,
  `stdlib_array_introspection_types.bas`,
  `stdlib_random_financial_expansion.bas`,
  `stdlib_rnd_isolated.bas`,
  `stdlib_time_serial_value.bas`, and
  `string_join_array_tag_count.bas`.

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
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern conversion_cint_basic.bas`
  - Expected current state: fails, proving the pre-existing typed retained-value
    baseline drift that `bd-9sed.18` must resolve.
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language`
  - Expected current state: fails with 70 mismatches over 163 files before this
    W10 promotion.

## Fresh-Eyes Review

Reviewed the promoted set against the raw Excel captures and scratch vm3 output.
Rows with array/object dumps, compile-vs-runtime error-shape differences,
float-format or algorithm tolerance questions, and random-number divergence were
left pending. This avoids turning the older OxVBA comparison shape into a VBA
compatibility claim.

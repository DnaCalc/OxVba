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

`bd-5kqj.5` replaced those three synthetic collection fixtures with real VBA
`Collection` source using `Dim ... As New Collection`, `.Add`, `.Item`, default
member access, `.Count`, and `.Remove`. A targeted Excel oracle rerun captured
the real retained scalar values and had 3 `ok`, 0 `error`, and 0 skipped rows:
`docs/evidence/conformance/oracle_captures/conformance_oracle_collection_real_vba_20260703/`.

`bd-5kqj.6` aligned bare single-source host/CLI execution with Excel's default
VBProject name, so source-less runtime faults now default `Err.Source` to
`VBAProject` instead of the synthetic module name `Main`.

`bd-5kqj.7` replaced the legacy vm3 broad-`Long` fallback for retained Variant
small-integer arithmetic with the Excel-observed subtype ladder. Integral
Variant arithmetic now returns the smallest carrier at least as wide as the
operands that can hold the result (`Byte` -> `Integer` -> `Long` -> `Double`,
with `Boolean` joining the `Integer` lane), while `LongLong` remains
`LongLong` unless it overflows to `Double`. The implicit default `For ... Next`
step literal now uses an `Integer` carrier, matching Excel's retained subtype
for the affected loop row.

`bd-5kqj.8` aligned normal `For Each` completion with Excel/VBA by clearing
the Variant loop variable when the enumerator is exhausted. The existing Excel
captures already showed array-literal and array-variable `For Each` leave the
loop variable `Empty`; an additional guarded Excel probe confirmed the same
normal-completion rule for `Collection` enumeration while preserving the last
visited item in a separate local.

`bd-5kqj.1` reconciled the remaining composite retained-value rows by reshaping
the basic-language fixtures so `Main` retains only VBA-observable scalar values.
Arrays and UDT records now live in helper functions/subs for those rows, while
dedicated VM/package lanes continue to inspect structural SAFEARRAY/record
carriers. A fresh guarded Excel oracle run captured the scalarized array,
`ReDim`, `Erase`, `For Each`, and UDT values; `udt_whole_assignment_overwrite`
was corrected to use explicit UDT field declarations because the old implicit
field declaration was not real VBA compile/runtime behavior.

The starting 70-mismatch surface was captured after the earlier W10 `Rnd` and
`Single` retained-value formatter fixes; this bead reconciles that post-W10
baseline.

This pass also removed a legacy vm3 behavior for `Round`: live Excel reports
runtime error 5 for `Round(19, -1)`, while vm3 previously accepted negative
decimal-place counts. `Round` now rejects negative `numdecimalplaces`; the old
differential regression that protected negative shifted rounding was replaced
with an error-5 regression.

## Current Gate

After the oracle-backed golden refreshes, `Round` fix, the `bd-5kqj.4`
module-level `Const` parser fix, the `bd-5kqj.3` compile-diagnostic fix, the
`bd-5kqj.2` CVErr/Resume-state fix, and the `bd-5kqj.5` collection-fixture
replacement, the `bd-5kqj.6` `Err.Source` default-name fix, and the
`bd-5kqj.7` Variant small-integer arithmetic fix, and the `bd-5kqj.8`
`For Each` loop-variable final-state fix, and the `bd-5kqj.1` composite
retained-value fixture scalarization:

- Command: `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language`
- Current result: `ok` over 192 files.
- Latest evidence: terminal output from the `bd-5kqj.1` closure run.
- Earlier 27-mismatch log before the `Const` fix:
  `target/conformance_basic_language_vm_after_remaining_oracle_20260702.log`

No basic-language mismatch row remains in this epic. Structural SAFEARRAY and
record carrier inspection remains covered by dedicated VM/package and COM lanes
rather than by the fast scalar conformance gate.

## Checks

- `cargo test -q -p oxvba-differential --test round_negative_digits_vm3`
- `cargo test -q -p oxvba-lib round`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern stdlib_math_primitives.bas`
- `cargo test -q -p oxvba-syntax`
- `cargo test -q -p oxvba-syntax function_type_suffix_rejects_explicit_return_type`
- `cargo test -q -p oxvba-bind byval_object_param`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern default_type_param_defobj_error.bas,function_return_explicit_as_precedence_error.bas`
- `cargo test -q -p oxvba-lib cverr_accepts_unsigned_error_code_range_only`
- `cargo test -q -p oxvba-bind on_error_resume_next_caught_error_does_not_arm_resume`
- `cargo test -q -p oxvba-bind cverr_out_of_range_raises_and_skips_assignment_under_resume_next`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern coercion_cverr_abs_normalization.bas,coercion_cverr_range_predicates.bas,error_nested_mode_transitions.bas,regression_cverr_error_resume_bridge.bas`
- `./scripts/run-conformance-oracle.ps1 -OutputDir docs/evidence/conformance/oracle_captures/conformance_oracle_collection_real_vba_20260703 -IncludePattern object_collection_add_item.bas,object_collection_count_chain.bas,object_collection_remove_count.bas -TestTimeoutMs 15000`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern object_collection_add_item.bas,object_collection_count_chain.bas,object_collection_remove_count.bas`
- `cargo test -q -p oxvba-host single_source_err_source_defaults_to_excel_project_name`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern err_surface_fields_subset.bas`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern module_const_basic.bas`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern financial_algorithm_npv_irr_mirr_subset.bas,module_const_basic.bas,object_identity_is_nothing.bas,object_identity_is_same_and_different.bas,stdlib_array_introspection_bounds.bas,stdlib_array_introspection_types.bas,stdlib_math_primitives.bas,stdlib_random_financial_expansion.bas,stdlib_rnd_isolated.bas,string_join_array_tag_count.bas`
- `./scripts/run-conformance-oracle.ps1 -OutputDir docs/evidence/conformance/oracle_captures/conformance_oracle_variant_integer_probe2_20260703 -IncludePattern zz_probe_variant_integer_arithmetic2.bas -TestTimeoutMs 15000`
- `./scripts/run-conformance-oracle.ps1 -OutputDir docs/evidence/conformance/oracle_captures/conformance_oracle_variant_integer_unary_probe_20260703 -IncludePattern zz_probe_variant_unary_and_byte.bas -TestTimeoutMs 15000`
- `./scripts/run-conformance-oracle.ps1 -OutputDir docs/evidence/conformance/oracle_captures/conformance_oracle_variant_ladder_full_20260703 -TestTimeoutMs 15000`
- `cargo test -q -p oxvba-runtime arithmetic`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern for_exit_for_basic.bas`
- `cargo test -q -p oxvba-differential --test for_counter_overflow_vm3`
- `./scripts/run-conformance-oracle.ps1 -OutputDir docs/evidence/conformance/oracle_captures/conformance_oracle_foreach_collection_final_probe_20260703 -IncludePattern zz_probe_foreach_collection_final.bas -TestTimeoutMs 15000`
- `cargo test -q -p oxvba-differential --test foreach_scalar_source_vm3`
- `cargo test -q -p oxvba-vm3 foreach`
- `cargo test -q -p oxvba-differential --test project_class_newenum_vm3`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern for_each_array_literal_basic.bas`
- `./scripts/run-conformance-oracle.ps1 -OutputDir docs/evidence/conformance/oracle_captures/conformance_oracle_composite_scalarized_20260703 -IncludePattern array_explicit_lower_bound.bas,array_multidim_indexing.bas,array_option_base_one_bounds.bas,array_store_load.bas,array_zero_index.bas,erase_array_basic.bas,for_each_array_variable_basic.bas,redim_expand_allows_new_index.bas,redim_preserve_keeps_values.bas,redim_preserve_multidim_last_dimension.bas,redim_preserve_shrink_expand_clears_tail.bas,redim_without_preserve_resets.bas,udt_field_access_basic.bas,udt_whole_assignment_copy.bas,udt_whole_assignment_overwrite.bas -TestTimeoutMs 15000`
- `./scripts/run-conformance-oracle.ps1 -OutputDir docs/evidence/conformance/oracle_captures/conformance_oracle_udt_overwrite_scalarized_20260703 -IncludePattern udt_whole_assignment_overwrite.bas -TestTimeoutMs 15000`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language -IncludePattern array_explicit_lower_bound.bas,array_multidim_indexing.bas,array_option_base_one_bounds.bas,array_store_load.bas,array_zero_index.bas,erase_array_basic.bas,for_each_array_variable_basic.bas,redim_expand_allows_new_index.bas,redim_preserve_keeps_values.bas,redim_preserve_multidim_last_dimension.bas,redim_preserve_shrink_expand_clears_tail.bas,redim_without_preserve_resets.bas,udt_field_access_basic.bas,udt_whole_assignment_copy.bas,udt_whole_assignment_overwrite.bas`
- `cargo test -q -p oxvba-differential --lib vm3_golden_snapshot`
- `./scripts/run-conformance.ps1 -Backend vm -Suite basic-language`
  - Current result: ok over 192 files.

## Fresh-Eyes Review

Reviewed the updated golden rows against both raw Excel captures and current
vm3 outputs. Rows were only refreshed to real Excel targets when the generic
oracle capture was a valid observation for that fixture shape. The final
scalarized UDT rows keep private UDT locals out of `Main`, so the generic
oracle harness captures portable scalar field observations instead of failing
on private record variables. The synthetic collection rows were replaced with
real VBA fixtures rather than treated as parity targets.
The `Round(19, -1)` change was backed by the Excel runtime error 5 capture and
by a focused differential regression. The `module_const_basic.bas` parser
failure was traced to `Base` being tokenized as an `Option Base` keyword; VBA
accepts it as an ordinary identifier in `Const BASE = 5` and `x = BASE + 2`,
and vm3 now matches the Excel-backed `i16:7` observable. The `bd-5kqj.3`
compile-diagnostic rows were rechecked with VBE Debug -> Compile and PID-scoped
UI Automation modal capture in
`docs/evidence/conformance/vm3_defobj_return_diagnostics_oracle_20260702/`:
`DefObj A-Z` plus scalar call to an implicit `Object` parameter reports
`Compile error: Type mismatch` at `Call Use(1)`, and `Function alpha%() As
Object` reports `Compile error: Expected: end of statement` with `As` selected.
The `bd-5kqj.2` CVErr/error-state rows now match the Excel captures:
`On Error Resume Next` records the trapped error but does not arm a later
`Resume`, so the explicit `Resume Next` raises error 20 while preserving the
first `Err.Number` capture; out-of-range `CVErr` raises VBA error 5 with the
standard `Invalid procedure call or argument` description and skips target
assignments under `On Error Resume Next`. Fresh-eyes review also found the
older `coercion_cverr_range_predicates.bas` golden was still blessing
out-of-range `CVErr` as an OK value row despite the March 2026 Excel oracle
recording runtime error 5, so that hidden legacy expectation was corrected in
the same CVErr lane. The `bd-5kqj.5` review removed the non-VBA
`CollectionAdd`/`CollectionItem`/`CollectionCount`/`CollectionRemove` fixture
surface from the basic-language target instead of preserving it as
compatibility behavior; the replacement rows use real VBA `Collection`
operations and match the targeted Excel capture exactly. The `bd-5kqj.6`
review found the VM already defaults `Err.Source` from the runtime project name;
the mismatch was the host/CLI single-source wrapper naming the synthetic project
`Main`. That wrapper now uses `VBAProject`, matching Excel's default VBProject
name, while explicit multi-project manifests still keep their declared project
names for cross-project origin behavior. The `bd-5kqj.7` review checked that
the prior broad `Long` promotion was a legacy OxVBA fallback, not a VBA
compatibility target. The targeted Excel probes and the full guarded oracle
capture agree on the small-integer ladder, and golden rows were updated only
where current vm3 output matched those Excel observations; broader array/UDT
residuals were left on their existing beads instead of being hidden by scalar
arithmetic changes. The `bd-5kqj.8` review confirmed
the fix writes `Empty` only on normal enumerator exhaustion; `Exit For` still
uses the loop break path, and collection/project-class/foreign-object
enumeration checks still pass. The final `bd-5kqj.1` review kept structural
array/record
inspection out of the scalar conformance gate instead of blessing OxVBA debug
dumps as VBA behavior. The corrected UDT overwrite fixture uses explicit fields
because Excel rejected the implicit-field form during the guarded oracle run.
The full basic-language gate now passes, so no residual mismatch is left only in
chat.

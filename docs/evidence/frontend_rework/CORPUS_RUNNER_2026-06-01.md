# Corpus Runner Evidence

Date: 2026-06-01
Bead: `bd-aprs.6.4`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `run_frontend_diff_corpus` to `crates/oxvba-compiler/src/frontend_diff.rs`.

The first automated route accepts fixture rows with:

- fixture name and evidence path;
- fixture class: compiler unit, conformance case, host project, or Excel oracle;
- optional inline source;
- bytecode drift expectation, diagnostic/acceptance drift expectation, rationale, and close
  condition for classifier policy.

Rows with inline source and class `CompilerUnit` or `ConformanceCase` run through the existing
legacy-vs-v2 harness and classifier. Host/oracle rows that cannot be bytecode-diffed inside the
compiler crate are now `RouteChecked` when they have a compiler-local HIR production route helper:
the corpus report records HIR route evidence from the shared route-audit helper while still marking
that full VM/host/oracle execution requires a higher-layer runner. This removes unexamined skips
from the seed corpus without claiming runtime/host execution inside the compiler crate.

## Automated Smoke Route

The unit test `frontend_corpus_runner_runs_source_backed_rows_and_skips_residuals` now runs a
reopened seed corpus with real repo fixture sources plus the route-backed v2 improvement fixture:

- `examples/basic/arithmetic.bas`: compiler unit row runs through the harness and classifies as
  equivalent;
- `conformance/tests/call_coercion_mixed_variant_to_long.bas`: conformance row runs through the
  harness and now reaches HIR production with matching bytecode and call descriptors; it classifies
  as `IntentionalImprovement` because HIR source-map metadata preserves the blank line before the
  second procedure while legacy metadata points one line early;
- `inline_statement_separator_bridge_improvement`: compiler unit row runs through the v2 bridge
  route and classifies as `IntentionalImprovement`, because legacy-default rejects the inline
  statement sequence while frontend v2 compiles it with bytecode and metadata;
- selected host/project/imported-COM/predeclared-document seed rows are route-checked as HIR
  production through their compiler-local project route helpers while still requiring VM/host
  execution for full corpus observations;
- source-backed Excel oracle rows are route-checked as HIR production in the compiler corpus while
  still requiring ignored live Excel oracle tests for environment-dependent behavior;
- no seed corpus row remains an unexamined skip; future broader corpus additions may still add
  skipped rows until they have route or execution helpers.

Expected report counts:

- `ran_count = 3`
- `route_checked_count = 12`
- `skipped_count = 0`
- `equivalent_count = 1`
- `intentional_improvement_count = 2`
- `bug_count = 0`

### host/project residual rows

- Class: `HostProject`
- Current status: `RouteChecked`
- Reason: the seed project rows have compiler-local HIR production route evidence, but still
  require a VM/host project runner for full diff/execution observations.
- Seed fixtures: `INTP-001`, `INTP-002`, `INTP-003`, `INTP-004`, `INTP-016`, `INTP-019`,
  inline imported `OxVba.TestDispatch`, inline imported `Scripting.Dictionary`, inline
  predeclared `ThisWorkbook` document reference, and inline predeclared `ThisWorkbook` method
  reference.
- Next route: a crate or script that can depend on the VM/host layer should call the same
  report/classifier shape and attach execution observations.

### excel oracle rows

- Class: `ExcelOracle`
- Current status: `RouteChecked`
- Reason: the compiler corpus can prove HIR production route classification for the source fixtures,
  but still requires targeted Excel oracle fixture execution for live behavior.
- Route-audit status: the source fixture
  `conformance/com/office/excel/excel_application_activation_smoke.bas` and the narrowed
  `conformance/com/office/excel/excel_workbook_range_smoke.bas`, plus the follow-up
  `conformance/com/office/excel/excel_dispatchinvoke_range_smoke.bas` and
  `conformance/com/office/excel/excel_named_argument_smoke.bas` fixtures, and the null-result
  `conformance/com/office/excel/excel_find_null_result_smoke.bas` fixture, now classify as
  `HirProduction`;
  live Excel-visible behavior remains environment-dependent and is checked by ignored
  `oxvba-host` oracle tests.
- Next route: oracle-backed rows need fixture evidence that records the expected Excel-visible
  behavior before they can be classified as harmless drift or intentional improvement.

## Checks

- `cargo test -p oxvba-compiler frontend_corpus --quiet`
- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The runner does not pretend the compiler crate can execute host projects or Excel oracle cases.
  Those classes are included in the diff report as explicit residual skips, not silently omitted;
  the route audit is the separate production-route gate for selected higher-layer source shapes.
- Source-backed compiler and conformance rows already exercise the harness/classifier end to end.
- The corpus test now asserts that `conformance_call_coercion_mixed_variant_to_long` is the
  documented metadata-improvement row; this prevents a green count from hiding a rationale attached
  to the wrong fixture.
- The reopened seed corpus now includes real repository fixture files, not only inline examples,
  and carries the diagnostic-improvement policy from FE-5.3 through the corpus runner.
- This keeps the dependency direction clean: compiler support code remains independent of VM and
  host crates, while leaving a concrete row shape for the later higher-layer runner.

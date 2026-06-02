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
legacy-vs-v2 harness and classifier. Host project and Excel oracle rows are represented in the
same corpus report but are marked `SkippedResidual` with an explicit reason until a higher-layer
runner can execute VM/host/oracle observations without creating a compiler dependency cycle. This
diff-corpus runner is intentionally narrower than the separate route audit: the route audit now
classifies the selected host/project/imported-COM/document rows and the Excel source fixture through
HIR production routes, while this runner still records that it cannot execute those higher-layer
observations itself.

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
- selected host project rows are present but skipped as residuals requiring VM/host execution;
- the Excel oracle row is present but skipped here as a residual requiring oracle-backed execution,
  even though the separate route audit now proves its source fixture classifies as HIR production.

Expected report counts:

- `ran_count = 3`
- `skipped_count = 9`
- `equivalent_count = 1`
- `intentional_improvement_count = 2`
- `bug_count = 0`

### host/project residual rows

- Class: `HostProject`
- Current status: `SkippedResidual`
- Reason: requires VM/host project runner in this diff-corpus harness.
- Seed fixtures: `INTP-001`, `INTP-002`, `INTP-003`, `INTP-004`, `INTP-016`, `INTP-019`,
  inline imported `OxVba.TestDispatch`, and inline predeclared `ThisWorkbook` document reference.
- Next route: a crate or script that can depend on the VM/host layer should call the same
  report/classifier shape and attach execution observations.

### excel-oracle-activation-smoke

- Class: `ExcelOracle`
- Current status: `SkippedResidual`
- Reason: requires targeted Excel oracle fixture execution in this diff-corpus harness.
- Route-audit status: the source fixture
  `conformance/com/office/excel/excel_application_activation_smoke.bas` now classifies as
  `HirProduction`; live Excel-visible behavior remains environment-dependent.
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

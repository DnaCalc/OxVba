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
- bytecode drift expectation, rationale, and close condition for classifier policy.

Rows with inline source and class `CompilerUnit` or `ConformanceCase` run through the existing
legacy-vs-v2 harness and classifier. Host project and Excel oracle rows are represented in the
same corpus report but are marked `SkippedResidual` with an explicit reason until a higher-layer
runner can execute VM/host/oracle observations without creating a compiler dependency cycle.

## Automated Smoke Route

The unit test `frontend_corpus_runner_runs_source_backed_rows_and_skips_residuals` verifies:

- compiler unit row runs through the harness and classifies as equivalent;
- conformance row runs through the harness and classifies as equivalent;
- host project row is present but skipped as a residual requiring VM/host execution;
- Excel oracle row is present but skipped as a residual requiring oracle-backed execution.

### host-project-residual

- Class: `HostProject`
- Current status: `SkippedResidual`
- Reason: requires VM/host project runner.
- Next route: FE-5.4 follow-on integration should call the same report/classifier shape from a
  crate or script that can depend on the VM/host layer.

### excel-oracle-residual

- Class: `ExcelOracle`
- Current status: `SkippedResidual`
- Reason: requires targeted Excel oracle fixture execution.
- Next route: oracle-backed rows need fixture evidence that records the expected Excel-visible
  behavior before they can be classified as harmless drift or intentional improvement.

## Checks

- `cargo test -p oxvba-compiler frontend_corpus --quiet`
- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The runner does not pretend the compiler crate can execute host projects or Excel oracle cases.
  Those classes are included in the report as explicit residual skips, not silently omitted.
- Source-backed compiler and conformance rows already exercise the harness/classifier end to end.
- This keeps the dependency direction clean: compiler support code remains independent of VM and
  host crates, while FE-5.4 leaves a concrete row shape for the later higher-layer runner.

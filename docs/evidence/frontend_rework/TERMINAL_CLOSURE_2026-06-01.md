# Terminal Closure Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.5`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Checks

Passed:

- `cargo test -p oxvba-compiler frontend_ --quiet`
  - 60 passed.
- `cargo test -p oxvba-syntax --quiet`
  - 79 unit tests passed, 2 integration/doc-style tests passed.
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

Not clean:

- `cargo test -p oxvba-compiler --quiet`
  - 923 passed, 1 failed.
  - Failing test:
    `tests::procedure_runtime_metadata_carries_expression_operator_and_coercion_descriptors`
  - Failing assertion expects a `COERCE-CALL-BYVAL-DECLARED-TARGET` coercion descriptor with
    `CallLet`, `Long` source type, and `Double` target type.
  - Direct filtered rerun of that test also fails deterministically.

Not run in this terminal bead:

- full VM crate;
- host crate;
- conformance suite;
- selected Excel oracle checks.

## Residual Ownership

- Full compiler metadata failure: residual owner is the metadata/coercion descriptor lane. The
  failure predates this terminal closure evidence for the frontend workset and is not caused by the
  new frontend modules, which are isolated support surfaces and pass their focused checks.
- VM/host/conformance/oracle broad runs: residual owner is the next execution-harness expansion
  lane. FE-5.4 already records host/oracle rows as explicit residual classes rather than silently
  claiming execution coverage.
- Legacy comparison harness: retained. It should not be archived while lowering remains an explicit
  residual in `frontend_route_policy`.

## Fresh-Eyes Review

- This workset created the planned bead graph and staged implementation surfaces for lexer/parser,
  diffing, HIR, SemanticModel, project semantics, typed lowering contracts, route policy, query
  invalidation, and IDE query sharing.
- The production compiler is not wholly flipped to frontend v2. The clean shape is
  per-construct v2 routing plus explicit residual lowering fallback.
- Because one full compiler test fails and broad VM/host/oracle checks were not run, this terminal
  evidence does not claim whole-repo parity closure. It closes the prepared frontend rework bead set
  with residual ownership documented.

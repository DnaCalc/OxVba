# Per-Construct Route Policy Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.1`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Reopened production-route update: `compile_with_options(..., CompileOptions::default())` now uses
the frontend-v2 route for completed construct families. The route policy also marks scoped lowering
as `V2Default` now that FE-8.5 provides production HIR lowering for the supported assignment and
expression surface.

Continuation update: the ordinary single-source `compile()` / `compile_with_runtime_metadata()`
entry point now tries production HIR lowering first for eligible sources. The old
`resolve::resolve_symbols` route is retained as the explicit `compile_with_runtime_metadata_legacy`
comparison helper for the diff harness and as the tracked fallback for unsupported residual
constructs.

Fallback is retained for tracked residuals when the frontend route reports an unsupported construct,
but completed constructs no longer require an explicit `frontend_v2: true` option and no longer
enter the legacy resolver first through the lightweight compile API.

Eligibility is deliberately narrower than "anything HIR can currently parse". The lightweight
default HIR route excludes DefType statements, functions/properties, optional/default/ParamArray
parameters, class/object-local compatibility contexts, and project-rewritten compilation until
those semantics are represented by HIR facts with route proof. Those sources continue through the
legacy residual path rather than accepting partial HIR output.

## Checks

- `cargo test -p oxvba-compiler frontend_route_policy --quiet`
- `cargo test -p oxvba-compiler compile_options_default --quiet`
- `cargo test -p oxvba-compiler compile_options_frontend_v2 --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_uses_hir_for_completed_constructs --quiet`
- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- This remains construct-scoped, not a false whole-compiler flip. The route policy leaves project
  semantics as the named residual, while the default compile-with-options path tries frontend-v2 and
  only falls back for explicitly unsupported residual constructs. The same is now true for the
  lightweight runtime-metadata compile path when no object-local/class-module compatibility context
  is supplied.
- The first attempt made the HIR default too broad and let partial HIR output bypass legacy DefType,
  optional-argument, function-return, and project-rewrite semantics. The eligibility guard now keeps
  those surfaces residual until their beads implement the missing HIR facts.
- Strict `frontend_v2: true` remains useful for tests that want diagnostics from the frontend-v2
  analyzer before fallback.
- Legacy-vs-v2 differential tests must call the explicit legacy helper; otherwise the baseline would
  follow the new default route and mask migration defects.

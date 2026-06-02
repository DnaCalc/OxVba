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

Eligibility is deliberately narrower than "anything HIR can currently parse", but the original
exclusion list has been narrowed by later FE-8.5 delivery. The lightweight default HIR route now
admits known DefType directives, `Option Compare Binary`/`Text`/`Database` (with the current
Database-as-binary runtime approximation), single-source `Option Private Module`, simple function
returns, simple property declaration/read/write shapes, explicit/defaultless/integer-expression
optional parameters, simple `ParamArray` packing, selected typed constants, and the documented
Declare subsets once their HIR facts and route proof landed. Follow-up FE-8.5.e work also admits
`Option Explicit` to the lightweight default route once HIR preserved the option flag and route proof
covered an otherwise completed source. Class/object-local compatibility contexts, project-rewritten
compilation, unsupported project property/default-member/COM rewrite shapes, and other explicitly
tracked residual constructs still fall back rather than accepting partial HIR output.

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
  optional-argument, function-return, and project-rewrite semantics. Later slices moved the
  completed DefType, function-return, optional/default, ParamArray, property, option, constant, and
  Declare subsets out of that residual with route proof. The guard still keeps incomplete project
  and compatibility-context surfaces residual until their beads implement the missing HIR facts.
- Strict `frontend_v2: true` remains useful for tests that want diagnostics from the frontend-v2
  analyzer before fallback.
- Legacy-vs-v2 differential tests must call the explicit legacy helper; otherwise the baseline would
  follow the new default route and mask migration defects.

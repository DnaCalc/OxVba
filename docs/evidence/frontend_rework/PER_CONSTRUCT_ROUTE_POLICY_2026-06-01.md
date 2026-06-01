# Per-Construct Route Policy Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.1`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Reopened production-route update: `compile_with_options(..., CompileOptions::default())` now uses
the frontend-v2 route for completed construct families. The route policy also marks scoped lowering
as `V2Default` now that FE-8.5 provides production HIR lowering for the supported assignment and
expression surface.

The legacy compiler entry point `compile()` remains available as the comparison/baseline route.
Fallback is retained for tracked residuals when the frontend-v2 route reports an unsupported
construct, but completed constructs no longer require an explicit `frontend_v2: true` option.

## Checks

- `cargo test -p oxvba-compiler frontend_route_policy --quiet`
- `cargo test -p oxvba-compiler compile_options_default --quiet`
- `cargo test -p oxvba-compiler compile_options_frontend_v2 --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- This remains construct-scoped, not a false whole-compiler flip. The route policy leaves project
  semantics as the named residual, while the default compile-with-options path tries frontend-v2 and
  only falls back for explicitly unsupported residual constructs.
- Strict `frontend_v2: true` remains useful for tests that want diagnostics from the frontend-v2
  analyzer before fallback.

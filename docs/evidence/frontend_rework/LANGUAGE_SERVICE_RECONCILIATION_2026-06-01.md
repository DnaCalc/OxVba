# Language-Service Reconciliation Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.4`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_language_service.rs`, a thin IDE query bridge that
answers symbol, type, and diagnostics from shared `SemanticModel` facts.

## Checks

- `cargo test -p oxvba-compiler frontend_language_service --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The bridge does not duplicate semantic logic. It delegates to `SemanticModel` symbol/type and
  diagnostic queries.
- Full `oxvba-languageservice` replacement remains staged, but the shared compiler API now has a
  test-backed query shape for IDE consumers.

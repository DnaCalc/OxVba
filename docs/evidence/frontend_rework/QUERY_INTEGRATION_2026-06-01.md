# Query Integration Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.3`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_query.rs`, an incremental query-layer facade for parse,
bind, typecheck, diagnostics, and SemanticModel invalidation.

## Checks

- `cargo test -p oxvba-compiler frontend_query --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- This is salsa-shaped but does not add the salsa dependency yet. It records the invalidation
  contract so later salsa adoption has a small, test-backed target.
- Parse invalidation recomputes all downstream layers; typecheck invalidation leaves parse/bind
  revisions untouched.

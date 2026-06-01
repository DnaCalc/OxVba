# Legacy Retirement Inventory Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.2`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_retirement_inventory.rs`, an explicit retirement and
quarantine inventory for legacy parser/rewriter paths.

Rows currently mark structural intrinsic magic names as replaced, while legacy expression parsing
and broad `project.rs` rewrites remain quarantined residuals with owners and replacement surfaces.

## Checks

- `cargo test -p oxvba-compiler frontend_retirement_inventory --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- This avoids claiming deletion before the production lowering path consumes all replacement
  surfaces.
- Every residual row has an owner and replacement path, so legacy fallback is not silent.

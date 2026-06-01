# Legacy Retirement Inventory Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.2`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Updated `crates/oxvba-compiler/src/frontend_retirement_inventory.rs`, the explicit retirement and
quarantine inventory for legacy parser/rewriter paths, to match the reopened production migration
state.

Rows now distinguish:

- replaced scoped production behavior: `resolve::parse_expr_for_syntax_bridge` is superseded for
  the completed assignment/expression production surface by
  `frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir`;
- quarantined compatibility fallback: `syntax_bridge` still falls back to the existing
  resolver/lowering path when HIR production lowering reports `Unsupported`;
- quarantined broad resolver parsing: `resolve::parse_expr` still exists inside the legacy
  production resolver and remains a terminal-audit residual, even though scoped HIR fixtures bypass
  it;
- quarantined project/class/COM/default-member rewrites: `project.rs` remains load-bearing for
  broad project semantics despite partial FE-7 classifier/index work;
- quarantined CST-to-legacy expression bridge: `syntax_bridge::lower_cst_expr` remains a
  compatibility/test bridge until the terminal route audit proves it is outside the claimed
  production surface or deletes it;
- replaced structural intrinsic names where FE-8.1 moved compiler-owned structural concepts to
  `frontend_structural_intrinsics::StructuralIntrinsic`.

Each row carries the partial work already done and the concrete closure condition needed before it
can be treated as retired rather than merely inventoried.

Executable route proof was added through a test-only route classifier in `syntax_bridge`: a scoped
assignment/arithmetic fixture classifies as `HirProduction`, while an unsupported `Call` statement
fixture classifies as `CstLegacyFallback`. This prevents FE-9.2 from silently treating fallback as
retirement.

## Checks

- `cargo test -p oxvba-compiler frontend_retirement_inventory --quiet`
- `cargo test -p oxvba-compiler syntax_bridge::tests::bridge_compiles_supported_statement_sequence_after_cst_validation --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The previous evidence was stale after FE-8.5: it still described `syntax_bridge::lower_cst_expr`
  as the replacement route. That is now corrected to HIR production lowering for the scoped
  surface, with the CST bridge classified as residual.
- This bead does not claim broad deletion of `parse_expr` or `project.rs` rewrites. Those remain
  load-bearing residuals until FE-9.6 proves they are outside the scoped surface or later beads
  remove/quarantine them construct by construct.
- Every residual row has an owner, replacement surface, partial-work note, and closure condition,
  so legacy fallback is not silent.

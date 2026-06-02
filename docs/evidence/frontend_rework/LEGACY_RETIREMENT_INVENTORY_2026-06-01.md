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
- quarantined compatibility fallback: default `compile_with_options` now tries HIR production
  lowering directly, then falls back to the existing resolver/lowering path when HIR production
  lowering reports `Unsupported`; explicit `frontend_v2` mode reports HIR unsupported as a
  front-end error instead of falling back;
- quarantined broad resolver parsing: `resolve::parse_expr` still exists inside the legacy
  production resolver and remains a terminal-audit residual, even though scoped HIR fixtures bypass
  it;
- quarantined bundle context fallback: bundle module context fact extraction now tries HIR
  `BoundModule` construction first, then falls back to `resolve_symbols` only for unsupported
  residual modules; the internal route decision is now executable-test visible so HIR-derived
  bundle facts and fallback-derived bundle facts cannot be conflated in terminal evidence;
- quarantined project/class/COM/default-member rewrites: `project.rs` production compilation now
  selects the module-aware plan unconditionally; the old rewrite bridge remains for internal parity
  tests while FE-7.1 through FE-7.6 own broader replacement of source-text lowering internals;
- replaced/test-only CST-to-legacy expression bridge: `syntax_bridge::lower_cst_expr` and the
  source bridge helpers are compiled only for internal bridge tests; `syntax_bridge` is
  crate-private, current code search finds no production caller, and production route
  classification validates CST before calling HIR lowering directly;
- replaced structural intrinsic names where FE-8.1 moved compiler-owned structural concepts to
  `frontend_structural_intrinsics::StructuralIntrinsic`.

Each row carries the partial work already done and the concrete closure condition needed before any
remaining residual can be treated as retired rather than merely inventoried.

Executable route proof was added through a test-only route classifier in `syntax_bridge`: scoped
assignment/arithmetic and simple same-module `Call` statement fixtures classify as
`HirProduction`. This prevents FE-9.2 from silently treating fallback as retirement.

Bundle module fact extraction now has a separate internal route classifier:
`bundle_fact_bound_module_route_uses_hir_for_supported_modules` proves a completed lightweight
module produces facts from the HIR `BoundModule` path, while
`bundle_fact_bound_module_route_marks_legacy_residual_fallback` proves an unsupported external
declaration shape is still classified as a legacy-resolver residual instead of being mistaken for
retirement.

The `syntax_bridge` module is now `pub(crate)`, and its CST-to-legacy expression/source bridge
helpers are behind `#[cfg(test)]`. They remain available to internal compatibility tests, but they
are no longer public compiler API and are not compiled into ordinary production builds. Current
repository search finds no production caller of `lower_expression_to_legacy_bound_expr`,
`compile_source_via_syntax_bridge`, or
`compile_source_with_runtime_metadata_via_syntax_bridge`; the only non-test internal use of the
module is the HIR route classifier consumed by the legacy route audit. The retirement inventory now
records this CST bridge path as `Replaced`, not a quarantined production residual.

## Checks

- `cargo test -p oxvba-compiler frontend_retirement_inventory --quiet`
- `cargo test -p oxvba-compiler bundle_fact_bound_module_route --quiet`
- `cargo test -p oxvba-compiler syntax_bridge --quiet`
- `cargo test -p oxvba-compiler frontend_legacy_route_audit --quiet`
- `cargo test -p oxvba-compiler syntax_bridge::tests::bridge_compiles_supported_statement_sequence_after_cst_validation --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The previous evidence was stale after FE-8.5: it still described `syntax_bridge::lower_cst_expr`
  as the replacement route. That is now corrected to HIR production lowering for the scoped
  surface, with the CST bridge classified as a replaced, test-only compatibility helper rather than
  a production residual. A later FE-9 route cleanup moved the public `compile_with_options` HIR
  attempt off `syntax_bridge` entirely; only the explicit default fallback policy reaches legacy
  compilation after `Unsupported`. A subsequent FE-9 package-context cleanup moved bundle module
  fact extraction to prefer HIR `BoundModule` facts, with `resolve_symbols` retained as an explicit
  unsupported-module fallback. This continuation adds route-visible bundle fact tests so that
  fallback-derived package context facts cannot satisfy a terminal HIR ownership claim.
- This bead does not claim broad deletion of `parse_expr` or `project.rs` rewrite-era internals.
  The project rewrite bridge is no longer production-selected, and the CST-to-legacy syntax bridge
  helpers are test-only/replaced for production purposes, but source-text lowering internals remain
  compatibility scaffolding until FE-7/FE-9 retirement beads finish replacing or quarantining them.
- Every residual row has an owner, replacement surface, partial-work note, and closure condition,
  so legacy fallback is not silent.

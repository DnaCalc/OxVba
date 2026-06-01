# VBA Grammar Coverage Matrix Summary

Date: 2026-06-01
Bead: `bd-aprs.2.3`
Matrix: `docs/evidence/frontend_rework/VBA_GRAMMAR_COVERAGE_MATRIX_2026-06-01.csv`
Grammar scaffold: `docs/spec/VBA_GRAMMAR_V1.md`

## Summary

The matrix was generated from every EBNF production in `VBA_GRAMMAR_V1.md`.

- Total production rows: 110.
- Rows with existing fixture anchors: 44.
- Rows needing explicit fixture anchors: 66.

The matrix columns are now split into two groups.

Original coverage-anchor columns:

- `production`
- `category`
- `fixture_anchor`
- `parser_status`
- `binder_status`
- `execution_status`
- `residual_disposition`
- `notes`

Production-route gate columns added after the workset reopen:

- `cst_parser_route`
- `production_binder_route`
- `production_lowering_route`
- `legacy_route_status`
- `route_evidence`

These route columns deliberately do **not** mark the current compiler as migrated. They make the
missing proof explicit:

- Rows needing CST route proof from existing anchors: 44.
- Rows needing HIR binder route proof from existing anchors: 39.
- Rows needing HIR lowering route proof from existing anchors: 44.
- Rows still classified as anchored by a production legacy route that is not retired: 44.

`route_evidence=none_yet` is intentional for this bead. Later FE-4 through FE-10 beads must replace
those values with executable route evidence as they migrate production behavior.

## Fresh-Eyes Notes

This matrix is intentionally not a parity claim. Many rows are scaffolded and marked as needing
fixtures, because the current conformance suite is execution-oriented and does not yet prove
syntax recovery, round-trip behavior, or binder diagnostics for every grammar production.

Fresh-eyes review after reopening: the original matrix was too easy to misuse as closure evidence
because `anchored_existing` could mean "legacy compiler has an execution/conformance fixture." It
now records that such rows still need production CST/HIR/lowering route proof and that legacy
production routes are not yet retired.

## Checks

- Matrix row count matches the 110 EBNF productions in `docs/spec/VBA_GRAMMAR_V1.md`.
- All non-empty fixture anchors in the matrix resolve to existing repository paths.
- Route-gate columns are present for every row.
- Current route-proof counts were verified with `Import-Csv`:
  - 44 `legacy_anchor_needs_cst_route_proof`;
  - 39 `legacy_anchor_needs_hir_route_proof`;
  - 44 `legacy_anchor_needs_hir_lowering_route_proof`;
  - 44 `production_legacy_route_not_yet_retired`.
- `git diff --check`: passed with line-ending warnings only for touched tracked files.

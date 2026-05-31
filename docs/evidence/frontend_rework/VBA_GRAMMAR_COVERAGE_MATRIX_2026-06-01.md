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

The matrix columns are:

- `production`
- `category`
- `fixture_anchor`
- `parser_status`
- `binder_status`
- `execution_status`
- `residual_disposition`
- `notes`

## Fresh-Eyes Notes

This matrix is intentionally not a parity claim. Many rows are scaffolded and marked as needing
fixtures, because the current conformance suite is execution-oriented and does not yet prove
syntax recovery, round-trip behavior, or binder diagnostics for every grammar production.

## Checks

- Matrix row count matches the 110 EBNF productions in `docs/spec/VBA_GRAMMAR_V1.md`.
- All non-empty fixture anchors in the matrix resolve to existing repository paths.
- `git diff --check`: passed with line-ending warnings only for touched tracked files.

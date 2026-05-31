# Parser Error Recovery Shape

Date: 2026-06-01
Bead: `bd-aprs.3.4`
Crate: `crates/oxvba-syntax`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

The parser now has an explicit missing-expression recovery shape for assignment RHS positions:

- source text remains lossless;
- a parse diagnostic is emitted;
- a zero-width `SyntaxKind::ErrorNode` is inserted at the missing expression position;
- parsing continues to the rest of the procedure.

## Implemented Path

- Added `parse_required_expr(message)` in `crates/oxvba-syntax/src/parser.rs`.
- Routed `Set`, `Let`, and implicit assignment RHS parsing through that helper.
- Added tests:
  - `incomplete_assignment_reports_error_node_without_losing_text`
  - `incomplete_set_assignment_reports_error_node_without_losing_text`
- Strengthened the existing unexpected-statement recovery test to require an explicit `ErrorNode`.

## Recovery Contract

For incomplete edit states in expression-required positions:

- do not drop source text;
- do not consume unrelated following line text as the missing expression;
- emit one stable diagnostic for the missing expression;
- insert an `ErrorNode` so IDE consumers can keep a structural placeholder;
- let later parser recovery beads add more contexts rather than baking special cases into callers.

## Remaining Gaps

- Missing callee/target diagnostics are not standardized yet.
- Incomplete block recovery needs targeted `If`/`For`/`Select`/`With` fixtures.
- Diagnostics still use strings, not stable diagnostic IDs.
- Error-node spans are currently represented by the red node range; zero-width placeholder semantics
  should be made explicit in the future diagnostic API.

## Checks

- `cargo test -p oxvba-syntax --quiet`: passed, 62 tests.
- `git diff --check`: passed with line-ending warnings only for touched tracked files.

## Fresh-Eyes Notes

The important fix is not just adding a diagnostic; it is keeping the CST usable for IDE scenarios.
The missing RHS now has a structural placeholder without losing text or blocking later lines.

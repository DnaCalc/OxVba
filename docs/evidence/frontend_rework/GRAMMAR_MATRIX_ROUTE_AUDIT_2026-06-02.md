# Grammar Matrix Route Audit Evidence

Date: 2026-06-02
Bead: `bd-aprs.10.7`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_grammar_matrix_route_audit.rs`, an executable bridge
between the VBA grammar coverage matrix and the production legacy-route audit.

The new audit maps 106 of the 110 grammar-matrix productions to HIR-production findings from
`run_production_legacy_route_audit()`. It includes all 44 currently anchored matrix productions and
scaffold subproductions that are demonstrably present inside those executable route fixtures. It
does not claim the full 110-row grammar matrix is closed; it prevents already-audited route proof
from remaining invisible behind stale `none_yet` matrix cells.

Covered categories include:

- top-level source, option, and attribute rows;
- declaration rows for constants, variable declarations, enums, UDTs, `Declare`, events, and
  `Implements`;
- procedure rows for `Sub`, `Function`, `Property Get/Let/Set`, and parameters;
- statement rows for assignment, calls, `If`, `Select Case`, loops, `With`, error control, labels,
  `GoSub`, `Return`, `Exit`, `Erase`, `ReDim`, and `RaiseEvent`;
- expression/lexical rows for comparison, concatenation, arithmetic, unary `Not`,
  postfix/member access, `TypeOf Is`, argument lists, named arguments, builtin types, and literals.

## Checks

- `cargo test -p oxvba-compiler frontend_grammar_matrix_route_audit --quiet`
- `cargo fmt --check`
- `git diff --check`

## Fresh-Eyes Review

- This is deliberately an executable route audit, not a manual CSV closure edit. The CSV remains the
  inventory of all 110 grammar productions; this audit is the route-proof overlay for rows already
  represented by current HIR-production fixtures.
- The audit fails if any mapped route finding disappears, falls back, or is renamed without updating
  the matrix mapping. That makes FE-9.7 less dependent on prose-only evidence.
- Remaining matrix work is still substantial: the four unmapped grammar productions
  (`static_decl`, `empty_stmt`, `exponent_expr`, and `qualified_identifier`) need fixtures or
  explicit out-of-scope/not-applicable classification, and the 106-row overlay still needs to be
  expanded into row-specific CST/binder/lowering evidence in the CSV or an equivalent generated
  matrix report before full matrix closure.

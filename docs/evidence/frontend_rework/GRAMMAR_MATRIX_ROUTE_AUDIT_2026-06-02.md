# Grammar Matrix Route Audit Evidence

Date: 2026-06-02
Bead: `bd-aprs.10.7`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_grammar_matrix_route_audit.rs`, an executable bridge
between the VBA grammar coverage matrix and the production legacy-route audit.

The new audit maps 40 anchored matrix productions to HIR-production findings from
`run_production_legacy_route_audit()`. It does not claim the full 110-row grammar matrix is closed;
it prevents already-audited route proof from remaining invisible behind stale `none_yet` matrix
cells.

Covered categories include:

- top-level source, option, and attribute rows;
- declaration rows for constants, enums, UDTs, and `Declare`;
- procedure rows for `Sub`, `Function`, `Property Get/Let/Set`, and parameters;
- statement rows for assignment, calls, `If`, `Select Case`, loops, `With`, error control, labels,
  `GoSub`, `Exit`, `Erase`, `ReDim`, and `RaiseEvent`;
- expression/lexical rows for comparison, arithmetic, postfix/member access, `TypeOf Is`, argument
  lists, named arguments, and literals.

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
- Remaining matrix work is still substantial: scaffold-only rows need fixtures, and anchored rows
  outside this 40-row overlay still need explicit CST/HIR/lowering route proof or owning bead
  reopen decisions.

# SemanticModel Query API Evidence

Date: 2026-06-01
Bead: `bd-aprs.7.3`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_semantic_model.rs`, a first IDE-facing query layer over
compiler-owned symbol and HIR facts.

The API provides:

- `SemanticNodeKey` keyed by CST syntax kind plus byte span;
- expression and statement node-to-HIR mappings;
- symbol queries for CST nodes and HIR expressions;
- type queries for CST nodes and HIR expressions;
- diagnostic storage and span-overlap filtering;
- read access to the underlying `SymbolModel` and `HirArenas`.

The query layer does not store semantic data on CST nodes. It maps from CST identity to HIR IDs and
answers from HIR/symbol/type facts.

Reopened update: `SemanticModel::from_bound_hir_module` now indexes the CST-fed HIR produced by
FE-6.2. It walks procedure bodies, maps statement/expression backpointers into query keys, records
name-expression symbols from HIR facts, and exposes byte-span query helpers:

- `expr_for_span`
- `stmt_for_span`
- `symbol_for_span`

This gives the language-service-style API an executable route from source text through
`oxvba-syntax` -> FE-6.1 symbols -> FE-6.2 HIR -> SemanticModel queries for the scoped subset.

## Checks

- `cargo test -p oxvba-compiler frontend_semantic_model --quiet`
- `cargo test -p oxvba-compiler frontend_hir --quiet`
- `cargo test -p oxvba-compiler frontend_symbols --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The API is intentionally small but shaped like the required Roslyn-style split: syntax identity
  comes in, compiler facts come out.
- Name expressions can answer symbol queries directly from `HirExprKind::Name`; non-name
  expressions use explicit HIR fact records. That avoids duplicating binding logic in the query
  layer.
- The reopened tests query a parameter symbol and an assignment statement by byte span from a
  source-built model, proving the API is no longer only manually populated.
- Diagnostics are currently simple code/message/span facts. FE-6.5 can add compatibility mapping
  without changing the basic query shape.
- `SemanticNodeKey` uses syntax kind plus byte span, which is stable enough for batch and thin IDE
  queries but not a final incremental identity. FE-9.3/salsa work should replace or augment it with
  snapshot-aware keys.
- Type facts still need FE-6.4 to populate them from real declared/coercion information; this bead
  preserves the query shape and source/HIR route.

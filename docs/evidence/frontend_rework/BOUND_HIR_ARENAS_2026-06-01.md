# Bound HIR Arenas Evidence

Date: 2026-06-01
Bead: `bd-aprs.7.2`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_hir.rs`, a first compiler-owned HIR arena model built
on the FE-6.1 symbol identity layer.

The arena defines typed IDs and node storage for:

- expressions;
- statements;
- declarations;
- calls;
- members;
- properties;
- type nodes.

Each node carries a `CstBackpointer` with syntax kind text and `FrontendSourceSpan`. This keeps
HIR independent of syntax-tree lifetimes while preserving enough source identity for the upcoming
SemanticModel and diagnostic mapping beads.

Reopened update: added `build_hir_from_source(module_name, source)`, a CST-fed HIR builder for the
initial scoped subset. It parses with `oxvba-syntax`, reuses the FE-6.1 symbol collector, and
allocates a `BoundHirModule` containing the symbol model, arenas, and root declaration IDs.

## Represented Constructs

Focused tests prove the arenas can represent selected parser corpus shapes:

- `x = 1` assignment as `HirStmtKind::Let` with name and literal expressions;
- CST-fed procedure declarations with parameter symbols, local declaration symbols, and statement
  body IDs;
- CST-fed simple assignment lowering from `AssignStmt`/`LetStmt`/`SetStmt` into HIR statements;
- CST-fed identifier, literal, parenthesized, and simple binary expressions with symbol-backed
  name references;
- member expression plus call and property nodes;
- object type node linked to a type symbol;
- procedure declaration containing a statement body.

## Checks

- `cargo test -p oxvba-compiler frontend_hir --quiet`
- `cargo test -p oxvba-compiler frontend_symbols --quiet`
- `cargo test -p oxvba-compiler frontend_semantic_model --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The arena is no longer hand-allocation-only: the reopened builder lowers an intentionally small
  parser/symbol subset into HIR from source. This gives FE-6.3 a real compiler-owned fact surface
  to query.
- Backpointers are value objects rather than borrowed syntax nodes, avoiding red-tree lifetime
  coupling in compiler-owned HIR.
- The HIR refers to `SymbolId` and `HirTypeId` rather than string names, matching the Roslyn-style
  split between syntax and semantics.
- The builder remains deliberately limited: it does not yet bind full VBA statement coverage,
  calls/postfix/member/default-member semantics, object identity, or production bytecode lowering.
  Those are owned by later FE-6/FE-7/FE-8 beads.

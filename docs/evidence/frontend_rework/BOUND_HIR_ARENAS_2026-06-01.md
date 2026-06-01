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

## Represented Constructs

Focused tests prove the arenas can represent selected parser corpus shapes:

- `x = 1` assignment as `HirStmtKind::Let` with name and literal expressions;
- member expression plus call and property nodes;
- object type node linked to a type symbol;
- procedure declaration containing a statement body.

## Checks

- `cargo test -p oxvba-compiler frontend_hir --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The arena is structural only. It does not yet lower parser CST into HIR automatically; that is
  later binder work. This bead establishes the typed storage and ID shape that later lowering can
  target.
- Backpointers are value objects rather than borrowed syntax nodes, avoiding red-tree lifetime
  coupling in compiler-owned HIR.
- The HIR refers to `SymbolId` and `HirTypeId` rather than string names, matching the Roslyn-style
  split between syntax and semantics.
- The node set is intentionally broad enough for FE-6.3/FE-6.4 without pretending every VBA
  construct has a final HIR variant yet.

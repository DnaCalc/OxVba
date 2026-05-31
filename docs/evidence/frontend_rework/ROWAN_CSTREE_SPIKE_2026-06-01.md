# Rowan/cstree Library Spike

Date: 2026-06-01
Bead: `bd-aprs.3.2`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Decision

Keep the current custom `oxvba-syntax` green/red tree as the frontend v2 substrate for now.

Do not migrate to `rowan` or `cstree` in Phase 0. Reconsider only if a later implementation bead
hits a concrete blocker in node identity, traversal performance, interning, memory behavior,
threading, or typed facade ergonomics that is cheaper to solve by migration than by hardening the
existing tree.

## Sources Checked

- Rust-analyzer syntax architecture notes:
  `https://rust-analyzer.github.io/book/contributing/syntax.html`
- `rowan` crate documentation:
  `https://docs.rs/crate/rowan/latest`
- `cstree` crate documentation:
  `https://docs.rs/cstree/latest/cstree/`
- Current repo audit:
  `docs/evidence/frontend_rework/GREEN_RED_TREE_AUDIT_2026-06-01.md`

## Comparison

| Criterion | Current custom tree | `rowan` | `cstree` | Decision impact |
|---|---|---|---|---|
| Lossless CST | Verified by existing and new syntax tests | Designed for lossless syntax trees | Designed for lossless syntax trees | No migration need |
| Red cursor offsets | Present via `SyntaxNode` offset/text range | Mature API with red nodes and text ranges | Similar green/red API | Current API is sufficient for next beads |
| Typed facade support | Existing ad hoc typed accessors | Common pattern in rust-analyzer-style crates | Supports typed facade pattern | FE-2.3 can harden current facade first |
| Node identity | Snapshot-local offset/root cursor only | More mature node/token API | More mature node/token API plus cache options | Future SemanticModel identity still must be project-defined |
| Interning/deduplication | None beyond `Arc<GreenNode>` sharing | Mature green tree sharing | Built-in cache/interner-oriented design | Not yet proven hot enough to justify migration |
| Threading | Current runtime/frontend assumption is single-threaded for this project phase | Library maturity helps broader tooling | Library maturity helps broader tooling | Not a current blocker |
| Maintenance cost | Small local code, already understood | New dependency and adapter rewrite | New dependency and adapter rewrite | Migration churn would delay parser/binder work |
| Error recovery | Parser-owned; custom tree can hold `ErrorNode` | Parser-owned; rowan stores nodes/tokens | Parser-owned; cstree stores nodes/tokens | Library choice does not solve parser recovery |

## Migration Triggers

A migration can be reopened if one of these becomes true:

- current red traversal allocation becomes a measured language-service bottleneck;
- SemanticModel node keys cannot be made stable enough with current root+range/node-kind strategy;
- memory use from token text storage becomes unacceptable on the fixture corpus;
- typed facade ergonomics require large local infrastructure that rowan/cstree would avoid;
- cstree's cache/interner model proves materially useful for multi-module/project parsing.

## Fresh-Eyes Notes

The major misconception would be believing a helper library supplies the Roslyn-style frontend.
It does not. The hard work remains lexer/parser coverage, typed facades, binder/HIR,
SemanticModel, and diagnostics. Since the current tree passes the FE-2.1 audit, migration now would
be churn without a proven payoff.

## Checks

- `cargo test -p oxvba-syntax --quiet`: previously passed in `bd-aprs.3.1` with 60 tests after
  audit coverage was added.
- `git diff --check`: passed with line-ending warnings only for touched tracked files.

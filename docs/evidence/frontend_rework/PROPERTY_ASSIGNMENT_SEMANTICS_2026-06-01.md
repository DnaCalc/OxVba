# Property and Assignment Semantics Evidence

Date: 2026-06-01
Bead: `bd-aprs.8.3`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_assignment_semantics.rs`, a typed property and
assignment semantics surface over HIR IDs.

The model records:

- Property Get/Let/Set accessor routes;
- default-member read/write/invoke routes;
- Let vs Set assignment intent;
- corresponding coercion descriptor kind;
- object/scalar assignment diagnostics.

## Checks

- `cargo test -p oxvba-compiler frontend_assignment_semantics --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The semantics are represented as HIR/symbol facts, not source rewrites.
- Object/scalar diagnostics are deliberately conservative: invalid `Set` and object-target `Let`
  cases produce stable binder diagnostic codes.
- This bead provides the property matrix contract; production emitter migration remains a later
  FE-7/FE-8 step.

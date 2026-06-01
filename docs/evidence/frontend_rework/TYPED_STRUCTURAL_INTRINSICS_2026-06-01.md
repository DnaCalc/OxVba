# Typed Structural Intrinsics Evidence

Date: 2026-06-01
Bead: `bd-aprs.9.1`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_structural_intrinsics.rs`, a typed enum for structural
compiler intrinsics that currently appear as magic legacy names.

Covered families: Nothing, Null, omitted args, project instances, WithEvents attach/detach,
dynamic dispatch get/let/set/invoke, and pointer helpers.

## Checks

- `cargo test -p oxvba-compiler frontend_structural_intrinsics --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The enum has an explicit legacy-name bridge for staged migration, but unknown magic strings do
  not classify as typed intrinsics.
- This bead does not delete legacy string use yet; it creates the exhaustive typed target for the
  lowering cleanup beads.

# Events and Implements Semantics Evidence

Date: 2026-06-01
Bead: `bd-aprs.8.5`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_event_semantics.rs`, a typed routing surface for
WithEvents, RaiseEvent, event handler matching, Implements, and related diagnostics.

## Checks

- `cargo test -p oxvba-compiler frontend_event_semantics --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- Routes are symbol-based and do not depend on generated source text.
- Stable diagnostics are present for missing event handlers and missing Implements members.
- This bead provides binder/HIR route facts; production deletion of legacy event rewriting remains
  a later migration step once the full lowering path consumes these routes.

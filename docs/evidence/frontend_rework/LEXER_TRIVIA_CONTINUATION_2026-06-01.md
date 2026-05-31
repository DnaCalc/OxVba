# Lexer Trivia And Continuation Semantics

Date: 2026-06-01
Bead: `bd-aprs.4.1`
Crate: `crates/oxvba-syntax`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

The lexer now has focused coverage for whitespace, apostrophe comments, `Rem` comments, physical
newline preservation, logical-statement separators, and line-continuation edge cases.

## Code Changes

- Added `Rem` comment recognition at physical/logical statement start:
  - start of file,
  - after a newline,
  - after a colon statement separator,
  - allowing only leading spaces/tabs before `Rem`.
- Kept non-comment identifiers such as `Remember` as identifiers.
- Tightened `_` line continuation recognition so `_` at EOF is not accepted as a continuation; a
  physical newline is required.
- Added tests:
  - `rem_comment_trivia_at_logical_statement_start`
  - `line_continuation_requires_physical_newline`
  - `trivia_snapshot_preserves_physical_and_logical_lines`

## Fresh-Eyes Notes

The existing lexer comment claimed `' or Rem at line start`, but only apostrophe comments were
implemented. That was a real drift between code and intended behavior. The other issue was `_` at
EOF being treated as a continuation even though line continuation requires a physical following
line. Both are now covered by tests.

Remaining lexer work is still owned by later FE-3 beads:

- full literal lexing expansion;
- identifier/keyword edge cases;
- broader snapshot corpus.

## Checks

- `cargo test -p oxvba-syntax --quiet`: passed, 65 tests.
- `cargo fmt --check -p oxvba-syntax`: passed.
- `git diff --check`: passed with line-ending warnings only for touched tracked files.

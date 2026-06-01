# Lexer Identifier and Keyword Completion Evidence

Date: 2026-06-01
Bead: `bd-aprs.4.3`
Workset lane: FE-3.3 Identifier and keyword lexing

## Outcome

`oxvba-syntax` now has targeted coverage for the identifier/keyword lexer shape needed by the
front-end rework:

- bracketed identifiers remain one lossless `BracketedIdent` token and preserve host/library names
  that collide with keywords, including spaces inside brackets;
- identifier and keyword spelling is case-preserving in token text while keyword lookup remains
  case-insensitive;
- attached VBA type suffixes `%`, `&`, `!`, `#`, `@`, and `$` are emitted after identifier-like
  word tokens, including keyword-colliding names such as `Name$`;
- keyword-colliding member/declaration names are represented without losing suffix or original
  spelling, leaving context decisions to the parser/binder.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-syntax --quiet`
  - Result: passed, 70 tests.
- `cargo fmt --check -p oxvba-syntax`
  - Result: passed after formatting.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The main bug risk was the prior `Ident`-only suffix emission rule. It preserved `x$`, but a
keyword-colliding declaration name such as `Function Name$()` tokenized `Name` as `KwName` and
then treated `$` as an error/punctuation path rather than as `TypeSuffix`. That lost syntax
structure before the parser had any chance to decide whether `Name` was a statement keyword or an
identifier-like name. The lexer now emits suffixes for all identifier-like word tokens.

This bead deliberately does not make keywords contextual in the lexer. The clean Roslyn-style shape
is to keep keyword token identity and let the parser/binder decide when a keyword token is usable as
a name. Full statement-start ambiguity, such as a bare host member colliding with a reserved
statement keyword, remains parser/front-end work rather than lexer work.

Residuals left for later beads:

- broader token snapshot corpus coverage belongs to FE-3.4;
- parser disambiguation for keyword-colliding statement starts belongs to FE-4;
- binder/name-resolution policy for host/library collisions belongs to later SemanticModel and
  project-reference beads.

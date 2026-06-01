# Lexer Literal Completion Evidence

Date: 2026-06-01
Bead: `bd-aprs.4.2`
Workset lane: FE-3.2 Literal lexing completion

## Outcome

`oxvba-syntax` now keeps the covered VBA literal forms atomic and lossless enough for the
front-end rework lexer lane:

- hexadecimal and octal literals accept integer type suffixes `%`, `&`, and `^`;
- decimal numeric literals accept numeric type suffixes `%`, `&`, `^`, `!`, `#`, and `@`;
- non-integer numeric suffixes `!`, `#`, and `@` classify as `FloatLiteral`, while integer
  suffixes remain `IntLiteral`;
- `Empty` and `Null` are syntax keywords and parse through the same literal expression path as
  `True`, `False`, and `Nothing`;
- unterminated string and malformed date literal fixtures recover losslessly through token text
  reconstruction.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-syntax --quiet`
  - Result: passed, 68 tests.
- `cargo fmt --check -p oxvba-syntax`
  - Result: passed after formatting.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The first suffix implementation kept `2#` and `2@` atomic but classified them as `IntLiteral`
because the suffix was trimmed before family classification. That would have been a misleading
token-family signal for later binder/HIR work, so classification now treats `!`, `#`, and `@`
as non-integer numeric literals even when the body has no decimal point or exponent.

Malformed literal handling is intentionally still lexer-level recovery, not a stable diagnostic
contract. The current lexer has no separate diagnostics API; the verified claim here is lossless
tokenization/reconstruction for malformed string/date fixtures. Stable diagnostic shape remains a
later parser/harness concern under the broader FE-3/FE-4 gates.

Residuals left for later beads:

- currency and decimal semantics are represented by token text/suffix rather than dedicated token
  kinds;
- invalid numeric suffix combinations and semantic range checks are not lexer responsibilities yet;
- broader corpus snapshots still belong to FE-3.4.

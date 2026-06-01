# Parser Statement Coverage Evidence

Date: 2026-06-01
Bead: `bd-aprs.5.3`
Workset lane: FE-4.3 Statement parser coverage

## Outcome

Hardened statement-level parsing in `oxvba-syntax`:

- added `Attribute` keyword recognition and an `AttributeStmt` CST node for exported module
  metadata lines;
- added statement-end handling that stops simple statement parsers at `:` instead of swallowing the
  rest of the physical line;
- taught block parsing to consume colon statement separators and continue parsing the next inline
  statement;
- added fixtures for exported attributes, inline assignments, inline `RaiseEvent`, and inline
  `On Error`/`Resume` sequences;
- retained existing statement coverage for declarations, blocks, `With`, `Property`, `Declare`,
  `Type`, `Enum`, `On Error`, `Resume`, and `RaiseEvent` round-trips.

After reopening, this bead was extended with production bridge route evidence rather than parser
snapshots alone:

- `syntax_bridge` validates a statement coverage corpus through `validate_source_with_cst`,
  including exported attributes, colon-separated inline statements, inline `RaiseEvent`, inline
  `On Error`/`Resume`, `With`, `Property`, `Declare`, `Type`, and `Enum`;
- `compile_source_via_syntax_bridge` still compiles a supported multiline assignment sequence after
  CST validation;
- the bridge test explicitly records that colon-separated inline assignment sequences are accepted
  by the CST parser but still rejected by the legacy compiler as one unsupported statement.

That last point is an implementation residual, not a parser residual: FE-4.4 owns splitting/lowering
inline statement lists into the legacy or replacement statement representation.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-syntax --quiet`
  - First-run result: passed, 78 unit tests plus 2 integration tests.
  - Reopen result: passed, 79 unit tests plus 2 integration tests.
- `cargo test -p oxvba-compiler syntax_bridge --quiet`
  - Reopen result: passed, 7 tests after adding statement coverage bridge validation.
- `cargo fmt --check -p oxvba-compiler -p oxvba-syntax`
  - Reopen result: passed.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The highest-risk behavior was colon-separated inline statements. Before this bead, statement
parsers such as assignment and `On Error` consumed to the physical line end, so `x = 1: y = 2`
round-tripped but did not produce two statement nodes. Simple statement parsers now stop at `:`,
and block parsing consumes the separator before continuing.

The attribute addition is intentionally syntax-only. Exported attributes are represented losslessly
as `AttributeStmt`; semantic interpretation of module metadata remains outside this parser bead.

Reopen fresh-eyes review found a critical distinction: the CST parser now correctly splits
colon-separated inline statements, but the legacy compiler still sees `x = 1: x = x + 1` as one
unsupported statement. The bridge tests now preserve that fact so FE-4.4 cannot claim statement
bridge closure from CST validation alone.

Residuals left for later beads:

- single-line `If ... Then ... Else ...` still needs richer statement-list structure;
- colon-separated inline statement lowering is parser-proven but still a FE-4.4 bridge/lowering
  residual;
- detailed declaration item parsing inside `Dim`, `Const`, `Type`, and `Enum` remains a typed facade
  and parser-expansion follow-up;
- CST-to-legacy lowering belongs to FE-4.4;
- diagnostic recovery snapshots belong to FE-4.5.

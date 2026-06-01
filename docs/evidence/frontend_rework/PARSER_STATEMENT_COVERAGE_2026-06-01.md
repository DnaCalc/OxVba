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

## Verification

Commands run from repository root:

- `cargo test -p oxvba-syntax --quiet`
  - Result: passed, 78 unit tests plus 2 integration tests.
- `cargo fmt --check -p oxvba-syntax`
  - Result: passed.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The highest-risk behavior was colon-separated inline statements. Before this bead, statement
parsers such as assignment and `On Error` consumed to the physical line end, so `x = 1: y = 2`
round-tripped but did not produce two statement nodes. Simple statement parsers now stop at `:`,
and block parsing consumes the separator before continuing.

The attribute addition is intentionally syntax-only. Exported attributes are represented losslessly
as `AttributeStmt`; semantic interpretation of module metadata remains outside this parser bead.

Residuals left for later beads:

- single-line `If ... Then ... Else ...` still needs richer statement-list structure;
- detailed declaration item parsing inside `Dim`, `Const`, `Type`, and `Enum` remains a typed facade
  and parser-expansion follow-up;
- CST-to-legacy lowering belongs to FE-4.4;
- diagnostic recovery snapshots belong to FE-4.5.

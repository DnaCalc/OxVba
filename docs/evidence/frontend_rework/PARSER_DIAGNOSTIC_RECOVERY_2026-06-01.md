# Parser Diagnostic Recovery Evidence

Date: 2026-06-01
Bead: `bd-aprs.5.5`
Workset lane: FE-4.5 Parser diagnostic recovery fixtures

## Outcome

Expanded IDE-style parser recovery coverage in `oxvba-syntax`:

- incomplete assignment and `Set` assignment fixtures continue to produce a diagnostic plus
  explicit `ErrorNode` without losing text;
- incomplete `If`, `For`, `Do`, and `With` block fixtures now preserve the partial tree and report
  stable recovery messages;
- `End Sub` is no longer silently accepted as the closing `End` token for incomplete `If` or `With`
  blocks.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-syntax --quiet`
  - Result: passed, 79 unit tests plus 2 integration tests.
- `cargo fmt --check -p oxvba-syntax`
  - Result: passed after formatting.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The first incomplete-block fixture exposed a real recovery bug: `If ...` followed by `End Sub` did
not report `expected End If` because the parser consumed `End` and did not validate the following
keyword. The same issue applied to `With`. The parser now checks the full compound terminator.

Diagnostics are still message strings plus byte offsets, not stable diagnostic IDs. That matches
the current parser API; formal diagnostic identity remains a later diagnostics/SemanticModel task.

Residuals left for later beads:

- single-line `If` recovery needs a dedicated statement-list parser;
- parser diagnostics still need stable IDs and richer expected-token metadata;
- semantic/binder diagnostics remain separate from syntax recovery.

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

After reopening, the diagnostic recovery proof was connected to the production bridge route:

- `compile_source_via_syntax_bridge` rejects recovered CST parse errors before legacy lowering;
- the bridge test proves incomplete assignment source remains lossless, contains an `ErrorNode`,
  carries the `expected expression after '='` parser diagnostic, and surfaces that syntax failure as
  a bridge error rather than compiling through legacy text parsing.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-syntax --quiet`
  - Result: passed, 79 unit tests plus 2 integration tests.
- `cargo test -p oxvba-compiler syntax_bridge --quiet`
  - Reopen result: passed, 8 tests after adding bridge diagnostic-route proof.
- `cargo fmt --check -p oxvba-compiler -p oxvba-syntax`
  - Reopen result: passed.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The first incomplete-block fixture exposed a real recovery bug: `If ...` followed by `End Sub` did
not report `expected End If` because the parser consumed `End` and did not validate the following
keyword. The same issue applied to `With`. The parser now checks the full compound terminator.

Diagnostics are still message strings plus byte offsets, not stable diagnostic IDs. That matches
the current parser API; formal diagnostic identity remains a later diagnostics/SemanticModel task.

Reopen fresh-eyes review checked for the old partial-closure trap: parser recovery fixtures alone
are not enough if production compile can still fall through to legacy lowering. The bridge now stops
on CST diagnostics first, so incomplete edit states are visible to compiler and IDE entry points
instead of being silently reinterpreted downstream.

Residuals left for later beads:

- single-line `If` recovery needs a dedicated statement-list parser;
- parser diagnostics still need stable IDs and richer expected-token metadata;
- semantic/binder diagnostics remain separate from syntax recovery.

# Lexer Snapshot Corpus Evidence

Date: 2026-06-01
Bead: `bd-aprs.4.4`
Workset lane: FE-3.4 Lexer snapshot corpus

## Outcome

Added `crates/oxvba-syntax/tests/lexer_snapshot_corpus.rs` with two coverage layers:

- exact token snapshots for grammar-row lexical forms: options, bracketed identifiers, typed
  literals, escaped strings, `Rem` trivia, line continuation, and keyword-colliding suffixed names;
- recursive lossless tokenization over checked-in VBA source fixture roots (`.bas`, `.cls`, and
  `.frm`).

After reopening, the corpus was widened to include the production migration fixture roots used by
the FE-5/FE-9 gates, not only the original focused syntax and conformance roots. The broad corpus
currently covers 309 checked-in VBA source files:

- `conformance/tests`: 214
- `conformance/vm_package/identity_seed`: 17
- `conformance/com`: 21
- `conformance/integration/projects`: 26
- `conformance/jit_v2/tracer_bullets`: 9
- `crates/oxvba-debug/tests/fixtures`: 7
- `examples/basic`: 9
- `examples/reflection_wrapper`: 2
- `examples/xll`: 4

The corpus test asserts that token text reconstructs each source exactly. This is intentionally a
lexer guarantee, not a claim that the current parser accepts every fixture.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-syntax --quiet`
  - First-run result: passed, 70 unit tests plus 2 integration tests.
- `cargo test -p oxvba-syntax checked_in_vba_fixture_corpus_tokenizes_losslessly --quiet`
  - Reopen result: passed after widening the production migration corpus roots and source
    extensions.
- `cargo fmt --check -p oxvba-syntax`
  - First-run result: passed after formatting.
- `git diff --check`
  - First-run result: passed.

## Fresh-Eyes Review

The main risk was overclaiming parser or semantic parity from a lexer-only bead. The new broad test
therefore checks lossless tokenization only. Exact token-kind snapshots are limited to grammar-row
lexical surfaces where FE-3 has made concrete claims.

Fresh-eyes review after reopening found one omission in the first-run corpus: it collected `.bas`
files only, so class modules in example/project roots were not covered. The test now collects
`.bas`, `.cls`, and `.frm` files and includes the checked-in example roots that serve as later
metadata/API and host-callable production migration fixtures.

The corpus roots are checked-in repository fixtures rather than optional external corpora, so the
test is deterministic in normal workspace checkouts. The test uses a 300-file lower-bound count
rather than an exact file count so adding future fixtures does not require mechanical test updates
while still guarding against accidentally dropping a production migration root.

Residuals left for later beads:

- parser acceptance and diagnostic recovery for the same corpus belong to FE-4;
- semantic execution/differential checks belong to FE-5 and later binder/HIR lanes;
- optional external real-world corpora remain inventory items, not required FE-3 gate inputs.

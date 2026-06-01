# Lexer Snapshot Corpus Evidence

Date: 2026-06-01
Bead: `bd-aprs.4.4`
Workset lane: FE-3.4 Lexer snapshot corpus

## Outcome

Added `crates/oxvba-syntax/tests/lexer_snapshot_corpus.rs` with two coverage layers:

- exact token snapshots for grammar-row lexical forms: options, bracketed identifiers, typed
  literals, escaped strings, `Rem` trivia, line continuation, and keyword-colliding suffixed names;
- recursive lossless tokenization over checked-in `.bas` fixture roots.

The broad corpus currently covers 294 `.bas` files:

- `conformance/tests`: 214
- `conformance/vm_package/identity_seed`: 17
- `conformance/com`: 21
- `conformance/integration/projects`: 26
- `conformance/jit_v2/tracer_bullets`: 9
- `crates/oxvba-debug/tests/fixtures`: 7

The corpus test asserts that token text reconstructs each source exactly. This is intentionally a
lexer guarantee, not a claim that the current parser accepts every fixture.

## Verification

Commands run from repository root:

- `cargo test -p oxvba-syntax --quiet`
  - Result: passed, 70 unit tests plus 2 integration tests.
- `cargo fmt --check -p oxvba-syntax`
  - Result: passed after formatting.
- `git diff --check`
  - Result: passed.

## Fresh-Eyes Review

The main risk was overclaiming parser or semantic parity from a lexer-only bead. The new broad test
therefore checks lossless tokenization only. Exact token-kind snapshots are limited to grammar-row
lexical surfaces where FE-3 has made concrete claims.

The corpus roots are checked-in repository fixtures rather than optional external corpora, so the
test is deterministic in normal workspace checkouts. The test uses a lower-bound count rather than
an exact file count so adding future fixtures does not require mechanical test updates while still
guarding against accidentally testing an empty or tiny corpus.

Residuals left for later beads:

- parser acceptance and diagnostic recovery for the same corpus belong to FE-4;
- semantic execution/differential checks belong to FE-5 and later binder/HIR lanes;
- optional external real-world corpora remain inventory items, not required FE-3 gate inputs.

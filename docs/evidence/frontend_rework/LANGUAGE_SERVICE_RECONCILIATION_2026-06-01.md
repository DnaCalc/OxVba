# Language-Service Reconciliation Evidence

Date: 2026-06-01
Bead: `bd-aprs.10.4`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Reworked the language-service integration so IDE-facing semantic answers can come from the same
front-end query/HIR/SemanticModel facts used by the compiler-side front-end path.

Changes:

- `crates/oxvba-compiler/src/frontend_language_service.rs` now exposes
  `answer_ide_query_from_source`, which builds through `FrontendQueryDatabase` and answers from
  the shared `SemanticModel` rather than requiring callers to hand-assemble semantic facts.
- `crates/oxvba-languageservice/src/semantic.rs` now prefers compiler front-end query/HIR facts
  when building a `SemanticSnapshot`:
  - diagnostics come from `FrontendQueryDatabase::diagnostics`;
  - document symbols are projected from typed HIR `SymbolModel` + type hooks;
  - the previous CST/legacy `BoundModule` correlation remains only as a compatibility fallback for
    syntax the new front-end cannot bind yet.
- The existing legacy `BoundModule` is still retained inside `SemanticSnapshot` because older
  service features such as signature help still read procedure metadata from it. This is an
  explicit remaining compatibility surface, not the preferred symbol/diagnostic source.

Executable proof:

- compiler-side IDE query from source resolves a symbol via the query-backed `SemanticModel`;
- language-service snapshots preserve parameter/local scopes and declared types from compiler HIR
  facts (`ByVal seed As Long`, `Dim label As String`);
- full `oxvba-languageservice` tests still pass, covering hover, completions, symbols,
  diagnostics, navigation, rename analysis, and host-session surfaces.

## Checks

- `cargo test -p oxvba-compiler frontend_language_service --quiet`
- `cargo test -p oxvba-languageservice semantic --quiet`
- `cargo test -p oxvba-languageservice --quiet`
- `cargo test -p oxvba-compiler --quiet`
- `cargo check -p oxvba-compiler --quiet`
- `cargo check -p oxvba-languageservice --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `cargo fmt --check -p oxvba-languageservice`
- `git diff --check`

## Fresh-Eyes Review

- The previous FE-9.4 evidence was too weak for the reopened workset: it added a compiler-side
  helper but left `oxvba-languageservice` building symbols and diagnostics through its own
  resolver/correlation path.
- The service now consumes the compiler-owned query/HIR facts for the supported snapshot surface,
  which makes the IDE and compiler front-end agree on symbols, declared types, and diagnostics for
  that surface.
- Not all language-service internals are retired. Signature help and some workspace features still
  use `SemanticSnapshot.bound` as compatibility data. That residual is visible and should be part
  of FE-9.6/terminal audit rather than hidden behind this bead.

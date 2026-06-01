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
  - diagnostics come from `FrontendQueryDatabase::diagnostics`, including the PtrSafe-required
    Declare diagnostic that drives the language-service quick fix;
  - document symbols are projected from typed HIR `SymbolModel` + type hooks;
  - callable signatures are projected into `SemanticSnapshot::callables` from typed HIR and type
    hooks, and signature help resolves against that projection;
  - unsupported front-end syntax reports front-end diagnostics instead of rebuilding legacy
    `BoundModule` symbol/callable correlation.
- The existing legacy `BoundModule` is no longer retained or exposed on `SemanticSnapshot`, is no
  longer used by signature help, and is no longer built by the language-service semantic snapshot
  path.

Executable proof:

- compiler-side IDE query from source resolves a symbol via the query-backed `SemanticModel`;
- language-service snapshots preserve parameter/local scopes and declared types from compiler HIR
  facts (`ByVal seed As Long`, `Dim label As String`);
- language-service signature help resolves procedure parameters from `SemanticSnapshot::callables`
  rather than `BoundModule::procedures`;
- PtrSafe quick fixes are driven by the front-end diagnostic layer without forcing a legacy
  `BoundModule` build for otherwise HIR-supported snapshots;
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
- The reopened continuation removed the fallback `BoundModule` build from `semantic.rs`. This means
  unsupported front-end syntax no longer gets a second, divergent semantic answer from the
  language-service layer; it gets front-end diagnostics until the relevant compiler-front-end facts
  are implemented.

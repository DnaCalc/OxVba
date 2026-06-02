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
- follow-up FE-9.7 language-service coverage builds semantic snapshots for representative
  grammar-matrix route-overlay shapes (`Static`, exponent expressions, qualified member chains,
  and trivia/comment-bearing sources) from compiler front-end facts with no language-service
  diagnostics, so the IDE surface now samples beyond the original seed rows.
- follow-up FE-9.7 workspace coverage now drives the existing `INTP-003` and `INTP-016` seed
  project manifests beyond symbol enumeration: the real qualified `LibMath.MathApi.AddFour` call
  navigates to the referenced project document with project-reference provenance, and the real
  `Adder.Multiply` class member call navigates to the class document and exposes signature help
  from the shared front-end callable facts.
- follow-up FE-9.7 imported-typelib coverage now loads multiple projected `Scripting` classes and
  drives projected `Dictionary.Count` through workspace search, go-to-definition, and signature
  help with imported-typelib provenance. This is still identifier-based IDE resolution evidence,
  not proof of complete type-directed COM member binding.
- follow-up FE-9.7 optional-default coverage now carries literal string and Boolean optional
  parameter defaults from the compiler front-end signature parser into `SemanticSnapshot::callables`
  and signature help. This proves the IDE callable surface can expose the same richer optional
  metadata now used by runtime descriptors for these literal defaults; it does not claim complete
  module-constant, Date/Currency, or arbitrary default-expression coverage. The same continuation
  also proves the `ParamArray` flag is preserved on snapshot callables and signature help.

## Checks

- `cargo test -p oxvba-compiler frontend_language_service --quiet`
- `cargo test -p oxvba-languageservice semantic --quiet`
- `cargo test -p oxvba-languageservice snapshot_covers_matrix_route_overlay_shapes_from_frontend_hir --quiet`
- `cargo test -p oxvba-languageservice workspace_symbols_cover_frontend_seed_project_routes --quiet`
- `cargo test -p oxvba-languageservice project_aware_workspace_loads_projected_typelib_references --quiet`
- `cargo test -p oxvba-languageservice snapshot_callables_preserve_optional_string_boolean_defaults --quiet`
- `cargo test -p oxvba-languageservice signature_help_preserves_optional_string_boolean_defaults --quiet`
- `cargo test -p oxvba-languageservice snapshot_callables_preserve_param_array_flag --quiet`
- `cargo test -p oxvba-languageservice signature_help_preserves_param_array_flag --quiet`
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
- The matrix-overlay snapshot check is deliberately source-level IDE evidence. It does not prove
  richer host/document semantics, imported COM breadth beyond the current routed cases, live Excel
  oracle behavior, or deeper multi-project workspace interactions; those remain FE-9.7/FE-7/FE-8
  delivery surfaces rather than documentation closure.
- The workspace route continuation deliberately uses existing integration seed project sources, not
  synthetic mini-projects, so it catches drift between the project route audit and IDE navigation
  route. It remains bounded to the current `INTP-003` reference-project and `INTP-016` class-member
  shapes.
- The imported-typelib continuation now covers more than the original single unqualified
  `FileSystemObject.GetBaseName` case by adding projected `Scripting.Dictionary.Count` to workspace
  search, navigation, and signature help. The resolver still chooses projected callables by
  identifier, so this does not close richer typed COM member/property/default-member or live
  reference behavior; predeclared document host behavior and live Excel execution still need their
  own route runners.
- The optional-default continuation intentionally recovers optional/default flags from the compiler
  signature parser because HIR params currently carry names and type hooks but not the full
  resolved parameter descriptor. That keeps the IDE result aligned with the production compiler
  parser for literal string/Boolean defaults, but it also marks a remaining cleanup: parameter
  descriptors should become first-class HIR/SemanticModel facts before claiming full Roslyn-style
  callable metadata parity.

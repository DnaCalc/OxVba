# Diagnostic Mapping Evidence

Date: 2026-06-01
Bead: `bd-aprs.7.5`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_diagnostics.rs`, a diagnostic mapping layer for the
front-end pipeline.

The mapper supports:

- parser diagnostics mapped from byte offsets to stable one-byte spans;
- binder diagnostics with stable front-end codes;
- type diagnostics with stable front-end codes;
- legacy-compatible diagnostics that preserve an existing legacy code/family when applicable;
- conversion to `SemanticDiagnostic` so SemanticModel span queries can consume mapped diagnostics.

Reopened update: parser diagnostics now have an executable source route through
`FrontendDiagnosticMapper::from_source_parse`. It parses with `oxvba-syntax`, maps real
`ParseError` values into stable front-end parser diagnostics, and feeds those diagnostics into the
SemanticModel query surface.

## Focused Fixtures

The tests verify:

- parser error at byte offset `12` maps to span `12..13` and `PARSE-E-SYNTAX`;
- a real parser recovery error from `x =` maps through `from_source_parse` and appears as a
  SemanticModel diagnostic;
- binder/type diagnostics retain their families and stable codes;
- legacy-compatible diagnostics retain the legacy code;
- mapped diagnostics feed `SemanticModel::diagnostics_for_span`.

## Checks

- `cargo test -p oxvba-compiler frontend_diagnostics --quiet`
- `cargo test -p oxvba-compiler frontend_semantic_model --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The mapper is deliberately simple and deterministic. It does not attempt to render human-facing
  diagnostics; it records stable family/code/span facts for compiler and IDE consumers.
- Existing compatibility codes are optional but preserved when provided, which keeps current
  conformance expectations reachable without forcing every new diagnostic into an old string shape.
- Parser offsets currently become one-byte spans. The new source-backed route proves real parser
  errors are wired through; later parser work can widen spans when richer token/node ranges are
  available without changing the diagnostic family model.
- Binder/type diagnostics are still pushed by callers rather than produced by the source-backed
  binder/type hooks. FE-6.6 owns wiring those diagnostics into the production binder path.

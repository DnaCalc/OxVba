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

## Focused Fixtures

The tests verify:

- parser error at byte offset `12` maps to span `12..13` and `PARSE-E-SYNTAX`;
- binder/type diagnostics retain their families and stable codes;
- legacy-compatible diagnostics retain the legacy code;
- mapped diagnostics feed `SemanticModel::diagnostics_for_span`.

## Checks

- `cargo test -p oxvba-compiler frontend_diagnostics --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The mapper is deliberately simple and deterministic. It does not attempt to render human-facing
  diagnostics; it records stable family/code/span facts for compiler and IDE consumers.
- Existing compatibility codes are optional but preserved when provided, which keeps current
  conformance expectations reachable without forcing every new diagnostic into an old string shape.
- Parser offsets currently become one-byte spans. Later parser work can widen spans when richer
  token/node ranges are available without changing the diagnostic family model.

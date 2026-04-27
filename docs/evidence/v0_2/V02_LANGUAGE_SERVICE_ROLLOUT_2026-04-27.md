# V0.2 Language-Service Roundout Rollout

Date: 2026-04-27

Bead: `bd-bqm8.8.1`

## Scope

`bd-bqm8.8` covers the V0.2 product-facing language-service surface for direct
OxIde embedding, `oxvba-lsp`, and the VS Code alternate-editor path. Existing
code already exposes diagnostics, symbols, hover, definition, references,
completion, rename preparation, code actions, semantic classifications, and
workspace loading, but the V0.2 lane needs explicit API, transport, host, and
evidence gates.

## Child Beads

- `bd-bqm8.8.1`: audit and roll out language-service child beads.
- `bd-bqm8.8.2`: publish the V0.2 language-service product matrix.
- `bd-bqm8.8.3`: harden direct `oxvba-languageservice` semantic query tests.
- `bd-bqm8.8.4`: harden `oxvba-lsp` transport/workspace tests.
- `bd-bqm8.8.5`: publish OxIde/VS Code host-consumption guidance and boundaries.
- `bd-bqm8.8.6`: run final language-service checklist and close `bd-bqm8.8`
  only if direct API, LSP transport, host guidance, and residual boundaries are
  explicit.

## Inventory

- Direct service implementation: `crates/oxvba-languageservice/src/service.rs`.
- Workspace model: `crates/oxvba-languageservice/src/workspace.rs`.
- LSP bridge: `crates/oxvba-lsp/src/lib.rs` and `crates/oxvba-lsp/src/main.rs`.
- Existing public docs: `docs/LANGUAGE_SERVICE_PUBLIC_INTERFACE.md`,
  `docs/LANGUAGE_SERVICE_HOST_BOUNDARIES.md`, `docs/LANGUAGE_SERVICE_SHOWCASE.md`,
  `docs/spec/LANGUAGE_SERVICE_SPEC_V1.md`, and
  `docs/spec/LANGUAGE_SERVICE_PLATFORM_SPEC_V2.md`.
- Prior worksets: `WORKSET_2026-03-31_LANGUAGE_SERVICE_FIRST_CLASS_PLATFORM_EXECUTION.md`,
  `WORKSET_2026-04-01_LSP_TRANSPORT_DISCIPLINE_AND_HOST_PROJECT_HELPERS.md`,
  and `WORKSET_2026-04-02_VSCODE_EXTENSION_AND_LSP_FEATURE_LADDER_EXECUTION.md`.

## Checks Run

- `rg --files crates/oxvba-languageservice crates/oxvba-lsp docs scripts | rg "(?i)(language|lsp|vscode|oxide|semantic|hover|completion|definition|diagnostic|workspace)"`
- `rg -n "LanguageService|hover|completion|definition|diagnostic|workspace|rename|semantic|CodeAction|document_symbol|references" crates/oxvba-languageservice crates/oxvba-lsp docs scripts -g "*.rs" -g "*.md" -g "*.ps1"`

## Result

`bd-bqm8.8.1` is complete as a rollout bead. The language-service capability lane
remains in-progress until the matrix, direct-service tests, LSP tests, host
guidance, and final checklist beads complete.

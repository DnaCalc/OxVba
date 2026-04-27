# V0.2 Language-Service LSP Transport Tests

Date: 2026-04-27

Bead: `bd-bqm8.8.4`

## Scope

This bead hardens the `oxvba-lsp` transport layer for the V0.2 language-service
product matrix. The LSP crate remains a thin transport/session shell over
`oxvba-languageservice`; semantic ownership stays in the direct service.

## Added Test

Added:

- `v02_lsp_core_exposes_transport_neutral_product_matrix_queries`

The test loads a project-backed workspace, opens a synchronized document overlay,
and exercises `OxvbaLspCore` across:

- URI-to-document mapping and synchronized document identity,
- semantic provenance from the loaded project,
- diagnostics substrate,
- document and workspace symbols,
- semantic classifications,
- completions and signature help,
- hover, go-to-definition, and find-references,
- rename preparation with safe reference analysis,
- diagnostics-driven code actions.

## Existing Coverage Refreshed

The full `oxvba-lsp` suite also covers:

- server info and advertised bounded LSP capabilities,
- full-text open/change/close synchronization,
- rejection of documents outside the loaded workspace,
- restoration of project-backed baseline sources on close,
- workspace reload clearing stale URI mappings,
- referenced-project document loading,
- root and referenced-project URI mapping,
- local editor sync/load latency budget.

## Checks Run

- `cargo test -p oxvba-lsp v02_lsp_core_exposes_transport_neutral_product_matrix_queries -- --nocapture`
- `cargo test -p oxvba-lsp -- --nocapture`

Result: full crate suite passed with existing deprecation warnings from
`tower_lsp::lsp_types::{DocumentSymbol, SymbolInformation}::deprecated`.

## Result

`bd-bqm8.8.4` is complete for LSP transport and workspace tests. The
language-service lane remains in-progress pending host-consumption guidance and
the final checklist.

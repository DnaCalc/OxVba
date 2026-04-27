# V0.2 Language-Service Host Consumption Guidance

Date: 2026-04-27

Bead: `bd-bqm8.8.5`

## Scope

This bead refreshes the OxIde and VS Code host-consumption guidance for the
V0.2 language-service lane.

## Updated Guidance

Updated:

- `docs/LANGUAGE_SERVICE_PUBLIC_INTERFACE.md`
- `docs/LANGUAGE_SERVICE_HOST_BOUNDARIES.md`
- `docs/LANGUAGE_SERVICE_SHOWCASE.md`

The refresh records:

- OxIde-class hosts consume `HostWorkspaceSession`, `oxvba-project` helper
  plans, `EmbeddedBuildRunHost`, `EmbeddedRunSession`, Immediate Window, and
  debugger substrates directly.
- OxIde should not route semantic editor behavior through LSP or parse CLI text
  for editor/build/run/debugger behavior.
- VS Code-class hosts consume `oxvba-lsp` as a thin transport for language
  features.
- VS Code project creation/module/reference authoring remains outside LSP in
  extension commands or CLI/direct-helper flows.
- Debugging for VS Code remains a later DAP projection over the same OxVba
  debugger core, not an `oxvba-lsp` responsibility.

## Boundary Corrections

The prior docs still described the LSP surface as mostly sync-only. They now
match the current tested `oxvba-lsp` state:

- advertised bounded capabilities,
- full-text synchronization,
- diagnostics,
- document/workspace symbols,
- hover,
- definition,
- references,
- completion,
- signature help,
- prepare-rename,
- code actions,
- semantic tokens.

The docs still explicitly exclude:

- full LSP parity with the direct API,
- LSP-owned project authoring,
- multi-root LSP semantics,
- complete VS Code extension packaging,
- designer/forms editing,
- complete refactoring parity.

## Checks Run

- `rg "does not yet advertise|sync-only|currently exposed LSP method surface" docs/LANGUAGE_SERVICE_PUBLIC_INTERFACE.md docs/LANGUAGE_SERVICE_HOST_BOUNDARIES.md docs/LANGUAGE_SERVICE_SHOWCASE.md`
- `./scripts/check-governance.ps1`
- `git diff --check`

## Result

`bd-bqm8.8.5` is complete for OxIde and VS Code host-consumption guidance. The
language-service lane remains in-progress pending the final checklist.

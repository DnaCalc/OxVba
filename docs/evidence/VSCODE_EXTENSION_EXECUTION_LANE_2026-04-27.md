# VS Code Extension Execution Lane

Date: 2026-04-27
Bead: `bd-oxi1.7`

## Scope

Close the planning gap for the alternate VS Code editor integration path while
preserving OxIde as the direct-host reference path.

## Current Lane

- `oxvba-lsp` is the thin language-feature transport over the direct
  `oxvba-languageservice` semantic core.
- `extensions/vscode-oxvba` is the bounded VS Code extension shell.
- The extension owns project-authoring commands instead of pushing project
  mutation semantics into LSP:
  - initialize project
  - capture convention folder
  - add COM reference
  - repair COM references
- Debugging is reserved for a later DAP projection over the shared OxVba
  debugger model documented in `docs/spec/OXVBA_DEBUGGER_DAP_PROJECTION_V1.md`.

## Validation

```powershell
cargo test -p oxvba-lsp --quiet
node -e "const fs=require('fs'); JSON.parse(fs.readFileSync('extensions/vscode-oxvba/package.json','utf8')); JSON.parse(fs.readFileSync('extensions/vscode-oxvba/language-configuration.json','utf8')); console.log('vscode extension json ok')"
```

Results:

- `cargo test -p oxvba-lsp --quiet`: pass, 10/10 unit tests and 4/4 integration tests
- VS Code extension JSON parse check: pass

## Boundary

This closes the execution-lane definition. It does not claim marketplace
packaging, VS Code debug adapter delivery, or that VS Code is the primary
OxVba host.

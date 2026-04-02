# OxVba VS Code Shell

This is the first bounded VS Code shell for OxVba.

Current scope:
- registers the `oxvba` language id
- associates `.bas`, `.cls`, `.frm`, `.basproj`, and `.vbp`
- launches the repo-local `oxvba-lsp` binary over stdio
- exposes bounded extension-owned project commands over `oxvba-cli`:
  - initialize project
  - capture convention folder
  - add COM reference
  - repair COM references

Current non-goals:
- full project authoring parity
- full feature-complete LSP coverage
- debugger delivery

Debugger path:
- the future VS Code debugger should project the shared OxVba debugger core through the DAP mapping documented in `docs/spec/OXVBA_DEBUGGER_DAP_PROJECTION_V1.md`
- the extension should stay adapter-only and must not invent debugger semantics locally

## Local Use

1. Build `oxvba-lsp`:
   - `cargo build -p oxvba-lsp`
2. Open this folder in VS Code as an extension development host, or package it later.
3. If needed, set `oxvba.server.path` to an explicit `oxvba-lsp` executable.
4. If needed, set `oxvba.cli.path` to an explicit `oxvba-cli` executable.

The extension prefers:
- `target/debug/oxvba-lsp`
- `target/release/oxvba-lsp`

relative to the OxVba repo root.

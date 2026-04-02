# OxVba VS Code Shell

This is the first bounded VS Code shell for OxVba.

Current scope:
- registers the `oxvba` language id
- associates `.bas`, `.cls`, `.frm`, `.basproj`, and `.vbp`
- launches the repo-local `oxvba-lsp` binary over stdio

Current non-goals:
- project authoring commands
- full feature-complete LSP coverage
- debugger delivery

## Local Use

1. Build `oxvba-lsp`:
   - `cargo build -p oxvba-lsp`
2. Open this folder in VS Code as an extension development host, or package it later.
3. If needed, set `oxvba.server.path` to an explicit `oxvba-lsp` executable.

The extension prefers:
- `target/debug/oxvba-lsp`
- `target/release/oxvba-lsp`

relative to the OxVba repo root.

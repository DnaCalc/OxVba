# Workset: VS Code Extension And LSP Feature Ladder Execution

Date: 2026-04-02
Owner: Codex
Status: in-progress

## Near-Term Priority Position

This is an alternate-editor lane and is intentionally lower priority than the current OxIde-facing Immediate Window / live-session REPL work.

## Purpose

Deliver a credible alternate-editor integration for OxVba in VS Code by:
- completing the minimum honest `oxvba-lsp` protocol surface,
- packaging a small VS Code extension,
- and keeping project-authoring and richer host workflows outside LSP.

## Why This Exists

OxVba already has:
- a strong direct language-service core,
- a thin `oxvba-lsp` transport,
- and explicit host-boundary doctrine that says LSP is transport, not truth.

What it does not yet have is a usable VS Code integration:
- no extension package,
- no protocol exposure for most semantic queries,
- no extension commands for project-authoring workflows,
- and no debug lane over an OxVba-native debugging model.

## Governing Policy

1. `oxvba-languageservice` remains the semantic source of truth.
2. `oxvba-lsp` remains transport-only; it must not own project rules.
3. Project creation and authoring stay in extension commands or direct OxVba helper APIs, not LSP.
4. The extension should be small glue over OxVba binaries and protocols, not a second language engine.
5. Debugging is a sibling lane over a semantic OxVba debugger core, not native-stack stepping.

## Required Outcomes

1. VS Code can recognize OxVba files/projects and start `oxvba-lsp`.
2. The minimum language-feature ladder is exposed over LSP:
   - diagnostics
   - document/workspace symbols
   - hover
   - definition
   - references
   - completion
   - signature help
   - rename
   - code actions
   - semantic tokens
3. The extension owns project commands such as:
   - create project
   - add module/class
   - add/manage references
   - open project/convert convention folder
4. The extension can route the future debugger lane through DAP or extension commands without inventing semantics locally.

## Execution Slices

1. package the minimal extension shell
2. expose diagnostics and basic navigation features through `oxvba-lsp`
3. expose completion/signature help/rename/code actions/semantic tokens
4. add extension-owned project commands over OxVba helpers
5. wire the later debugger lane

Current execution state:
- policy/workset is published
- debugger DAP projection guidance is now published in `docs/spec/OXVBA_DEBUGGER_DAP_PROJECTION_V1.md`
- the minimal VS Code extension shell now exists under `extensions/vscode-oxvba`
- `oxvba-lsp` now publishes diagnostics and answers document/workspace symbols, hover, definition, and references over the same direct semantic core
- `oxvba-lsp` now also exposes completion, signature help, rename, code actions, and semantic tokens over the same direct semantic core
- the next delivery slice is extension-owned project commands over OxVba helper APIs

## Non-Goals

- making VS Code the primary OxVba host
- duplicating project logic in TypeScript
- claiming full LSP coverage before the methods are real
- native-stack debugging through LLDB/GDB/CDB

## Relationship To Other Worksets

- Policy parent:
  - `WORKSET_2026-04-01_OXIDE_HOST_SURFACE_AND_VSCODE_ALTERNATE_EDITOR_EXECUTION.md`
- Shared debugger core:
  - `WORKSET_2026-04-02_OXVBA_DEBUGGING_SERVICE_AND_HOST_INTEGRATION.md`
- Debugger DAP projection guidance:
  - `docs/spec/OXVBA_DEBUGGER_DAP_PROJECTION_V1.md`

## Exit Condition

This workset is complete only when:
- a minimal VS Code extension exists in-repo,
- the minimum honest language-feature ladder works over `oxvba-lsp`,
- project commands are clearly split between extension and OxVba helper APIs,
- and the debug path is explicitly connected to the OxVba debugger lane rather than left as an abstraction gap.

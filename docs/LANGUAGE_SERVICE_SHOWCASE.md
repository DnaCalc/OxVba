# Language Service Showcase

This document is the user/developer-facing status note for OxVba language services.

It is intentionally narrower than the platform specs:
- it describes what is implemented now,
- it shows how to exercise it today,
- and it states what is still outside the current claim.

For the architectural target, see:
- `docs/spec/LANGUAGE_SERVICE_PLATFORM_SPEC_V2.md`

For host-boundary guidance, see:
- `docs/LANGUAGE_SERVICE_HOST_BOUNDARIES.md`
- `docs/OXIDE_DIRECT_HOST_SHOWCASE_BOUNDARY.md`

For the direct public host-facing surface, see:
- `docs/LANGUAGE_SERVICE_PUBLIC_INTERFACE.md`

## Current Capability Ladder

The current repo state supports these levels:

1. `LS-P0`: bounded internal substrate
   - workspace model
   - semantic snapshots
   - stable symbol identity and provenance
   - invalidation stats and local editor-budget harnesses

2. `LS-P1`: direct Rust query surface
   - direct host-facing workspace/document session via `HostWorkspaceSession`
   - direct host-facing workspace-target roster/reference inspection via `inspect_workspace_target`
   - diagnostics
   - document symbols
   - workspace symbols
   - semantic classification
   - completions
   - signature help
   - hover
   - go to definition
   - find references
   - rename preparation
   - safe reference-update analysis
   - bounded diagnostics-driven code-action planning

3. `LS-P2`: first transport and embedding shell
   - `oxvba-lsp` stdio server bootstrap
   - transport-owned `initialize` / `initialized` / `shutdown`
   - full-text `didOpen` / `didChange` / `didClose` synchronization
   - workspace loading from `.basproj`, bounded `.vbp`, or convention directories
   - protocol-exposed diagnostics, symbols, hover, definition, references,
     completion, signature help, prepare-rename, code actions, and semantic
     tokens over the direct semantic core
   - CLI debug harness: `oxvba-lsp debug-workspace <path>`

The current implementation does not claim a full `LS-P3` IDE or full LSP parity
surface. It claims a bounded transport projection over the direct semantic core.

## What You Can Use Today

### Direct Rust API

The rich surface lives in `crates/oxvba-languageservice`.

That crate is the semantic source of truth for:
- project-aware workspace loading,
- symbol and diagnostic queries,
- rename/reference analysis,
- and bounded code-action planning.

The LSP transport is intentionally layered on top of that crate and does not own a second parser or semantic model.

That direct Rust API is the primary editor-integration story for OxIde-style hosts.
The current gap is no longer the absence of a session seam. The first bounded host-facing session contract now exists as `HostWorkspaceSession`; the remaining work is to broaden authoring/build flows and have OxIde consume that contract directly.

For the current honest OxIde-facing evidence boundary, see:
- `docs/OXIDE_DIRECT_HOST_SHOWCASE_BOUNDARY.md`

### Thin LSP Shell

The `oxvba-lsp` binary can already run as a stdio server:

```powershell
oxvba-lsp
```

Current protocol truth:
- it supports server startup/shutdown,
- it supports full-text document synchronization,
- it can preload a workspace from `rootUri` / `workspaceFolders`,
- and it exposes bounded language features for diagnostics, document/workspace
  symbols, hover, definition, references, completion, signature help,
  prepare-rename, code actions, and semantic tokens.

Current protocol boundary:
- it is single-root,
- it does not own project creation or project editing,
- it does not implement rename apply orchestration,
- it is not a complete VS Code extension package.

### Debug Harness

The easiest external proof path today is:

```powershell
oxvba-lsp debug-workspace .\MyProject
```

That command:
- loads a workspace through the real OxVba project loader,
- builds the language-service workspace,
- lists loaded documents,
- and prints document diagnostics.

This is meant as a host/debug harness, not as a polished end-user editor UX.

## Current Boundary

These things are inside the current claim:
- project-aware language-service analysis
- project-reference-aware workspace loading
- imported-typelib-aware workspace loading through the existing projected-reference seam
- stable symbol identity/provenance for editor-facing semantic queries
- bounded code-action planning tied to diagnostics
- thin transport synchronization over the same workspace model

These things are not inside the current claim:
- full LSP parity
- Roslyn parity
- rust-analyzer parity
- rename apply/edit orchestration over LSP
- broad refactoring/code-action families
- workspace-wide multi-root transport semantics beyond the current single-core shell

## Validation Snapshot

Current validation evidence includes:
- direct language-service unit coverage in `crates/oxvba-languageservice`
- transport sync/debug coverage in `crates/oxvba-lsp`
- multi-module and reference-aware transport regressions
- bounded local-editor latency tests for transport sync round-trips

Canonical validation rows are tracked in:
- `docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv`

The important honesty rule is:
- the direct API surface is the canonical semantic and project model,
- `oxvba-lsp` is a thin, bounded transport projection over that model,
- VS Code-class project authoring and debugging should use extension commands
  and DAP-style projections rather than expanding LSP into a second product
  model.

## Near-Term Next Steps

The next likely expansion points are:
- capture real OxIde-side consumption evidence over `HostWorkspaceSession`,
  project helpers, embedded build/run, immediate, and debugger seams,
- add any missing typed workspace/document/session helpers only when OxIde
  adoption exposes concrete gaps,
- keep transport synchronization aligned with the real project model,
- and publish a broader end-to-end editor showcase with OxIde as the direct-host reference consumer.

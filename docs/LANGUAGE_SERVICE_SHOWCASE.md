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
   - CLI debug harness: `oxvba-lsp debug-workspace <path>`

The current implementation does not yet claim a full `LS-P3` editor transport with the semantic query ladder exposed over LSP methods.

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
The current gap is not semantic capability; it is packaging the existing capability into a clearer first-class host-facing service contract.

### Thin LSP Shell

The `oxvba-lsp` binary can already run as a stdio server:

```powershell
oxvba-lsp
```

Current protocol truth:
- it supports server startup/shutdown,
- it supports full-text document synchronization,
- and it can preload a workspace from `rootUri` / `workspaceFolders`.

Current protocol boundary:
- it does not yet advertise hover, completion, definition, references, rename, code actions, symbols, or semantic tokens as LSP methods.

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
- protocol-exposed semantic query coverage for the whole direct API
- rename apply/edit orchestration over LSP
- broad refactoring/code-action families
- semantic tokens wire protocol support
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
- the direct API surface is richer than the current LSP protocol surface,
- so OxVba should be described as having a first-class language-service core with a thin, still-bounded transport layer.

## Near-Term Next Steps

The next likely expansion points are:
- tighten the public direct host-facing interface for OxIde-class hosts,
- add any missing typed workspace/document/session helpers needed by OxIde,
- expose selected direct queries over actual LSP methods for VS Code-class hosts,
- keep transport synchronization aligned with the real project model,
- and publish a broader end-to-end editor showcase with OxIde as the direct-host reference consumer.

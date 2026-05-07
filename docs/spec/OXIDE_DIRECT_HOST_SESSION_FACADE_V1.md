# OxIde Direct Host Session Facade V1

This document defines the first bounded direct-host session facade that OxIde should consume from OxVba.

The implemented V1 surface is intentionally thin:
- no second semantic model,
- no transport logic,
- no CLI parsing,
- no detached editor-only file identity outside the real project model.

## Implemented V1 Surface

The first direct host session lives in:
- `oxvba-languageservice::HostWorkspaceSession`

Supporting public types:
- `oxvba-languageservice::HostWorkspaceDocument`
- `oxvba-languageservice::HostSessionError`

This session is layered over:
- `oxvba-project::load_workspace_target`
- `oxvba-languageservice::LanguageService`

## Ownership Split

OxVba owns:
- workspace target loading and discovery,
- project-model truth,
- document identity tied to the loaded workspace,
- semantic snapshots and queries,
- restore-to-baseline behavior for project-backed documents.

OxIde owns:
- `ProjectSession` orchestration,
- `DocumentSession` orchestration,
- editor state and text presentation,
- UI workflow and command routing.

## Current V1 Operations

### Workspace lifetime

- `HostWorkspaceSession::load_workspace_path`
- `HostWorkspaceSession::reload_workspace`
- `HostWorkspaceSession::workspace_target`
- `HostWorkspaceSession::workspace_stats`
- `HostWorkspaceSession::workspace_roster`
- `HostWorkspaceSession::documents`

### Document lifetime

- `HostWorkspaceSession::document_source`
- `HostWorkspaceSession::set_document_text`
- `HostWorkspaceSession::close_document`

Current close semantics are deliberate:
- project-backed documents restore to their loaded baseline source,
- the facade does not fabricate detached documents outside the loaded workspace.

### Query surface

The current V1 roster surface includes:
- stable direct-host workspace/project/module/document ID DTOs
- selected source policy
- snapshot revision
- module include/source paths
- logical module names
- `Attribute VB_Name` reconciliation state
- document versions and overlay flags

The current V1 query surface includes:
- diagnostics
- document symbols
- workspace symbols
- completions
- hover
- go-to-definition
- find-references
- semantic provenance

## Error Behavior

Errors are currently:
- typed,
- deterministic,
- and explicit about workspace load failure or unknown document identity.

The facade does not:
- invent shadow documents,
- silently swap workspaces,
- or hide load failures behind empty results.

## Immediate OxIde Consumption Model

OxIde should use this facade as follows:

1. `ProjectSession` loads one `HostWorkspaceSession` for the active OxVba target.
2. `DocumentSession` keeps the current editor text.
3. On editor changes, OxIde calls `set_document_text`.
4. For diagnostics and semantic queries, OxIde calls the session directly.
5. On document close/revert, OxIde calls `close_document` to restore project-backed baseline source.

This replaces the need to build an editor-facing semantic workflow around CLI calls.

## Known Next Expansions

The next OxVba-side additions are expected to include:
- broader typed project-authoring helpers for OxIde flows,
- typed embedded build/run requests and results,
- potentially a slightly richer direct session/query facade if OxIde needs more orchestration help.

Build/run design reference:
- `docs/spec/OXVBA_EMBEDDED_BUILD_RUN_CONTRACT_V1.md`

Those are follow-on steps.
The current V1 contract is specifically the first direct editor/session seam.

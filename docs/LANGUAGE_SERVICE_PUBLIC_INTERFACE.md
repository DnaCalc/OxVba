# Language Service Public Interface

This document defines the intended public OxVba-side interface for editor hosts.

The key split is:
- direct-embed hosts such as OxIde should consume the Rust API directly,
- VS Code-class hosts should consume `oxvba-lsp` plus extension commands,
- project-authoring semantics remain canonical in OxVba rather than being re-invented by hosts.

## Public Host Surface Today

Today the real direct host surface is spread across two crates:

### `oxvba-languageservice`

This crate currently exports the direct semantic/query ladder:
- workspace and document identity,
- a direct host-facing workspace/document session via `HostWorkspaceSession`,
- diagnostics,
- document and workspace symbols,
- semantic classification,
- completions,
- signature help,
- hover,
- go-to-definition,
- references,
- rename preparation,
- safe reference-update analysis,
- bounded diagnostics-driven code-action planning.

This is the right crate for:
- host-facing workspace/document overlay sessions,
- live editor overlays,
- semantic queries over in-memory source text,
- project-aware language features.

### `oxvba-project`

This crate currently exports the direct project/helper ladder:
- canonical workspace target loading,
- project discovery/loading policy,
- module scaffolding and typed project-edit intents in `host_helpers`,
- logical module-name inspection,
- `Attribute VB_Name` reconciliation,
- module/reference authoring helper plans.

This is the right crate for:
- project load/open flows,
- module/class/reference authoring,
- file-name versus logical-name reconciliation,
- project-model truth that should not be reimplemented in the IDE.

## Public Host Surface Policy

For direct-embed hosts:
- OxVba should expose typed request/response Rust APIs,
- no CLI string parsing should be required for core editor scenarios,
- no LSP transport should be required for OxIde-class hosts,
- document identity should remain tied to the real project/workspace model.

For VS Code-class hosts:
- `oxvba-lsp` should remain a thin transport over the same direct semantics,
- project creation and project editing should live in extension commands or CLI/direct-helper flows,
- LSP should not become a second project-authoring model.

## What OxIde Needs Next From OxVba

OxIde already has the right application seams:
- `ProjectSession`,
- `DocumentSession`,
- an `OxVbaServices` seam,
- and an editor surface that should stay separate from project/language-service logic.

The current gap is that OxIde still shells out to the CLI for build/run and does not yet consume a first-class direct editor host API.

The next OxVba-side improvements should therefore be:

1. A clearer direct host session surface
- load/reload a workspace once,
- open/update/close documents by document identity,
- query diagnostics/symbols/completion/hover/etc. without the host manually wiring lower-level pieces each time.

Status:
- the first bounded version of this now exists as `oxvba_languageservice::HostWorkspaceSession`

2. A broader direct project-authoring surface
- inspect project/module rosters,
- create/add/remove modules and classes,
- list/add/remove project and COM references,
- reconcile file path, logical module name, and `Attribute VB_Name` deterministically.

3. Typed build/run integration
- keep CLI as an end-user tool,
- but expose typed build/run requests/results suitable for an embedded IDE host.

4. Stable host-facing guidance
- document which crates/types are intended for direct host use,
- keep transport-only types out of that public story,
- keep UI/editor widget concerns out of OxVba crates.

## Recommended Consumption Pattern

### OxIde and other direct-embed hosts

Use:
- `oxvba_project::load_workspace_target`
- `oxvba_project::host_helpers::*`
- `oxvba_languageservice::LanguageService`
- `oxvba_languageservice::HostWorkspaceSession`
- the direct query/result types re-exported by `oxvba-languageservice`

Prefer:
- host-owned project/session orchestration,
- host-owned document/session orchestration,
- OxVba-owned project, semantic, and build/runtime semantics.

Avoid:
- shelling out for semantic features,
- inventing host-local project naming rules,
- bypassing the real project model with detached editor-only files.

### VS Code extension hosts

Use:
- `oxvba-lsp` for text synchronization and protocol-exposed language features,
- extension commands for project creation and project-edit flows,
- the same OxVba helper semantics as the source of truth for authoring actions.

Do not use:
- LSP as the only project-authoring contract,
- extension-local heuristics for module naming or reference semantics.

## Near-Term Execution Direction

The next execution lane should:
- tighten the documented direct host surface around OxIde,
- add any missing typed host-facing session APIs on the OxVba side,
- and treat VS Code as an alternate integration lane over the same semantics rather than the primary editor model.

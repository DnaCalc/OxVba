# Language Service Host Boundaries

> [!CAUTION]
> **Historical deleted-stack guidance.** Use [`spec/OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md`](spec/OXVBA_LANGUAGE_SERVICE_ARCHITECTURE_V1.md) and the current language-service workset.

This document describes how OxVba language services should be consumed by different host classes.

The governing rule is:
- `oxvba-languageservice` and `oxvba-project` own semantic and project-authoring behavior,
- `oxvba-lsp` owns transport and protocol behavior,
- editor hosts should not invent a second project model.

## Host Classes

### VS Code-class LSP hosts

These hosts want:
- an LSP server process,
- protocol-shaped diagnostics and semantic queries,
- text synchronization,
- workspace/session lifetime handling,
- and editor commands outside LSP for project creation and authoring flows.

For these hosts:
- use `oxvba-lsp` for document/session synchronization and protocol-exposed language features,
- use OxVba CLI commands or direct host-helper APIs for project/module/reference authoring,
- do not push project-authoring semantics down into ad hoc LSP-only heuristics.

Current OxVba status:
- `oxvba-lsp` provides bootstrap, single-root workspace loading, full-text sync,
  diagnostics publishing, document/workspace symbols, hover, definition,
  references, completion, signature help, rename preparation, code actions, and
  semantic tokens,
- the direct semantic/query ladder remains the source of truth and `oxvba-lsp`
  is only a projection of the supported transport subset,
- project-authoring flows should stay outside LSP for now.

### Direct-embed hosts

Examples:
- OxIde,
- custom Rust hosts that embed OxVba directly.

These hosts should prefer the direct API:
- `oxvba-languageservice` for diagnostics, symbols, semantic queries, rename/code-action planning, and workspace overlays,
- `oxvba-project` for canonical project loading and host project-helper operations.

These hosts do not need LSP unless they specifically want protocol interoperability.

Current OxIde reality:
- OxIde already models explicit `ProjectSession` and `DocumentSession` seams,
- OxVba now exposes direct workspace, semantic, project-helper, embedded
  build/run, immediate, and debugger substrates for those seams,
- the remaining gap is real OxIde-side adoption evidence, not routing OxIde
  through LSP.

## Responsibility Split

### Direct API responsibilities

The direct API owns:
- workspace identity,
- document identity,
- project loading,
- semantic snapshots,
- diagnostics and semantic queries,
- rename/reference/code-action planning,
- host project-helper operations.

For OxIde-class hosts, the intended public story is:
- direct typed Rust APIs for workspace/document/session behavior,
- direct typed Rust APIs for project/module/reference authoring,
- no required dependence on CLI output parsing for editor scenarios.

Current implemented direct session anchor:
- `oxvba_languageservice::HostWorkspaceSession`
- this wraps canonical workspace loading plus project-backed document overlay/restore behavior

Current host-helper operations live in `oxvba-project::host_helpers` and cover:
- planned `.bas` / `.cls` module creation,
- typed project-edit intents for modules and references,
- validated project-edit planning and `.basproj` apply flow,
- typed workspace-target roster/reference inspection,
- typed COM candidate and active-selection assessment,
- logical module-name inspection,
- `Attribute VB_Name` redundancy/requirement detection,
- file-name versus logical-name reconciliation.

### LSP transport responsibilities

`oxvba-lsp` owns:
- JSON-RPC/LSP process lifetime,
- initialize/shutdown behavior,
- single-root workspace session selection,
- full-text `didOpen` / `didChange` / `didClose` synchronization,
- protocol projection for diagnostics, document/workspace symbols, hover,
  definition, references, completion, signature help, prepare-rename, code
  actions, and semantic tokens,
- protocol-shaped errors and capability advertisement.

`oxvba-lsp` must not own:
- a second parser,
- a second semantic model,
- transport-local project-discovery policy,
- shadow documents that bypass the real project model,
- editor-local module naming rules.

## Project Creation And Authoring

Project creation is not an LSP responsibility.

The intended model is:
1. the host creates or updates project files through OxVba CLI or direct helper APIs,
2. the host writes or updates module/class source through OxVba helper plans,
3. the language-service workspace reloads or synchronizes against that real project state,
4. LSP, if present, only reflects the resulting workspace/session state.

For a VS Code-style extension, that usually means:
- command palette / UI command creates the project,
- command palette / UI command adds modules or references,
- `oxvba-lsp` handles the editing session afterward.

For a direct-embed host, that usually means:
- call `oxvba_project::load_workspace_target`,
- call `oxvba_project::plan_new_module` / `reconcile_module_identity` / typed edit helpers,
- apply file/project changes,
- refresh the in-process language-service workspace.

Near-term OxIde-driven additions are now mostly consumer proof:
- keep the public host-facing session API aligned with OxIde needs,
- add any missing typed project-authoring operations only when concrete host
  flows expose a gap,
- capture evidence when OxIde consumes typed build/run and language-service
  requests directly.

The intended design for that remaining build/run area now lives in:
- `docs/spec/OXVBA_EMBEDDED_BUILD_RUN_CONTRACT_V1.md`

Current status:
- the first bounded host-facing session API is now implemented,
- typed project roster/reference inspection is now implemented,
- validated project-edit planning and apply is now implemented for `.basproj` workspaces,
- the first bounded embedded build/run surface is now implemented via `oxvba_host::embedded`.

Current embedded build/run anchor:
- `oxvba_host::EmbeddedBuildRunHost`
- `oxvba_host::EmbeddedRunSession`
- `oxvba_languageservice::HostWorkspaceSession::prepare_embedded_workspace_snapshot(...)`

Current OxIde execution guidance for that surface:
- `docs/OXIDE_EMBEDDED_BUILD_RUN_INTEGRATION_GUIDANCE.md`

## Current Honest Boundary

OxVba can currently claim:
- a first-class direct language-service core,
- a thin and honest LSP transport shell,
- shared project-loading policy outside the transport,
- explicit host-facing project-helper APIs.

OxVba should now start claiming more explicitly:
- OxIde is the intended showcase direct-embed host,
- the direct public host surface is the primary editor integration contract,
- VS Code is an alternate integration path over the same semantics.

OxVba does not yet claim:
- full LSP parity with the direct API,
- a complete editor extension package for VS Code,
- multi-root transport semantics,
- host-authoring flows exposed over LSP methods.

## V0.2 Host Rules

OxIde/direct-embed hosts:
- consume `HostWorkspaceSession`, `oxvba-project` helper plans, and
  `EmbeddedBuildRunHost` directly,
- keep document overlays synchronized into OxVba rather than reconstructing
  compiler input from buffers,
- avoid CLI parsing for semantic, project, build/run, immediate, or debugger
  behavior.

VS Code-class hosts:
- consume `oxvba-lsp` for language features,
- keep project creation/module/reference authoring in commands outside LSP,
- treat debugging as a later DAP projection over the OxVba debugger core,
- do not treat the current single-root LSP shell as multi-root or full IDE
  parity.

## Practical Guidance

If you are building a VS Code-class host:
- treat LSP as the language-feature transport,
- keep project creation and project editing in extension commands or CLI-backed flows,
- use the direct project-helper semantics as the source of truth.

If you are building OxIde or another direct-embed host:
- embed `oxvba-languageservice` and `oxvba-project` directly,
- use `oxvba-lsp` only if you explicitly need protocol interoperability,
- and prefer a typed OxVba-side host service seam over shelling out to the CLI for editor-facing behavior.

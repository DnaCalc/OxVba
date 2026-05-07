# Language Service Public Interface

This document defines the intended public OxVba-side interface for editor hosts.

The key split is:
- direct-embed hosts such as OxIde should consume the Rust API directly,
- VS Code-class hosts should consume `oxvba-lsp` plus extension commands,
- project-authoring semantics remain canonical in OxVba rather than being re-invented by hosts.

## Public Host Surface Today

Today the real direct host surface is spread across three crates:

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
- typed workspace-target roster/reference inspection via `inspect_workspace_target`,
- typed COM reference discovery and active-selection models via `com_selection`,
- registered-library and ProgID-backed COM candidate discovery helpers,
- file-backed typelib carrier discovery helpers,
- deterministic COM add/replace/repair/remove planning helpers,
- a direct OxIde-facing COM selection service and project-state surface,
- project discovery/loading policy,
- module scaffolding and typed project-edit intents in `host_helpers`,
- logical module-name inspection,
- `Attribute VB_Name` reconciliation,
- module/reference authoring helper plans.

This is the right crate for:
- project load/open flows,
- typed project roster/reference panes in a direct-embed IDE,
- module/class/reference authoring,
- file-name versus logical-name reconciliation,
- project-model truth that should not be reimplemented in the IDE.

### `oxvba-host`

This crate currently exports the direct runtime/session ladder:
- `Engine`
- `ProjectRuntimeSession`
- shared direct-host identity, capability, command-status, source-span, and issue DTOs via the `DirectHost*` type family
- the first bounded immediate-session contract via `ImmediateSession`
- typed immediate request/result/output shapes for future CLI and OxIde consumption
- the first non-debug live-session evaluator core for bounded procedure invocation and reset/reload
- the first direct debug-session substrate for paused runtime inspection and stepping

This is the right crate for:
- live runtime-session ownership,
- the low-level runtime substrate that a future typed embedded build/run facade will sit over,
- Immediate Window / REPL session semantics,
- later debugger and paused-context runtime session surfaces.

## Public Host Surface Policy

For direct-embed hosts:
- OxVba should expose typed request/response Rust APIs,
- no CLI string parsing should be required for core editor scenarios,
- no LSP transport should be required for OxIde-class hosts,
- document identity should remain tied to the real project/workspace model.

For VS Code-class hosts:
- `oxvba-lsp` remains a thin transport over the same direct semantics,
- protocol methods may expose diagnostics, symbols, hover, navigation,
  completion, signature help, rename preparation, code actions, and semantic
  tokens,
- project creation and project editing live in extension commands or
  CLI/direct-helper flows,
- LSP must not become a second project-authoring model.

## What OxIde Needs Next From OxVba

OxIde already has the right application seams:
- `ProjectSession`,
- `DocumentSession`,
- an `OxVbaServices` seam,
- and an editor surface that should stay separate from project/language-service logic.

The current gap is external adoption evidence: OxVba now exposes the first
bounded direct editor host API and embedded build/run substrate, but the OxIde
repo still needs to consume that stack before an end-to-end OxIde showcase can
be claimed.

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
- list/add/remove/reorder project and COM references,
- reconcile file path, logical module name, and `Attribute VB_Name` deterministically.

Status:
- typed workspace-target roster/reference inspection now exists via `oxvba_project::inspect_workspace_target`
- typed COM candidate and active-selection models now exist via `oxvba_project::com_selection`
- registered-library and ProgID candidate discovery now exist via `discover_registered_com_candidates` and `discover_prog_id_com_candidates`
- file-backed candidate discovery now exists via `discover_file_backed_com_candidates`
- typed active-selection repair and reorder planning now exists via `assess_project_com_selections`, `plan_*_com_*` helpers, and `plan_reorder_com_references`
- COM capability profile and runtime availability DTOs now exist via `com_capability_profile` and `com_runtime_invocation_availability`
- a direct OxIde-facing COM helper surface now exists via `ComSelectionService`, `inspect_workspace_com_project_state`, `capability_profile`, `runtime_invocation_availability`, and `plan_reorder_references`
- direct project compile/options DTOs now exist via `inspect_workspace_compile_options` and `HostProjectCompileOptionsSurface`
- canonical `.basproj` mutation/apply flows now exist via `apply_host_project_edits_to_basproj` and `apply_host_project_edits_to_basproj_path`
- validated project roster/reference edit planning and apply flow now exists via `prepare_host_project_edit_plan`, `validate_host_project_edits`, and `apply_host_project_edit_plan`
- validated scalar project/compile settings planning and apply flow now exists via `prepare_host_project_settings_edit_plan`, `validate_host_project_settings_edits`, and `apply_host_project_settings_edit_plan`

3. Typed build/run integration
- keep CLI as an end-user tool,
- but expose typed build/run requests/results suitable for an embedded IDE host.

Design status:
- the intended shape is now captured in `docs/spec/OXVBA_EMBEDDED_BUILD_RUN_CONTRACT_V1.md`
- the chosen direction is an explicit source-policy model so hosts choose between on-disk state and workspace-overlay state honestly
- the owning contract module and type family now live in `oxvba_host::embedded`
- the executable source-policy handoff now exists via `HostWorkspaceSession::prepare_embedded_workspace_snapshot`
- the first typed `build_workspace` substrate now exists via `EmbeddedBuildRunHost::build_workspace`
- the first typed live runtime-session substrate now exists via:
  - `EmbeddedBuildRunHost::run_project`
  - `EmbeddedRunSession::reset_runtime`
  - `EmbeddedRunSession::invoke_entry_point`
  - `EmbeddedRunSession::invoke_procedure`
- OxIde-facing host-consumption guidance is now published in
  `docs/OXIDE_EMBEDDED_BUILD_RUN_INTEGRATION_GUIDANCE.md`

4. Stable host-facing guidance
- document which crates/types are intended for direct host use,
- keep transport-only types out of that public story,
- keep UI/editor widget concerns out of OxVba crates.

## Recommended Consumption Pattern

### OxIde and other direct-embed hosts

Use:
- `oxvba_project::load_workspace_target`
- `oxvba_project::host_helpers::*`
- `oxvba_project::inspect_workspace_target`
- `oxvba_project::prepare_host_project_edit_plan`
- `oxvba_project::apply_host_project_edit_plan`
- `oxvba_project::inspect_workspace_compile_options`
- `oxvba_project::prepare_host_project_settings_edit_plan`
- `oxvba_project::apply_host_project_settings_edit_plan`
- `oxvba_project::assess_project_com_selections`
- `oxvba_host::Engine`
- `oxvba_host::EmbeddedBuildRunHost`
- `oxvba_host::EmbeddedRunSession`
- `oxvba_host::EmbeddedWorkspaceSnapshot`
- `oxvba_host::ImmediateSession`
- `oxvba_host::DebugSession`
- `oxvba_languageservice::LanguageService`
- `oxvba_languageservice::HostWorkspaceSession`
- the direct query/result types re-exported by `oxvba-languageservice`

Prefer:
- host-owned project/session orchestration,
- host-owned document/session orchestration,
- OxVba-owned project, semantic, and build/runtime semantics.
- validated plan/apply authoring over direct helper APIs instead of host-side `.basproj` XML mutation.

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

## Host Consumption Map

For OxIde-class hosts, the V0.2 consumption path is:
- load the workspace through `oxvba-project`,
- keep editor overlays in `HostWorkspaceSession`,
- query semantics through `oxvba-languageservice`,
- apply project/module/reference edits through validated `oxvba-project`
  helper plans,
- build/run through `EmbeddedBuildRunHost` and keep live runtime ownership in
  `EmbeddedRunSession`,
- keep Immediate Window and debugger UI over the same runtime session.

For VS Code-class hosts, the V0.2 consumption path is:
- start `oxvba-lsp` as the language-feature transport,
- use one loaded workspace per LSP session,
- rely on LSP for synchronization, diagnostics, symbols, hover, definition,
  references, completion, signature help, rename preparation, code actions, and
  semantic tokens,
- implement project creation/module/reference authoring as extension commands
  backed by OxVba CLI or direct helper semantics,
- use a later DAP projection for debugging rather than folding debugger
  ownership into `oxvba-lsp`.

Unsupported in V0.2:
- LSP-owned project authoring,
- multi-root LSP semantics,
- full VS Code extension completion,
- designer/forms editing,
- complete refactoring parity.

## Near-Term Execution Direction

The next execution lane should:
- keep the documented direct host surface around OxIde aligned with executable
  evidence,
- execute the DNA OxIde full-scope host integration support workset in
  `docs/worksets/WORKSET_2026-05-07_DNAOXIDE_FULL_SCOPE_HOST_INTEGRATION_SUPPORT.md`,
- capture real OxIde-side consumption evidence when that repo adopts the direct
  stack,
- and treat VS Code as an alternate integration lane over the same semantics
  rather than the primary editor model.

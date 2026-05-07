# Workset: DNA OxIde Full-Scope OxVba Host Integration Support

Date: 2026-05-07
Status: in-progress
Source handoff: `../OxIde/docs/HANDOFF_DNAOXIDE_OXVBA_REQUIREMENTS.md`
Parent bead: `bd-avdu`

## Purpose

Process the DNA OxIde requirements handoff and turn the remaining OxVba-side
support into an executable workset for direct desktop IDE development tooling.

The immediate product pressure from DnaOxIde is a Windows desktop IDE that can:
open real `.basproj` workspaces, edit with OxVba semantics, build/check through
typed APIs, manage references and COM type libraries, run a live OxVba runtime
session, use Immediate Window, debug with watches/breakpoints/source mapping,
and expose unavailable capability states honestly.

## Boundary

OxVba owns project, language-service, build/run, runtime, Immediate, debug,
watch, breakpoint, COM/reference, capability/error taxonomy, and source mapping
truth. DnaOxIde/OxIde owns UI layout, pane rendering, command routing, Tauri
shell wiring, transcript/history UX, and no-claim presentation when services are
unavailable.

This workset does not claim that OxIde has already adopted all surfaces. It
records what OxVba exposes now and tracks the OxVba gaps that must be delivered
before DnaOxIde can flip full native IDE capability claims.

## Status Terms

- `available`: a checked-in direct Rust surface exists and is covered by local
  evidence for the named subset.
- `available-subset`: a useful direct surface exists, but the handoff requires
  more DTO shape, identity, status, or evidence before the requirement is
  satisfied for DnaOxIde.
- `planned`: no adequate direct OxVba-side contract exists yet; delivery beads
  below own the work.

## Current Available OxVba Host Surfaces

### Workspace and editor semantics

Available direct surfaces:

- `oxvba_languageservice::HostWorkspaceSession`
- `oxvba_languageservice::HostWorkspaceDocument`
- `oxvba_languageservice::HostSessionError`
- `oxvba_languageservice::LanguageService`
- `oxvba_languageservice::LanguageServiceProvider`

Current details:

- `HostWorkspaceSession::load_workspace_path`, `reload_workspace`,
  `workspace_target`, `workspace_stats`, `workspace_roster`, and `documents`
  load a real OxVba workspace/project target.
- `document_source`, `set_document_text`, and `close_document` support direct
  project-backed editor overlay flows.
- `workspace_roster` exposes one direct-host project/module roster DTO with
  workspace/project/module/document IDs, selected source policy, snapshot
  revision, source paths, logical module names, `Attribute VB_Name`
  reconciliation state, document versions, and overlay flags.
- `project_manifest_snapshot` and `prepare_embedded_workspace_snapshot` provide
  the explicit source-policy handoff to build/run (`DiskOnly` vs
  `WorkspaceOverlay`).
- The session facade exposes diagnostics, document symbols, workspace symbols,
  completions, hover, go-to-definition, references, and semantic provenance.
- The lower-level `LanguageServiceProvider` surface also includes semantic
  classifications, signature help, rename preparation, reference-update
  analysis, and diagnostics-driven code-action planning.

Evidence anchors:

- `crates/oxvba-languageservice/src/host_session.rs`
- `cargo test -p oxvba-languageservice host_session_workspace_roster_reports_identity_and_overlay_revision --quiet`
- `docs/spec/OXIDE_DIRECT_HOST_SESSION_FACADE_V1.md`
- `docs/evidence/v0_2/V02_LANGUAGE_SERVICE_DIRECT_API_TESTS_2026-04-27.md`

### Project authoring and references

Available direct surfaces:

- `oxvba_project::load_workspace_target`
- `oxvba_project::inspect_workspace_target`
- `oxvba_project::host_helpers::*`
- `oxvba_project::prepare_host_project_edit_plan`
- `oxvba_project::validate_host_project_edits`
- `oxvba_project::apply_host_project_edit_plan`
- `oxvba_project::com_selection::*`
- `oxvba_project::ComSelectionService`
- `oxvba_project::ComCapabilityProfile`
- `oxvba_project::ComRuntimeInvocationAvailability`
- `oxvba_project::ComReferenceReorderPlan`

Current details:

- `HostProjectSurface` exposes workspace kind, target, project file/dir, project
  name, output type, module roster, and reference roster.
- `HostProjectModuleInfo` carries module kind, include path, source path, and
  `ModuleIdentityInfo` with file stem, declared `Attribute VB_Name`, effective
  name, and attribute reconciliation flags.
- `plan_new_module` covers module and class-module scaffolding through
  `BasProjModuleKind`.
- `HostProjectEdit` and plan/apply helpers cover add/remove module, project
  reference, and COM reference flows for `.basproj` targets.
- `HostProjectCompileOptionsSurface` exposes project properties, build target,
  runtime flavor, entry point/run target, default profile/policy/root object,
  conditional constants, build profile, source policy, and build/check command
  status DTOs.
- `HostProjectSettingsEdit` plus prepare/validate/apply helpers cover validated
  scalar `.basproj` settings edits and preview surfaces.
- `ComSelectionService` covers registered, ProgID-backed, and file-backed COM
  candidate discovery, active project COM selection assessment, add/repair/
  replace/remove/reorder plan generation, COM capability profile DTOs, and COM
  runtime invocation availability DTOs.

Evidence anchors:

- `crates/oxvba-project/src/host_helpers.rs`
- `cargo test -p oxvba-project compile_options --quiet`
- `cargo test -p oxvba-project project_settings_edit_plan --quiet`
- `crates/oxvba-project/src/com_selection.rs`
- `cargo test -p oxvba-project com_capability_profile_reports_platform_specific_runtime_availability --quiet`
- `cargo test -p oxvba-project plan_reorder_com_references_rewrites_only_valid_complete_orders --quiet`
- `docs/spec/COM_REFERENCE_SELECTION_SERVICE_V1.md`
- `docs/worksets/WORKSET_2026-04-02_COM_REFERENCE_SELECTION_SERVICE_AND_HOST_HELPERS.md`

### Shared identity, capability, command-status, and issue DTOs

Available direct surfaces:

- `oxvba_host::DirectHostWorkspaceId`
- `oxvba_host::DirectHostProjectId`
- `oxvba_host::DirectHostDocumentId`
- `oxvba_host::DirectHostBuildRequestId`
- `oxvba_host::DirectHostRuntimeSessionId`
- `oxvba_host::DirectHostImmediateSessionId`
- `oxvba_host::DirectHostDebugSessionId`
- `oxvba_host::DirectHostBreakpointId`
- `oxvba_host::DirectHostStackFrameId`
- `oxvba_host::DirectHostWatchId`
- `oxvba_host::DirectHostIssueKind`
- `oxvba_host::DirectHostIssue`
- `oxvba_host::DirectHostCommandStatus`
- `oxvba_host::DirectHostCapabilityKind`
- `oxvba_host::DirectHostCapabilityStatus`
- `oxvba_host::DirectHostCapability`
- `oxvba_host::DirectHostSourceSpan`

Current details:

- R9 handoff categories now have stable `DH-*` codes in
  `DirectHostIssueKind::stable_code`.
- Disabled command states can carry typed `DirectHostIssue` reasons with
  retryability and optional workspace/project/document/session/source context.
- Current errors from `HostSessionError`, `PhaseDiagnostic`,
  `EmbeddedRunSessionError`, `ImmediateSessionError`, and `DebugSessionError`
  now project into the shared issue DTO shape.

Evidence anchors:

- `crates/oxvba-host/src/direct_host.rs`
- `crates/oxvba-host/src/engine.rs`
- `crates/oxvba-host/src/embedded.rs`
- `crates/oxvba-host/src/immediate.rs`
- `crates/oxvba-host/src/debugger.rs`
- `crates/oxvba-languageservice/src/host_session.rs`
- `cargo test -p oxvba-host direct_host --quiet`
- `cargo test -p oxvba-languageservice host_session_rejects_documents_outside_loaded_workspace --quiet`

Remaining scope:

- Broaden use of the shared DTOs through project/COM/build/run response shapes.
- Add concrete runtime/debug/watch/breakpoint IDs when those contracts harden in
  `bd-avdu.3.1`, `bd-avdu.4.1`, and `bd-avdu.4.2`.

### Embedded build/run and runtime sessions

Available direct surfaces:

- `oxvba_host::EmbeddedWorkspaceInput`
- `oxvba_host::EmbeddedWorkspaceSnapshot`
- `oxvba_host::EmbeddedBuildRunHost`
- `oxvba_host::EmbeddedBuildRequest`
- `oxvba_host::EmbeddedRunRequest`
- `oxvba_host::EmbeddedBuildRunHostCommandStatus`
- `oxvba_host::EmbeddedRunSession`
- `oxvba_host::EmbeddedRunSessionCommandStatus`
- `oxvba_host::EmbeddedResetRequest`
- `oxvba_host::EmbeddedInvokeEntryPointRequest`
- `oxvba_host::EmbeddedInvokeProcedureVariantRequest`

Current details:

- Source policy is explicit: `DiskOnly` or `WorkspaceOverlay`.
- `EmbeddedBuildRequest` and `EmbeddedRunRequest` carry direct-host request IDs
  and support caller-supplied IDs.
- `EmbeddedBuildRunHost::build_workspace` returns typed build status,
  request ID, and compile diagnostics.
- `EmbeddedBuildRunHost::build_workspace_with_events` emits ID-bearing
  `BuildStarted` plus terminal build lifecycle events.
- `EmbeddedBuildRunHost::run_project` returns a live `EmbeddedRunSession` with
  a stable runtime session ID or a typed failed run result.
- `EmbeddedBuildRunHost::run_project_with_events` emits ID-bearing `RunStarted`
  plus `SessionReady` or failed run lifecycle events.
- `EmbeddedRunSession` exposes command availability, runtime reset,
  entry-point invocation, and bounded procedure invocation with retained
  `Variant` arguments/return values.
- Stop/cancel is explicitly disabled with a typed direct-host reason until a
  real cancellation path is implemented.

Evidence anchors:

- `crates/oxvba-host/src/embedded.rs`
- `cargo test -p oxvba-host embedded_build_run_ids_events_and_command_status_are_correlated --quiet`
- `crates/oxvba-languageservice/src/host_session.rs`
- `docs/OXIDE_EMBEDDED_BUILD_RUN_INTEGRATION_GUIDANCE.md`
- `docs/worksets/WORKSET_2026-04-03_EMBEDDED_BUILD_RUN_DIRECT_HOST_EXECUTION.md`

### Immediate and debug

Available direct surfaces:

- `oxvba_host::ImmediateSession`
- `oxvba_host::ImmediateEvaluationRequest`
- `oxvba_host::ImmediateVariantEvaluationResult`
- `oxvba_host::ImmediateSessionCommandStatus`
- `oxvba_host::DebugSession`
- `oxvba_host::DebugEvaluationRequest`
- `oxvba_host::DebugSessionCommandStatus`
- `oxvba_host::DebugBreakpointRecord`
- `oxvba_host::DebugBreakpointBindingStatus`
- `oxvba_host::DebugBreakpointUnresolvedReason`
- `oxvba_host::DebugWatchRecord`
- `oxvba_host::DebugWatchEvaluation`
- `oxvba_host::DebugWatchEvaluationStatus`
- `oxvba_host::DebugVariantPauseState`
- `oxvba_host::HostDebugVariantRunResult`

Current details:

- `ImmediateSession` is live-runtime-backed, carries a stable
  `immediate_session_id`, can carry the current `runtime_session_id`, and
  supports reset, default target module selection, command availability,
  retained `Variant` value output, printed-line output, empty output, and
  diagnostics.
- Current immediate evaluation is bounded to existing procedure invocation and
  literal arguments; arbitrary ad hoc expression compilation remains future
  work.
- `EmbeddedRunSession::into_immediate_session` and `into_debug_session` provide
  consuming direct attach/create paths from the active runtime session while
  preserving runtime-session correlation IDs.
- `DebugSession` carries a stable `debug_session_id`, can carry the current
  `runtime_session_id`, and supports VM-backed start, continue, step
  into/over/out, command availability, stable frame IDs, debugger-owned watch
  records/evaluation statuses, source breakpoint records with bind/unresolved
  state, VM breakpoint registration, paused frame/local projection, and bounded
  current-frame identifier evaluation.
- Current debug frame source projection is procedure/range oriented rather than
  a full editor span DTO for every stop/breakpoint state; breakpoint hit counts
  are tracked when the VM reports a breakpoint stop.

Evidence anchors:

- `crates/oxvba-host/src/immediate.rs`
- `crates/oxvba-host/src/debugger.rs`
- `crates/oxvba-host/tests/oxide_direct_host_consumption.rs`
- `cargo test -p oxvba-host embedded_run_session_attaches_immediate_and_debug_with_stable_ids --quiet`
- `cargo test -p oxvba-host debug_session_watch_registry_reports_unavailable_error_and_value_states --quiet`
- `cargo test -p oxvba-host debug_session_breakpoint_records_bind_disable_clear_and_count_hits --quiet`
- `docs/OXIDE_IMMEDIATE_WINDOW_INTEGRATION_GUIDANCE.md`
- `docs/evidence/OXIDE_DIRECT_IMMEDIATE_DEBUG_SEAMS_2026-04-28.md`

## Requirement Matrix From DNA OxIde Handoff

| Requirement | Current OxVba status | Available details | Remaining OxVba work |
| --- | --- | --- | --- |
| R1 workspace/project/document identity | available-subset | `HostWorkspaceSession` loads/reloads a target, exposes `DocumentId`, document overlays, baseline close, snapshot extraction, source policy, and `HostWorkspaceRoster` with workspace/project/module/document IDs, module paths, logical names, `Attribute VB_Name` state, snapshot revision, document versions, and overlay flags. Shared direct-host ID newtypes exist in `oxvba-host`. | Remaining hardening is broader source-span correlation across runtime/debug/breakpoint DTOs. Beads: `bd-avdu.3.1`, `bd-avdu.4.1`, `bd-avdu.4.2`. |
| R2 language service editing | available-subset | Direct APIs exist for diagnostics, symbols, completions, hover, definition, references; lower-level provider includes semantic classifications, signature help, rename prep, reference-update analysis, and code actions. | Lift the full language-service set through the host session/facade consistently and ensure all editor-facing outputs carry navigation-ready source spans and stable document IDs. Bead: `bd-avdu.2.2`. |
| R3 project authoring and compile options | available-subset | `HostProjectSurface`, `HostWorkspaceRoster`, `HostProjectCompileOptionsSurface`, module/class scaffolding, add/remove module/reference/COM plans, scalar settings edit plans, compile/build status, source-policy options, run target list, conditional constants, and validated apply flow exist for `.basproj`. | Remaining gaps are module rename/reorder and broader option policy breadth where DnaOxIde needs more than scalar `.basproj` settings. Follow-up can be added from OxIde consumption feedback. |
| R4 build/check contract | available-subset | `EmbeddedBuildRunHost::build_workspace` accepts `EmbeddedWorkspaceSnapshot`, honors explicit source policy, returns typed request ID/status/diagnostics, exposes build/run command status, and has `build_workspace_with_events` for ID-bearing lifecycle events. | Remaining gaps are warning-vs-error richness where diagnostics support it, invalid workspace taxonomy breadth outside prepared snapshots, and phase/timing labels. |
| R5 runtime/run session contract | available-subset | `run_project` creates a live `EmbeddedRunSession` with stable runtime session ID; `run_project_with_events` emits ID-bearing lifecycle events; reset, entry-point invoke, procedure invoke, and command availability exist with typed result status. Stop/cancel is typed disabled. | Remaining gaps are runtime error source-span projection and COM runtime availability status. Beads: `bd-avdu.4.1`, `bd-avdu.5.1`. |
| R6 Immediate Window contract | available-subset | `ImmediateSession` evaluates typed requests against a live runtime, carries stable immediate/runtime session IDs when attached from `EmbeddedRunSession`, exposes command availability, and returns typed value/printed/reset/empty outputs plus diagnostics. | Remaining gaps are no-session host-level disabled state and broader deterministic evaluation-failure taxonomy beyond the current bounded evaluator. |
| R7 debug/watches/breakpoints | available-subset | `DebugSession` carries stable debug/runtime session IDs when attached from `EmbeddedRunSession`; exposes start/continue/step controls, command availability, stable frame IDs, frame/local values, debugger-owned watch registry/evaluation statuses, source breakpoint records with bind/unresolved states, VM breakpoint registration, pause state, and bounded paused identifier evaluation. | Remaining gaps are broader source remapping/full editor source-span DTOs for every stop/breakpoint state and richer paused-context expression evaluation. |
| R8 references and Windows COM | available-subset | Active COM references, candidate discovery, missing/ambiguous/resolved state, add/repair/replace/remove/reorder plans, COM capability profile DTOs, and runtime invocation availability DTOs exist. Windows COM bridge exists in `oxvba-com`; non-Windows COM discovery/runtime are typed degraded/unavailable with `DH-COM-*` issue codes. | Remaining gaps are richer runtime bitness/apartment probing beyond declarative requirements and broader Office/environment-specific availability checks. |
| R9 capability and error taxonomy | available-subset | `DirectHostIssueKind` now covers the handoff categories with stable `DH-*` codes; `DirectHostIssue`, `DirectHostCommandStatus`, and `DirectHostCapability*` carry typed disabled/unavailable states with retryability and context. Current host/session/project/COM errors and availability surfaces project into this shape. | Continue applying the shared taxonomy to any future host DTOs introduced from OxIde consumption feedback. |
| R10 native service / sidecar boundary | planned | In-process Rust APIs are the current intended direct consumption path. HAL/profile docs state Windows COM support is active and non-Windows COM unsupported. | Document Tauri-safe in-process calling/threading/cancellation constraints and define sidecar/version/capability handshake only if DnaOxIde needs an out-of-process service. Bead: `bd-avdu.5.1`. |
| R11 test fixtures and evidence | available-subset | Existing tests prove host-session overlay snapshots, embedded build/run, direct Immediate, direct debug, COM selection, and no CLI/LSP immediate/debug proof. The DnaOxIde ThinSliceHello fixture ladder now covers overlay/build/run/Immediate/debug/watch/breakpoint and COM broken-reference/runtime-availability DTO paths over temp project copies. | Remaining evidence depends on OxIde-side direct consumption tests proving UI wiring avoids CLI/LSP fallback. Blocker: `BLK-OXIDE-DIRECT-CONSUMPTION-001`. |
| R12 minimal unblock sequence | available-subset | The W355/W360/W365/W370 backing surfaces all have first bounded OxVba substrates. | Harden the DTOs/evidence in the order below so OxIde claim flags flip only after matching tests. Beads: `bd-avdu.2.3`, `bd-avdu.5.1`, `bd-avdu.3.1`, `bd-avdu.4.1`, `bd-avdu.4.2`, `bd-avdu.6.1`. |

## Minimum Delivery Order For DnaOxIde

1. W355 compile/build UX:
   - project properties / compile options / run targets DTOs;
   - request IDs, command availability, and typed build/check results.
   - owning bead: `bd-avdu.3.1` (done for DTO/request/session/event basics; later beads own runtime/debug/COM-specific breadth).
2. W360 COM references:
   - COM/reference capability profile, active roster, candidate search, repair,
     unavailable/runtime status, and reference reorder support where needed.
   - owning bead: `bd-avdu.5.1` (done for `ComCapabilityProfile`,
     `ComRuntimeInvocationAvailability`, and `ComReferenceReorderPlan`).
3. W365 runtime + Immediate:
   - runtime session IDs/lifecycle events plus Immediate attach/session IDs and
     typed responses without fake data.
   - owning bead: `bd-avdu.4.1` (done for attach/session IDs and command status; `bd-avdu.4.2` owns watches/breakpoint DTO breadth).
4. W370 debug/watch/breakpoints:
   - debug attach/session IDs, command states, callstack/locals, watch registry,
     breakpoint binding/source mapping.
   - owning beads: `bd-avdu.4.1` and `bd-avdu.4.2` (done for stable frame/watch/breakpoint IDs, watch registry, watch evaluation states, and breakpoint bind/unresolved DTOs; remaining source-span breadth can follow OxIde consumption feedback).
5. Claim evidence:
   - ThinSliceHello and related fixture ladder proves each claim before OxIde
     flips real/native/COM/debug/Immediate capability flags.
   - owning bead: `bd-avdu.6.1` (done for OxVba-side temp-project fixture ladder; OxIde-side direct consumption remains blocked by `BLK-OXIDE-DIRECT-CONSUMPTION-001`).

## Bead Tree

Parent:

- `bd-avdu` - DNA OxIde full-scope OxVba host integration support

Epics:

- `bd-avdu.1` - DNA OxIde handoff audit and status alignment
- `bd-avdu.2` - workspace project identity and compile options DTOs
- `bd-avdu.3` - embedded build run and runtime session hardening
- `bd-avdu.4` - Immediate debug watch and breakpoint IDE contracts
- `bd-avdu.5` - references COM capability and native boundary profile
- `bd-avdu.6` - DNA OxIde fixture ladder and evidence gates

First executable beads:

- `bd-avdu.1.1` - publish DNA OxIde R1-R12 OxVba availability matrix
- `bd-avdu.2.1` - define shared direct-host identity capability and error DTOs (done; `DirectHost*` DTOs landed in `oxvba-host`)
- `bd-avdu.2.2` - unify workspace roster DTO with module paths logical names revisions and VB_Name status (done; `HostWorkspaceRoster` landed in `oxvba-languageservice`)
- `bd-avdu.2.3` - expose project compile options run targets and validated settings plans (done; `HostProjectCompileOptionsSurface` and settings edit plan/apply landed in `oxvba-project`)
- `bd-avdu.3.1` - add request IDs command availability and lifecycle events to embedded build/run (done; request/runtime IDs, command status, and `*_with_events` APIs landed in `oxvba-host`)
- `bd-avdu.4.1` - attach ImmediateSession and DebugSession to EmbeddedRunSession with stable IDs (done; consuming attach helpers and session command-status DTOs landed in `oxvba-host`)
- `bd-avdu.4.2` - implement watch registry and breakpoint binding DTOs for IDE panes (done; debugger-owned watch records/evaluations and source breakpoint binding DTOs landed in `oxvba-host`)
- `bd-avdu.5.1` - publish COM capability profile reference reorder and runtime availability DTOs (done; `ComCapabilityProfile`, `ComRuntimeInvocationAvailability`, and `ComReferenceReorderPlan` landed in `oxvba-project`)
- `bd-avdu.6.1` - build DNA OxIde ThinSliceHello fixture ladder (done; `crates/oxvba-languageservice/tests/dnaoxide_thin_slice_hello.rs` and evidence note landed)

## Exit Condition

This workset may be described as complete only when:

1. DnaOxIde can consume direct Rust DTOs/APIs for R1-R12 without CLI text parsing
   or LSP-internal routing for core semantics.
2. Each unavailable capability returns typed unavailable/disabled states rather
   than fake data.
3. Runtime, Immediate, debug, watch, breakpoint, and COM claims have matching
   fixture evidence, including
   `docs/evidence/DNAOXIDE_THIN_SLICE_HELLO_FIXTURE_2026-05-07.md`.
4. Windows-only and native-service/sidecar constraints are explicit and tested
   for unavailable paths.
5. Docs and evidence identify any residual subset boundaries without using full
   completion language for partial support.

## Non-Goals

- OxIde UI layout, pane composition, keybindings, or Tauri packaging.
- DnaOneCalc shell placement.
- Browser/WASM COM runtime execution.
- Replacing `oxvba-lsp` for VS Code-class hosts.
- Native AOT compiler delivery.

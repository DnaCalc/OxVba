# OxIde Embedded Build/Run Integration Guidance

> [!CAUTION]
> **Historical integration guidance.** It does not prove a current direct-host surface; use the current architecture and `HOST-SESSION-001`.

This note defines how OxIde should consume the current direct embedded build/run surface from OxVba.

The goal is one coherent OxVba-owned execution model:
- `oxvba_languageservice::HostWorkspaceSession` owns workspace overlays and semantic queries
- `oxvba_host::EmbeddedBuildRunHost` owns typed build/run lifecycle
- `oxvba_host::EmbeddedRunSession` owns the live runtime session used by run, reset, invoke, Immediate Window, and debugger flows
- OxIde owns UI, command routing, save policy, output panes, and transcript/panel presentation

## Current Implemented Surface

OxIde should treat these OxVba types as the execution boundary:
- `HostWorkspaceSession`
- `EmbeddedWorkspaceInput`
- `EmbeddedWorkspaceSnapshot`
- `EmbeddedBuildRunHost`
- `EmbeddedBuildRequest`
- `EmbeddedRunRequest`
- `EmbeddedBuildRunHostCommandStatus`
- `EmbeddedResetRequest`
- `EmbeddedInvokeEntryPointRequest`
- `EmbeddedInvokeProcedureRequest`
- `EmbeddedRunSession`
- `EmbeddedRunSessionCommandStatus`

Current source policy is explicit:
- `DiskOnly`
- `WorkspaceOverlay`

Recommended defaults:
- OxIde should use `WorkspaceOverlay`
- CLI can continue using `DiskOnly`

## Ownership Split

OxVba owns:
- project truth
- workspace-overlay snapshot extraction
- typed compile/build results
- live runtime session creation
- runtime reset
- entry-point/procedure invocation
- Immediate Window and debugger attachment substrate

OxIde owns:
- toolbar/menu/command routing
- save-before-build or run-with-unsaved-edits UX
- build output panes and status banners
- deciding when to reuse or discard a live run session
- wiring Immediate Window and debugger panes onto the returned runtime session

## Recommended Request Flow

For build:
1. Keep current editor buffers synchronized into `HostWorkspaceSession`.
2. Construct `EmbeddedWorkspaceInput` for the active workspace target with `WorkspaceOverlay`.
3. Call `HostWorkspaceSession::prepare_embedded_workspace_snapshot(...)`.
4. Submit that snapshot through `EmbeddedBuildRunHost::build_workspace(...)`.
5. Render `EmbeddedBuildResult` directly into build output and diagnostics UI.
6. Use `build_workspace_with_events(...)` when the host wants typed
   ID-bearing build lifecycle events instead of only the terminal result.

For run:
1. Prepare the same workspace snapshot using the same source policy.
2. Call `EmbeddedBuildRunHost::run_project(...)` or
   `run_project_with_events(...)` if lifecycle events are needed.
3. Keep the returned `EmbeddedRunSession` alive as the current project runtime owner and use its `runtime_session_id()` for UI correlation.
4. Route reset/reinvoke requests through that same session instead of recreating ad hoc runtime state.

For procedure invocation:
- use `EmbeddedRunSession::invoke_entry_point(...)` for startup behavior
- use `EmbeddedRunSession::invoke_procedure(...)` for bounded direct procedure execution

OxIde should not:
- reconstruct compiler input from editor buffers itself
- shell out to CLI for normal build/run IDE actions
- parse CLI text for build status or diagnostics
- invent a second runtime/session layer over the returned `EmbeddedRunSession`

## Suggested OxIde Ownership Model

- `ProjectSession` owns one `HostWorkspaceSession`
- `ProjectSession` also owns one `EmbeddedBuildRunHost`
- `ProjectSession` may own zero or one active `EmbeddedRunSession`
- `DocumentSession` continues to push edits into `HostWorkspaceSession`
- build/run commands operate against the current `ProjectSession`

That keeps one clean layering:
- editor state in OxIde
- workspace/project/runtime truth in OxVba

## Runtime Session Composition

The returned `EmbeddedRunSession` is the bridge to richer interactive tooling.

OxIde should treat it as the shared runtime anchor for:
- rerun/reset
- direct procedure invocation
- Immediate Window composition
- debugger composition

Recommended policy:
- when the workspace target changes materially, drop the old run session
- when the user requests reset, call `reset_runtime(...)`
- when build/run source policy changes, prepare a fresh workspace snapshot and start a fresh run session

## Current Evidence

The embedded execution lane currently has direct regression coverage for:
- request/result contract construction in `crates/oxvba-host/src/embedded.rs`
- build success/failure typing in `embedded::tests`
- runtime run/reset/invoke behavior in `embedded::tests`
- `HostWorkspaceSession` snapshot extraction in `crates/oxvba-languageservice/src/host_session.rs`
- disk-only versus workspace-overlay divergence in `host_session_embedded_round_trip_uses_disk_and_overlay_snapshots_independently`
- compile-diagnostic versus runtime-reset separation in `host_session_embedded_validation_separates_build_diagnostics_from_runtime_reset_flow`

That means OxIde can now consume:
- a typed workspace snapshot handoff
- a typed build result
- a typed live runtime session
- typed reset and invoke operations
- request IDs, runtime session IDs, command availability DTOs, and ID-bearing
  build/run lifecycle events

without falling back to CLI parsing.

## Practical Integration Order

1. Keep the current OxIde direct `HostWorkspaceSession` integration.
2. Add `EmbeddedBuildRunHost` to `ProjectSession`.
3. Implement build over `WorkspaceOverlay`.
4. Implement run and keep the returned `EmbeddedRunSession`.
5. Route existing or upcoming Immediate Window actions over the active run session.
6. Route debugger attachment over the same run session model.

## Rule Of Thumb

If OxIde needs:
- build status
- run/reset/invoke semantics
- runtime ownership
- source-policy-aware execution

that belongs in OxVba.

If OxIde needs:
- output presentation
- save prompts
- command wiring
- session lifetime UX
- pane composition

that belongs in OxIde.

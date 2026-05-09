# OxVba Embedded Build/Run Contract v1

This document defines the intended typed build/run contract for direct-embed hosts such as OxIde.

It is a design target, not a claim of completed implementation.

## Purpose

OxVba already has the low-level substrate needed for embedded execution:
- canonical project loading in `oxvba-project`
- host-facing workspace/document session APIs in `oxvba-languageservice`
- runtime/session ownership in `oxvba-host`
- direct immediate and debugger session surfaces in `oxvba-host`

What is still missing is a single typed host-facing orchestration layer for:
- build
- run
- reset
- invoke
- structured output/diagnostics/status reporting

This contract exists so OxIde does not have to:
- shell out to the CLI,
- parse CLI output,
- or guess how to stitch lower-level OxVba calls together for basic IDE workflows.

## Governing Rules

1. The embedded build/run contract must sit over existing OxVba project, language-service, and runtime APIs.
2. It must not introduce a second compiler path or a second project model.
3. CLI behavior may reuse the same substrate, but the contract is owned by the direct Rust API, not by CLI text.
4. Build diagnostics and runtime/session events must be typed and distinct.
5. Immediate Window and debugger flows must be able to attach to the same live runtime session created by the run contract.
6. The contract must define how unsaved editor overlays from `HostWorkspaceSession` are incorporated into build/run so OxIde does not have to invent a parallel “dirty buffer” compile path.

Decision:
- the contract will expose an explicit build/run source policy
- it will not silently choose between disk state and workspace overlay state

## Ownership Split

OxVba owns:
- project loading and build preparation
- compile/build execution
- runtime session creation and reset
- invoke semantics
- typed diagnostics, status, and output events

OxIde owns:
- command routing and toolbar/menu actions
- status displays and output panes
- build/run UX
- deciding when to build, run, rerun, reset, or attach immediate/debug views

## Intended Surface

The owning home for this contract is `oxvba-host::embedded`.

The initial facade shape is:
- `oxvba_host::EmbeddedBuildRunHost`
- `oxvba_host::EmbeddedWorkspaceSnapshot`
- the `Embedded*Request` / `Embedded*Result` / `EmbeddedBuildRunEvent` family

This chooses the public host boundary now without claiming the full executable substrate is already complete.

### Requests

Expected first request family:
- `BuildWorkspace`
- `RunProject`
- `ResetRuntime`
- `InvokeEntryPoint`
- `InvokeProcedure`

Potential future request family:
- `RunWithArguments`
- `BuildWithProfile`
- `BuildWithPolicy`
- `StartDebugSession`
- `AttachImmediateWindow`

Each request should identify whether it operates against:
- canonical on-disk project state,
- the current `HostWorkspaceSession` overlay state,
- or a previously prepared build/runtime handle.

Expected first source policy enum:
- `DiskOnly`
- `WorkspaceOverlay`
- optionally later `PreparedSnapshot`

### Results

Expected first result family:
- `BuildResult`
- `RunResult`
- `ResetResult`
- `InvokeResult`

These results should expose:
- success/failure
- structured diagnostics
- produced artifact or manifest metadata when relevant
- runtime session availability
- structured output events
- duration/phase metrics where inexpensive

For wrapper targets, build requests and results must expose the physical build
target as typed data. The current `WrappedComServer` DTO slice uses:
- `EmbeddedBuildTarget::WrappedComServer`
- `EmbeddedBuildPlan { target, artifacts, required_tools, warnings }`
- `EmbeddedBuildResult { plan, dll_path, tlb_path, registration_plan, diagnostics }`
- `EmbeddedComServerCapabilityProfile { windows, bitness, toolchain, registration_scopes }`
- `EmbeddedComServerRegistrationPlan { scope, requires_admin, command_hint }`

OxIde should use these fields directly for disabled states, artifact display,
registration affordances, and toolchain messaging. It should not infer
WrappedComServer availability by parsing CLI output.

### Events

Expected event family:
- `BuildStarted`
- `BuildCompleted`
- `BuildFailed`
- `RunStarted`
- `RunCompleted`
- `RunFailed`
- `OutputLine`
- `RuntimeReset`
- `SessionReady`

These events are for live host consumption and should remain typed, not line-parsed.

## Minimal V1 Shape

The first bounded implementation should cover:

1. `build_workspace(...)`
- loads or accepts a canonical project target
- optionally consumes current `HostWorkspaceSession` overlay state
- compiles/builds it
- returns typed diagnostics and build status

2. `run_project(...)`
- builds if needed or accepts a prepared manifest/session plan
- operates against the same workspace/overlay state model used by `build_workspace`
- starts a `ProjectRuntimeSession`
- returns a typed run result plus a live runtime/session handle

3. `reset_runtime(...)`
- resets the current live runtime session for the same workspace
- returns typed reset status and any reset-time diagnostics

4. `invoke_entry_point(...)`
- invokes the configured startup/entry behavior against a prepared runtime session
- returns typed invoke status and structured output

The first version does not need:
- arbitrary argument-passing surface
- multi-session orchestration
- debug attach in the same batch
- artifact packaging policy

The first version does need one explicit policy:
- build/run requests carry a source policy
- `DiskOnly` means execute the current on-disk project state
- `WorkspaceOverlay` means execute a snapshot derived from the current `HostWorkspaceSession`
- the host does not construct compiler input itself

## Relationship To Existing Surfaces

This contract should sit above:
- `oxvba_project::load_workspace_target`
- `oxvba_languageservice::HostWorkspaceSession`
- `oxvba_host::Engine`
- `oxvba_host::ProjectRuntimeSession`
- `oxvba_host::ImmediateSession`
- `oxvba_host::DebugSession`

The important rule is:
- `HostWorkspaceSession` owns editor/workspace overlays
- embedded build/run owns build/runtime lifecycle
- both must share the same real project identity

The build/run contract should therefore either:
- accept a `HostWorkspaceSession` directly,
- accept a typed workspace snapshot derived from it,
- or document a deterministic snapshot handoff step.

It should not require OxIde to manually recreate compiler input from editor buffers.

Recommended concrete direction:
- the request carries the source policy plus workspace identity
- OxVba performs any required snapshot extraction internally
- `oxvba_languageservice::HostWorkspaceSession::prepare_embedded_workspace_snapshot(...)`
  is the first concrete handoff point for that snapshot extraction
- OxIde defaults to `WorkspaceOverlay`
- CLI defaults to `DiskOnly`

## Error Model

The contract should separate:
- invalid workspace/project shape
- compile/build failure
- runtime startup failure
- missing/invalid entry point
- invoke failure
- host policy rejection

These must stay typed so hosts can present:
- diagnostics panes,
- build status,
- runtime error banners,
- and retry/reset actions
without string parsing.

## OxIde Consumption Model

OxIde should eventually consume the contract like this:

1. Use `HostWorkspaceSession` for workspace/document overlays and semantic queries.
2. Use project helper APIs for module/reference authoring.
3. Use the embedded build/run contract for:
   - build,
   - run,
   - rerun,
   - reset,
   - invoking the current project/runtime.
4. Attach Immediate Window and debugger views to the returned live runtime session.

Recommended host sequence:
1. apply project authoring through validated project-edit plans,
2. keep unsaved editor text in `HostWorkspaceSession`,
3. request build/run with `WorkspaceOverlay`,
4. attach immediate/debug tooling to the returned runtime session.

This keeps the host architecture clean:
- editor state in OxIde
- semantics/project/build/runtime truth in OxVba

## Non-Goals

This contract does not imply:
- replacing the CLI
- replacing `oxvba-lsp`
- native packaging targets beyond the typed wrapper DTOs named in this contract
- COM/XLL runtime hosting contracts
- browser/web host transport contracts

Those remain separate lanes.

## Intended Follow-On

The next bounded implementation bead for this area should:
- publish the OxIde-facing guidance and evidence for the now-landed:
  - snapshot handoff,
  - `build_workspace`,
  - `run_project`,
  - `reset_runtime`,
  - `invoke_entry_point`,
  - `invoke_procedure` surface.

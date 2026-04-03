# OxIde Direct Host Showcase Boundary

This note records the current honest OxVba-side showcase boundary for OxIde as a direct-embed host.

It does not claim that OxIde has already consumed every surface below.
It records:
- what OxVba now exposes for OxIde,
- what direct-host evidence exists inside this repo,
- and what still requires real OxIde-side adoption before the broader showcase claim is complete.

## Current OxVba-Side Direct Host Stack

OxIde now has these intended direct Rust seams available from OxVba:

### Workspace and editor semantics

- `oxvba_languageservice::HostWorkspaceSession`
- `oxvba_languageservice::HostWorkspaceDocument`
- `oxvba_languageservice::HostSessionError`

These cover:
- workspace load/reload
- real project-backed document identity
- text overlays
- restore-to-baseline close semantics
- diagnostics
- symbols
- completions
- hover
- definition
- references
- semantic provenance

### Project and authoring helpers

- `oxvba_project::inspect_workspace_target`
- `oxvba_project::prepare_host_project_edit_plan`
- `oxvba_project::validate_host_project_edits`
- `oxvba_project::apply_host_project_edit_plan`
- COM selection and project-state helpers in `oxvba_project::com_selection`

These cover:
- workspace roster/reference inspection
- module/logical-name inspection and reconciliation
- validated `.basproj` edit planning/application
- COM reference discovery, assessment, and repair planning

### Interactive runtime surfaces

- `oxvba_host::ImmediateSession`
- `oxvba_host::DebugSession`
- `oxvba_host::EmbeddedBuildRunHost`
- `oxvba_host::EmbeddedRunSession`

These cover:
- non-debug Immediate Window / REPL evaluation
- direct debug session control and paused evaluation
- typed embedded build/run
- runtime reset
- entry-point and bounded procedure invocation

## Current OxVba-Side Evidence

The current direct-host evidence inside this repo is:

### Session and overlay evidence

- `crates/oxvba-languageservice/src/host_session.rs`
- host-session tests proving:
  - workspace document loading
  - referenced-project document inclusion
  - close-to-baseline behavior
  - project-manifest snapshot extraction
  - disk-only versus overlay snapshot divergence

### Immediate Window evidence

- `docs/OXIDE_IMMEDIATE_WINDOW_INTEGRATION_GUIDANCE.md`
- `crates/oxvba-host/src/immediate.rs`
- targeted REPL/immediate tests in `oxvba-host` and `oxvba-cli`

### Debugger evidence

- `docs/evidence/DEBUGGER_HOST_HARNESS_V1.md`
- `crates/oxvba-host/tests/debug_session_host_harness.rs`

That harness proves a direct host can:
- start a paused debug session
- step into/out
- inspect frames
- inspect locals
- evaluate current-frame identifiers

### Embedded build/run evidence

- `docs/OXIDE_EMBEDDED_BUILD_RUN_INTEGRATION_GUIDANCE.md`
- `crates/oxvba-host/src/embedded.rs`
- `crates/oxvba-languageservice/src/host_session.rs`

Targeted regression coverage proves:
- request/result contract shape
- typed build success/failure
- source-policy-aware snapshot handoff
- disk-only versus workspace-overlay execution differences
- compile-diagnostic versus runtime-reset separation
- live runtime reset
- entry-point and bounded procedure invocation

## Recommended OxIde Composition

The intended OxIde composition is now:

- `ProjectSession` owns `HostWorkspaceSession`
- `ProjectSession` owns project-helper orchestration over `oxvba-project`
- `ProjectSession` owns `EmbeddedBuildRunHost`
- active runtime ownership is one `EmbeddedRunSession`
- Immediate Window composes over the active runtime session
- debugger composes over the active runtime session

OxIde should not:
- parse CLI output for editor/build/run behavior
- reconstruct compiler input from editor buffers
- route semantic editor behavior through LSP
- invent a parallel project model

## Current Honest Claim

OxVba can now honestly claim:
- the direct-host surface for OxIde is broad and intentional
- the major OxVba-side seams exist for:
  - workspace/editor semantics
  - project authoring helpers
  - Immediate Window
  - debugger
  - embedded build/run
- those seams have direct-host evidence inside the OxVba repo

OxVba cannot yet honestly claim:
- that OxIde itself has already adopted the full direct host stack
- that the OxIde showcase is complete end-to-end
- that build/run/immediate/debug flows are already proven inside the OxIde repo

## Remaining Showcase Gap

The remaining gap is now external-consumer proof, not missing OxVba substrate.

The next real step is:
- update OxIde to consume the direct host surfaces,
- then capture evidence that OxIde is using:
  - `HostWorkspaceSession`
  - project-helper plans
  - `EmbeddedBuildRunHost`
  - `EmbeddedRunSession`
  - direct Immediate Window and debugger seams

Until that happens, this note is the honest boundary:
- OxVba-side direct-host substrate: ready and evidenced
- OxIde-side full showcase adoption: still pending

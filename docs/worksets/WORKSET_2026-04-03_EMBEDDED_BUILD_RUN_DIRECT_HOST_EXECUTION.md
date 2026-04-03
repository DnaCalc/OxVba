# Workset: Embedded Build/Run Direct Host Execution

Date: 2026-04-03
Owner: Codex
Status: in-progress

## Purpose

Turn the embedded build/run design into an executable OxVba-side delivery lane for direct-embed hosts such as OxIde.

This workset exists to deliver the missing typed host-facing build/run substrate that sits between:
- `oxvba-project` project truth,
- `oxvba-languageservice::HostWorkspaceSession` editor/workspace overlays,
- and `oxvba-host` runtime/session ownership.

## Why This Exists

OxVba now has:
- a direct workspace/document session API,
- validated project edit planning and apply flows,
- typed immediate and debugger surfaces,
- and a published embedded build/run design with an explicit source-policy decision.

What is still missing is the direct host-facing execution seam that lets OxIde say:
- build this workspace,
- run this workspace,
- reset this runtime,
- invoke this startup/procedure,
without shelling out to CLI or stitching together lower-level calls ad hoc.

## Governing Policy

1. The embedded build/run contract is a direct Rust host API, not a CLI protocol.
2. It must reuse the canonical project/language-service/runtime substrate.
3. It must not introduce a second compiler path or a second project model.
4. Source-of-truth selection is explicit:
   - `DiskOnly`
   - `WorkspaceOverlay`
   - later optionally `PreparedSnapshot`
5. OxVba performs overlay snapshot extraction internally; hosts do not recreate compiler input from editor buffers.
6. Immediate Window and debugger flows must be able to attach to the resulting live runtime session.

## Required Outcomes

1. A first typed request/result/event surface exists for direct embedded build/run.
2. The source-policy handoff between `HostWorkspaceSession` and build/run is implemented explicitly.
3. Hosts can build and run against:
   - on-disk project state
   - or current workspace overlays
4. A typed runtime/reset/invoke surface exists over the same live runtime session model.
5. Regression coverage proves:
   - disk-only behavior
   - workspace-overlay behavior
   - diagnostics separation from runtime failure
   - deterministic reset/reinvoke behavior

## Execution Slices

1. define concrete request/result/event types
2. choose owning crate/module and facade shape
3. implement source-policy snapshot handoff
4. implement first `build_workspace` substrate
5. implement first `run_project` / `reset_runtime` / `invoke_entry_point` substrate
6. implement bounded `invoke_procedure` substrate over the same runtime/session model
7. add unit/integration/transcript evidence
8. publish OxIde-facing guidance and evidence for the new execution seam

## Relationship To Existing Work

This workset executes the design in:
- `docs/spec/OXVBA_EMBEDDED_BUILD_RUN_CONTRACT_V1.md`

It is the execution follow-on to:
- `docs/worksets/WORKSET_2026-04-01_OXIDE_HOST_SURFACE_AND_VSCODE_ALTERNATE_EDITOR_EXECUTION.md`

It must stay consistent with:
- `docs/spec/OXIDE_DIRECT_HOST_SESSION_FACADE_V1.md`
- `docs/spec/OXVBA_IMMEDIATE_EVALUATOR_CONTRACT_V1.md`
- `docs/spec/OXVBA_DEBUGGER_CONTRACT_V1.md`

## Non-Goals

This workset does not claim:
- wrapper/native packaging
- COM/XLL hosting
- browser/web transport
- LSP transport behavior
- full OxIde UI implementation

## Exit Condition

This workset is complete only when:
- a typed embedded build/run substrate is implemented,
- the source-policy handoff is real and tested,
- direct hosts can use it without CLI parsing,
- and the OxIde-facing guidance/evidence lane is published honestly.

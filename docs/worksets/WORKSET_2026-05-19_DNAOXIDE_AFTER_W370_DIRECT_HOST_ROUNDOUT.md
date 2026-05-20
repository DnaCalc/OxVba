# Workset: DNA OxIde After-W370 Direct Host Roundout

Date: 2026-05-19
Status: in-progress
Source handoff: `../OxIde/docs/HANDOFF_OXVBA_REQUESTED_WORK_AFTER_W370.md`

## Purpose

Process the OxIde after-W370 requested-work handoff into OxVBA-owned truth.
The handoff supersedes several OxIde-side notes, but it was written against
the OxVBA source snapshot vendored by OxIde. Current OxVBA `master` already
contains several of the requested W365/W370/W380 direct-host DTOs.

This workset therefore has two jobs:

1. freeze which requests are already satisfied by checked-in OxVBA surfaces;
2. create delivery lanes for the remaining OxVBA-owned gaps without claiming
   OxIde has consumed them.

## Boundary

OxVBA owns public direct-host API truth for runtime, Immediate, debug, watch,
breakpoint, language-service/editor versioning, COM/runtime capability states,
browser/WASM host availability, source spans, and shared error/capability
taxonomy.

OxIde owns UI rows, adapter-local IDs while OxVBA surfaces are missing from the
vendored dependency, stale-response guards inside the app, and evidence that a
particular OxIde build consumes a newer OxVBA surface.

This workset does not claim:

- that OxIde has updated its vendored OxVBA dependency;
- that current OxVBA has full editor source-span coverage;
- that browser/WASM runtime/debug/COM execution is available;
- that COM runtime invocation is safe or broadly supported.

## Intake Result

| Handoff Priority | Current OxVBA State | Processing Decision |
| --- | --- | --- |
| P1 debug identity, watches, breakpoints, source spans | Most requested DTO names already exist in `oxvba-host`: `DirectHostDebugSessionId`, `DirectHostStackFrameId`, `DirectHostWatchId`, `DirectHostBreakpointId`, `DebugSessionCommandStatus`, `DebugWatchRecord`, `DebugWatchEvaluation`, `DebugWatchEvaluationStatus`, `DebugBreakpointRecord`, `DebugBreakpointBindingStatus`, `DebugBreakpointUnresolvedReason`, `DebugVariantPauseState`, and `HostDebugVariantRunResult`. Tests cover stable IDs, command status, watch add/update/remove/evaluation states, breakpoint binding/unresolved/hit/clear behavior, and ThinSliceHello-style debug proof. | Treat identity/watch/breakpoint DTOs as `available-subset`; keep residual delivery open for broader source-span/remap coverage and richer paused-context expression evaluation. |
| P2 runtime/Immediate session attachment | `EmbeddedRunSession::into_immediate_session` exists in `oxvba-host`, and the ThinSliceHello fixture test attaches Immediate from a live runtime session with runtime/immediate ID correlation. | Treat attachment as `available-subset`; keep residual delivery open for no-session host-level disabled DTOs, source-span diagnostics, and broader evaluation-failure taxonomy. |
| P3 runtime/debug source-span breadth | `DirectHostSourceSpan` exists and debug pause/frame records expose procedure/range-oriented source data, but not every requested row has editor-grade spans. | Accept as remaining delivery work. |
| P4 workspace/editor version identity | `HostWorkspaceRoster` exposes `snapshot_revision`, per-module `document_version`, and overlay flags. The lower-level language-service API has semantic provenance, but not every editor-facing response carries a stable document/workspace version DTO. | Accept as remaining delivery work. |
| P5 browser/WASM host feasibility | `oxvba-web-host`, HAL browser profile descriptors, and typed browser/native unsupported issue kinds exist. A checked browser-safe crate-graph gate and feature-separated unavailable packet family are not yet proven here. | Accept as remaining delivery work. |
| P6 COM runtime invocation boundary | `ComCapabilityProfile` and `ComRuntimeInvocationAvailability` exist and expose typed unavailable states; no broad COM runtime invocation claim is made. | Accept as boundary-hardening work; do not claim invocation support beyond existing availability DTOs. |
| P7 shared error/capability taxonomy | `DirectHostIssueKind`, `DirectHostIssue`, `DirectHostCommandStatus`, and `DirectHostCapability*` exist with stable `DH-*` codes for many handoff categories. Missing or under-specific categories include stale document/workspace version, runtime busy/running/stopped distinctions, no-session host-level Immediate/debug states, and browser/WASM unsupported at the response-family level. | Accept as taxonomy hardening tied to future DTO lanes. |

## Current Evidence Anchors

- `crates/oxvba-host/src/direct_host.rs`
- `crates/oxvba-host/src/embedded.rs`
- `crates/oxvba-host/src/immediate.rs`
- `crates/oxvba-host/src/debugger.rs`
- `crates/oxvba-languageservice/src/host_session.rs`
- `crates/oxvba-languageservice/tests/dnaoxide_thin_slice_hello.rs`
- `docs/worksets/WORKSET_2026-05-07_DNAOXIDE_FULL_SCOPE_HOST_INTEGRATION_SUPPORT.md`
- `docs/evidence/DNAOXIDE_THIN_SLICE_HELLO_FIXTURE_2026-05-07.md`

Representative checks already cited by the May 7 workset:

```powershell
cargo test -p oxvba-host embedded_run_session_attaches_immediate_and_debug_with_stable_ids --quiet
cargo test -p oxvba-host debug_session_watch_registry_reports_unavailable_error_and_value_states --quiet
cargo test -p oxvba-host debug_session_breakpoint_records_bind_disable_clear_and_count_hits --quiet
cargo test -p oxvba-languageservice --test dnaoxide_thin_slice_hello --quiet
```

## Execution Lanes

### Lane A - After-W370 Surface Reconciliation

Effect: support.

Outcome: keep this intake, the May 7 DnaOxIde workset, and public guidance in
sync with current `master` so OxIde can distinguish stale vendored gaps from
real OxVBA gaps.

Close condition:

- requested DTO vocabulary is mapped to concrete OxVBA types or residual gaps;
- residual gaps have delivery owners below;
- no full-support claim is made for source spans, browser/WASM, or COM runtime
  invocation.

### Lane B - Source-Span Breadth

Effect: delivery.

Outcome: editor-grade `DirectHostSourceSpan` coverage for runtime errors,
debug pause/current statement, stack frames, watch diagnostics, breakpoint
binding/unresolved rows, and Immediate diagnostics where source is available.

Close condition:

- direct-host DTOs carry source spans or typed "no source span available"
  reasons for the listed row families;
- ThinSliceHello-style fixture evidence proves editor navigation rows without
  mutating checked-in fixtures.

### Lane C - Editor Version Identity

Effect: delivery.

Outcome: every editor-facing language/build/check response intended for direct
OxIde consumption carries document/workspace version identity sufficient for
host-side stale-response rejection.

Close condition:

- diagnostics, classifications, outline/symbols, hover, definition,
  references, completions, signature help, rename preparation, code actions,
  and build/check diagnostics expose provenance versions through stable DTOs;
- overlapping unsaved-overlay request tests prove stale responses can be
  rejected using OxVBA-provided versions.

### Lane D - Browser/WASM Safe Subset

Effect: delivery.

Outcome: a browser-safe OxVBA subset can build without native COM/runtime
dependencies and returns typed unavailable packets for unsupported services.

Close condition:

- `wasm32-unknown-unknown` or the chosen browser target has a checked crate
  graph for workspace load and language-service/check surfaces;
- runtime/debug/COM/native services expose typed unavailable results instead
  of compile failures in the browser profile;
- feature gates are documented for OxIde browser builds.

### Lane E - COM Runtime Boundary And Shared Taxonomy

Effect: delivery.

Outcome: COM runtime invocation availability and shared direct-host issue
taxonomy are precise enough for OxIde command enablement and tests.

Close condition:

- Windows/non-Windows, missing COM, host policy, unsupported runtime profile,
  bitness/apartment, stale version, runtime busy/running/stopped, no-session,
  and browser unsupported cases have stable typed issue/status coverage;
- safe COM runtime invocation remains disabled unless a separately evidenced
  invocation contract is delivered.

## Bead Tree

Parent:

- `bd-94av` - DNA OxIde after-W370 direct-host roundout

Epics:

- `bd-94av.1` - after-W370 handoff reconciliation and rollout
- `bd-94av.2` - runtime/debug/Immediate source-span breadth
- `bd-94av.3` - editor response version identity
- `bd-94av.4` - browser/WASM safe subset
- `bd-94av.5` - COM runtime boundary and direct-host taxonomy hardening

First executable beads:

- `bd-94av.1.1` - publish after-W370 intake map and residual lanes
- `bd-94av.2.1` - audit runtime/debug/Immediate row families for source-span DTO gaps (audit published in `docs/reviews/DNAOXIDE_AFTER_W370_SOURCE_SPAN_AUDIT_2026-05-20.md`; delivery children open)
- `bd-94av.2.2` - add source-bearing runtime diagnostic DTOs
- `bd-94av.2.3` - project debug pause/frame/breakpoint source spans
- `bd-94av.2.4` - project watch and Immediate diagnostic source spans
- `bd-94av.3.1` - audit editor-facing responses for missing version provenance
- `bd-94av.4.1` - prove or bound the browser-safe crate graph
- `bd-94av.5.1` - extend direct-host issue taxonomy for stale/busy/no-session/browser gaps

## Terminal Condition

This workset can close only when every real residual from the after-W370 handoff
is either:

1. delivered with local tests/evidence and public DTO documentation; or
2. explicitly deferred to a named workset/bead with a non-claim boundary that
   OxIde can consume honestly.

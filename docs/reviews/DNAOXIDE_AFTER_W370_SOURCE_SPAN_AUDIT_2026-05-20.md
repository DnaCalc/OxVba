# DNA OxIde After-W370 Source-Span Audit

Date: 2026-05-20
Bead: `bd-94av.2.1`
Workset: `docs/worksets/WORKSET_2026-05-19_DNAOXIDE_AFTER_W370_DIRECT_HOST_ROUNDOUT.md`

## Scope

This audit covers the after-W370 request for editor-grade source-span DTOs on
runtime, debug, watch, breakpoint, and Immediate direct-host surfaces.

The audit is not a delivery claim. Its purpose is to identify which source
truth already exists, which direct-host rows expose it, and which follow-up
delivery beads own the missing projection work.

## Current Source Truth

Available source truth:

- `oxvba_host::DirectHostSourceSpan` exists with `document_id`, `start`, and
  `end` positions.
- `DirectHostIssueContext` can already carry an optional
  `DirectHostSourceSpan`.
- compiler `ProcedureRuntimeMetadata` carries `module_name`, `procedure_name`,
  `entry_pc`, `source_line_start`, `source_line_end`, `statement_line_numbers`,
  and `statement_entry_pcs`.
- VM `DebugStop` carries `DebugSourceLocation` with `module_name`,
  `procedure_name`, `entry_pc`, `statement_pc`, and optional `line_number`.
- `HostWorkspaceRoster` exposes module/document identity and per-document
  versions for workspace-backed projects.

Important limitation:

- runtime/debug/Immediate host DTOs generally know module names and line
  numbers, while `DirectHostSourceSpan` requires a `DirectHostDocumentId`.
  The missing bridge is a stable module-to-document source map at the direct
  runtime/debug/Immediate boundary.

## Row-Family Findings

| Row family | Current exposed source shape | Gap | Delivery owner |
| --- | --- | --- | --- |
| Runtime build/run diagnostics | `EmbeddedBuildResult`, `EmbeddedRunResult`, and invocation results carry `PhaseDiagnostic` values. `PhaseDiagnostic` has phase and message only. | No document ID, source range, or typed no-source reason is available on runtime/build/run diagnostics. | `bd-94av.2.2` |
| Runtime invocation failures | `EmbeddedInvokeVariantResult` carries target module/procedure and diagnostics on failure. | The target gives a module/procedure hint, but failure rows do not carry an editor span or explicit source-unavailable reason. | `bd-94av.2.2` |
| Debug pause/current statement | `HostDebugVariantRunResult::Paused` carries `DebugVariantPauseState`; `DebugStop` carries module/procedure and optional line. | Stop rows do not expose `DirectHostSourceSpan`, document ID, or span-unavailable status. | `bd-94av.2.3` |
| Debug stack frames | `DebugFrameVariant` carries module/procedure plus procedure `source_line_start` and `source_line_end`. | Frame rows expose line ranges but not document-ID-backed `DirectHostSourceSpan`; frame locals have no declaration/source range. | `bd-94av.2.3` |
| Debug breakpoint binding rows | `DebugBreakpointRecord` carries module name, line number, binding status, unresolved reason, and hit count. | Bound/unresolved rows do not carry `DirectHostSourceSpan`; unresolved reasons are not source-linked when the module exists but no executable statement exists on the requested line. | `bd-94av.2.3` |
| Watch evaluations | `DebugWatchEvaluation` carries watch ID, expression text, and value/unavailable/error status. Error statuses carry `DirectHostIssue` but no source span. | Watch diagnostics do not carry the paused-frame source span, expression source span, or explicit no-source reason. | `bd-94av.2.4` |
| Immediate diagnostics | `ImmediateVariantEvaluationResult` carries output and diagnostics. Parse/literal diagnostics are returned as `PhaseDiagnostic`. `ImmediateSessionError` can project some target-module context into `DirectHostIssue`. | Immediate diagnostics lack input/source spans, target module document IDs, and no-session/source-unavailable DTO status. | `bd-94av.2.4` |
| ThinSliceHello evidence | Existing fixture proves overlay build/run, Immediate, debug, watch, and breakpoint behavior over temp project copies. | It proves module/line metadata is usable for breakpoint setup, but it does not assert document-ID-backed spans on output rows. | `bd-94av.2.3`, `bd-94av.2.4` |

## Delivery Split

The missing work should be split because each lane has a different owner and
source-map shape:

1. Runtime/build/run diagnostics need a source-bearing diagnostic DTO layered
   over `PhaseDiagnostic` without breaking existing callers.
2. Debug pause/frame/breakpoint rows need a module/document source-map bridge
   and stable `DirectHostSourceSpan` projection.
3. Watch and Immediate diagnostics need diagnostic/status rows that can attach
   either paused-frame spans, input spans, or typed no-source reasons.

## Delivery Updates

### `bd-94av.2.2` Runtime Diagnostic DTOs

Status: delivered subset.

`DirectHostDiagnostic`, `DirectHostSourceSpanStatus`, and
`DirectHostSourceUnavailableReason` now provide an additive direct-host
diagnostic row over existing `PhaseDiagnostic` values. Existing build/run/
invoke result fields remain unchanged.

Runtime/build/run projection now covers:

- `EmbeddedBuildResult::direct_host_diagnostics`;
- `EmbeddedRunResult::direct_host_diagnostics`;
- `EmbeddedInvokeVariantResult::direct_host_diagnostics`;
- `EmbeddedInvokeVariantResult::direct_host_diagnostics_with_source`.

Tests prove:

- build compile failures project a typed direct-host diagnostic with
  `NoSourceLocation`;
- run compile failures project a typed direct-host diagnostic with
  `NoSourceLocation`;
- runtime invocation failure rows can carry a known `DirectHostSourceSpan` and
  propagate the document ID into `DirectHostIssueContext`.

Residual:

- automatic runtime error source recovery from VM string errors is not claimed;
- module/document source-map projection for debug pause/frame/breakpoint rows
  remains under `bd-94av.2.3`;
- watch and Immediate diagnostic source spans remain under `bd-94av.2.4`.

### `bd-94av.2.3` Debug Pause/Frame/Breakpoint Spans

Status: delivered subset.

`DebugVariantPauseState`, `DebugFrameVariant`, and `DebugBreakpointRecord`
now carry `DirectHostSourceSpanStatus` values. The direct debugger layer maps
manifest module identity to `DirectHostDocumentId` and projects line-backed
spans where compiler/VM metadata identifies a module and source line.

Debug projection now covers:

- pause/current statement spans on `DebugVariantPauseState::current_source`;
- procedure range spans on `DebugFrameVariant::source`;
- bound breakpoint spans on `DebugBreakpointRecord::source`;
- module-existing unresolved breakpoint spans for non-executable requested
  lines;
- `NoMatchingDocument` for breakpoint rows whose module cannot be mapped.

Tests prove:

- entry pauses carry a document-ID-backed current statement span;
- stepped frames carry document-ID-backed procedure spans;
- bound breakpoint rows carry the requested executable line span;
- module-existing unresolved breakpoint rows retain the requested source line;
- no-matching-module breakpoint rows carry a typed no-source status instead of
  a fake span.

Residual:

- source-map projection is manifest-module based; path/URI mapping remains an
  adapter concern;
- watch and Immediate diagnostic source spans remain under `bd-94av.2.4`.

### `bd-94av.2.4` Watch and Immediate Diagnostic Spans

Status: delivered subset.

`DebugWatchEvaluation` and `ImmediateVariantEvaluationResult` now carry
`DirectHostSourceSpanStatus` values, and `ImmediateSessionError` exposes
`direct_host_source()` for error rows that do not return an evaluation result.

Watch and Immediate projection now covers:

- not-paused watch rows with typed `NoSourceLocation`;
- paused watch value/error rows with the current paused-frame source status;
- Immediate parse/literal diagnostics with typed
  `GeneratedOrSyntheticSource`, avoiding fake document IDs for ad hoc input;
- missing unqualified Immediate targets with typed `NoSourceLocation`;
- unknown Immediate target modules with typed `NoMatchingDocument` and no fake
  source span.

Tests prove:

- not-paused watch unavailable rows carry a typed no-source status;
- unknown watch-expression errors carry the paused-frame source status;
- Immediate literal and parse diagnostics carry synthetic-input source status;
- unknown target modules do not fabricate a `DirectHostSourceSpan`.

Residual:

- Immediate input text is not yet represented by a first-class document ID;
- richer expression-range spans remain future editor/adapter projection work.

## Non-Claims

This audit does not claim:

- full editor source-span coverage;
- source ranges for every local variable declaration;
- arbitrary Immediate expression parsing spans;
- complete runtime error source recovery for VM errors that currently surface
  only as strings.

Those remain delivery work under the child beads created from this audit.

## Review Notes

Fresh-eyes review points before closing `bd-94av.2.1`:

- The audit distinguishes existing compiler/VM source truth from direct-host
  DTO projection gaps.
- It avoids calling the current module/line-only debug surface complete.
- It leaves child delivery beads for every row family named in the handoff.
- It does not broaden COM, browser/WASM, or editor-version claims outside this
  bead's source-span scope.

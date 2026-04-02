# OxVba Debugger Contract V1

This spec defines the first bounded OxVba-side debugger contract for direct hosts.

The design target is a semantic debugger for OxVba code:
- source and statement oriented,
- VM-backed first,
- directly consumable from OxIde,
- and later projectable into DAP for VS Code.

## Current Runtime Starting Point

OxVba already has the key substrate for a real debugger:
- persistent `ProjectRuntimeSession` in `oxvba-host`,
- `Engine::compile_and_prepare_session(...)`,
- `Engine::invoke_procedure(...)`,
- a live `Vm` that can execute procedure bodies against existing session state.

That means the debugger should be built as an extension of the current live-session runtime model, not as a second evaluator or an external native debugger wrapper.

## Governing Rules

1. Debugger truth is semantic OxVba state.
2. Breakpoints are statement/source based, not native-PC based.
3. The first debugger lane must be VM-backed and cross-platform in principle.
4. Direct hosts consume typed Rust APIs.
5. VS Code is a later DAP projection over the same debugger core.
6. Native debugger integration is non-goal add-on territory, not the product foundation.

## Required Core Types

The first debugger surface should converge on these typed concepts:
- `DebugSourceLocation`
- `DebugStatementId`
- `DebugBreakpoint`
- `DebugBreakpointId`
- `DebugStopReason`
- `DebugFrame`
- `DebugScope`
- `DebugValue`
- `DebugSession`
- `DebugEvaluationRequest`
- `DebugEvaluationResult`

Names may change, but the shape should remain:
- source location and statement identity are explicit,
- breakpoints have stable host-facing IDs,
- pauses report stop reason plus current frame/source location,
- frame and value inspection remain OxVba-typed rather than stringly.

## Required Runtime Metadata

The VM/debugger substrate will need:
- candidate source statement lines per procedure,
- emitted statement-entry PCs per procedure,
- later statement-to-bytecode/source zippering across nested control flow,
- procedure entry metadata enriched with source range identity,
- stable frame metadata for active procedure calls,
- slot/value inspection for arguments, locals, and return values.

Current bounded landing:
- procedure runtime metadata carries module/procedure identity,
- source line start/end,
- candidate source statement lines,
- top-level emitted statement-entry PCs,
- and a VM-backed paused execution substrate with:
  - entry pause,
  - line breakpoints,
  - continue,
  - step into,
  - step over,
  - step out.

That is enough substrate to begin semantic stop planning honestly and to exercise bounded stepping behavior over simple procedure/call shapes, but not yet enough for final breakpoint/step resolution across all nested control-flow shapes. The zippering and richer paused-state projection layers remain later debugger beads.

## Execution Model

### First lane: VM debug mode

The first debugger implementation should run through the interpreter/VM path and expose:
- set/clear/list breakpoints,
- continue,
- step into,
- step over,
- step out,
- inspect stack frames,
- inspect locals/arguments,
- evaluate bounded expressions in the current frame.

### Later lane: JIT coexistence

JIT execution does not need first-pass debug parity.
Acceptable initial policy:
- debug sessions force VM mode, or
- debug sessions deopt/fall back into VM execution.

## Direct Host Boundary

OxIde should eventually be able to:
- create a debug session from a loaded project/runtime target,
- register and update breakpoints,
- receive stop events,
- inspect current frame/source/locals,
- evaluate expressions in the paused context,
- continue or step without inventing its own execution model.

OxVba should own:
- debug execution semantics,
- source/statement identity,
- stack/value projection,
- expression evaluation semantics.

OxIde should own:
- breakpoint UI,
- watch panes,
- call stack panes,
- command routing,
- status/pause presentation.

## Relationship To Immediate Window

The planned Immediate Window / CLI REPL lane should reuse this substrate later.

Order:
1. live-session non-debug evaluation can land first,
2. debugger paused-context evaluation should then reuse the same semantic evaluation core with frame context added.

## First Executable Ladder

1. publish this debugger contract and child-bead ladder
2. add source/statement identity metadata and first emitted-statement PC substrate sufficient to start semantic stop planning
3. add VM pause/continue/breakpoint substrate
4. expose typed host-facing debug session APIs
5. prove direct-host/OxIde consumption with a bounded harness
6. plan or project DAP later

## Exit Condition For V1

V1 is complete when:
- a host can debug OxVba code semantically through typed APIs,
- breakpoints and stepping are driven by statement/source identity,
- pause state exposes stack and locals,
- and OxIde has a credible direct integration path without native debuggers.

# OxVba Debugger DAP Projection V1

> [!CAUTION]
> **Future/historical design, not current capability.** Any debugger revival must conform to `DEBUG-CORE-001` and use the current project/runtime session architecture.

This note defines the intended Debug Adapter Protocol projection over the OxVba semantic debugger core.

It does not introduce a second debugger.
The adapter should remain a projection over `oxvba_host::DebugSession`.

## Core Rule

VS Code debugging must project the same OxVba-owned semantics that OxIde uses directly:
- breakpoints stay source/statement based
- stepping stays semantic
- frames and values come from `DebugSession`
- paused evaluation comes from OxVba, not TypeScript-side interpretation

## Current OxVba Substrate

Today the debugger core already provides:
- VM-backed semantic pause/continue
- step into / over / out
- source locations
- typed pause state
- frame/value projection
- bounded current-frame identifier evaluation

That is enough for a first DAP mapping plan, but not yet enough to claim full adapter delivery.

## Recommended Adapter Ownership Split

OxVba should own:
- `DebugSession`
- breakpoint matching semantics
- stop reasons
- frame/value projection
- paused evaluation semantics

The DAP adapter should own:
- protocol transport
- request/response translation
- VS Code launch/attach wiring
- path/URI mapping
- presentation-oriented shaping required by DAP

The adapter must not:
- invent a parallel breakpoint model
- evaluate expressions itself
- derive frames from native state
- duplicate OxVba runtime truth in TypeScript

## Direct Mapping Shape

Recommended first mapping:

- `setBreakpoints`
  - maps to `DebugSession` breakpoint registration
  - breakpoints should stay line/module oriented until richer statement IDs are exposed

- `continue`
  - maps to `DebugSession::continue_execution()`

- `next`
  - maps to `DebugSession::step_over()`

- `stepIn`
  - maps to `DebugSession::step_into()`

- `stepOut`
  - maps to `DebugSession::step_out()`

- `stackTrace`
  - projects `DebugPauseState.frames`

- `scopes`
  - first bounded shape can expose one locals scope per frame

- `variables`
  - projects `DebugFrame.values`

- `evaluate`
  - maps to `DebugSession::evaluate(...)`
  - current bounded truth: current-frame identifier lookup only

- `stopped` event
  - reason derives from OxVba stop reason:
    - `Entry`
    - `Breakpoint`
    - `Step`

## Launch Model

Recommended first model:
- VS Code extension launches an OxVba debug adapter
- the adapter loads the OxVba project/workspace
- the adapter creates one `DebugSession`
- all DAP execution control delegates into that live session

This should remain separate from `oxvba-lsp`.
LSP and DAP may share workspace/project discovery helpers, but should not be merged into one protocol surface.

## Honest Current Gaps

The DAP projection is technically credible now, but some debugger-core work still needs to deepen before the adapter should claim broad parity:
- stronger unit/behavior coverage
- direct-host integration/transcript evidence
- richer paused-context evaluation
- fuller breakpoint/statement zippering across complex nested control flow

So the correct next order is:
1. finish debugger validation and host evidence
2. then build the adapter projection

## Relationship To VS Code Lane

The VS Code lane should consume this as guidance:
- `docs/worksets/WORKSET_2026-04-02_VSCODE_EXTENSION_AND_LSP_FEATURE_LADDER_EXECUTION.md`

The debugger lane remains the owner of semantic debugging truth:
- `docs/worksets/WORKSET_2026-04-02_OXVBA_DEBUGGING_SERVICE_AND_HOST_INTEGRATION.md`

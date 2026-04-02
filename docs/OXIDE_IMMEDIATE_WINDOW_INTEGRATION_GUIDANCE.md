# OxIde Immediate Window Integration Guidance

This note defines how OxIde should consume the current OxVba immediate and debugger surfaces.

The goal is one coherent OxVba-owned execution model:
- non-debug Immediate Window uses `oxvba_host::ImmediateSession`
- paused debug inspection uses `oxvba_host::DebugSession`
- OxIde owns UI, transcript, focus, panes, and command routing
- OxVba owns runtime truth, evaluation semantics, pause state, and frame/value projection

## Current Non-Debug Shape

OxIde should treat one loaded OxVba project as the owner of one live immediate evaluator:
- create one `ImmediateSession` from the active project manifest
- keep it alive across repeated Immediate Window commands
- reset or recreate it when the underlying project/runtime target changes materially

Recommended ownership:
- `ProjectSession` owns the current `ImmediateSession`
- the Immediate Window pane submits typed requests into that session
- OxIde renders `ImmediateEvaluationResult` into transcript/UI form

## Current Request Flow

For the bounded V1 surface, OxIde should:
- map the active editor module to `ImmediateSession::set_default_target_module(...)`
- submit one line at a time through `ImmediateEvaluationRequest`
- render:
  - `Empty`
  - `Value`
  - `PrintedLine`
  - `Reset`
- surface compile/runtime diagnostics directly from `ImmediateEvaluationResult`

OxIde should not:
- recompile a fresh project for each line
- invent a parallel evaluator
- parse CLI output
- route the Immediate Window through LSP

## Reset And Session Lifecycle

OxIde should recreate or reset the immediate session when:
- the workspace/project is reloaded
- runtime policy/profile changes invalidate the live session
- the user explicitly resets the Immediate Window
- document/source changes require a fresh compiled runtime baseline

Good UX policy:
- preserve transcript presentation in OxIde
- but make session resets explicit to the user

## Debugger-Context Layering

When OxIde later enters break mode, it should not switch to a different evaluator product.

Instead:
- `DebugSession` owns the paused runtime truth
- OxIde should use `DebugSession` for:
  - breakpoints
  - continue
  - step into / over / out
  - pause state
  - frame/value inspection
- the Immediate Window UI should then layer on the paused debug session

Current bounded paused evaluation:
- `DebugSession::evaluate(...)` supports current-frame identifier lookup
- this is enough for first watch/evaluate and locals-pane style interactions
- broader paused-context expression evaluation remains a later debugger/evaluator slice

## Recommended OxIde Split

OxVba should own:
- `ImmediateSession`
- `DebugSession`
- request/result types
- runtime resets
- pause state
- frame/value projection

OxIde should own:
- Immediate Window pane
- transcript rendering
- watch pane / locals pane presentation
- active-module targeting policy
- command routing between normal and break modes

## Practical Integration Order

1. Non-debug Immediate Window over `ImmediateSession`
2. Active-module targeting from the current editor document
3. Explicit reset/reload UX
4. Debug panes over `DebugSession`
5. Paused-context Immediate Window routing over `DebugSession::evaluate(...)`
6. Later richer paused-context expression support as OxVba grows

## Rule Of Thumb

If OxIde needs:
- runtime truth,
- evaluation,
- frames,
- locals,
- or paused inspection,

that belongs in OxVba.

If OxIde needs:
- presentation,
- focus,
- transcript history,
- keybindings,
- or pane composition,

that belongs in OxIde.

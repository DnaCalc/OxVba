# Handoff: build `oxvba-dap` on `DebugSessionHandle`

Audience: future `oxvba-dap` workset implementers.

Scope: this is an adapter design handoff only. It does **not** claim a DAP server is implemented in this repository.

## Layering rule

`oxvba-dap` should be a protocol adapter over `oxvba_debug::DebugSessionHandle`:

```text
DAP client <-> oxvba-dap request/event adapter <-> oxvba_debug::DebugSessionHandle
```

Do not reintroduce a second debugger worker, a second source-map implementation, or a dependency from `oxvba-host` back to `oxvba-debug`.

## Session setup

For `initialize` / `launch` / `attach`-style startup:

1. Build or receive an `Engine`, `ProjectManifest`, and `DebugAttachConfig`.
2. Call `attach_debug_session(engine, manifest, config)`.
3. Store the returned `DebugSessionHandle` in the DAP session object.
4. Spawn an event pump over `DebugSessionAttach.events` and later `handle.subscribe()` receivers as needed.
5. Reply to DAP requests from handle method results; emit DAP events from `DebugEvent` values.

Use `DebugAttachConfig` to select event channel mode, output capture, start mode, and COM apartment policy. Do not create an internal Tokio runtime in `oxvba-debug`; if the DAP server is async, enable the `tokio` feature and use `*_async` methods.

## DAP request mapping

| DAP request | `oxvba-debug` surface |
|---|---|
| `initialize` | adapter-only capability response; no handle call required |
| `launch` / custom attach | `attach_debug_session(...)` |
| `configurationDone` | `handle.start()` if the adapter chooses manual start after configuration |
| `continue` | `handle.continue_execution()` |
| `next` | `handle.step_over()` |
| `stepIn` | `handle.step_into()` |
| `stepOut` | `handle.step_out()` |
| `setBreakpoints` | diff existing `handle.breakpoints()` against requested lines, then call `set_source_breakpoint`, `set_breakpoint_enabled`, and/or `clear_source_breakpoint` |
| `breakpointLocations` | derive from `DebugSourceMap` / module source-map data; do not parse editor text independently |
| `threads` | v1 reports primary thread id `1` from events/paused state |
| `stackTrace` | `handle.stack_frames()` |
| `scopes` | create adapter-side scopes for locals/watch/evaluate; frame ids come from `DebugFrameView` |
| `variables` | `handle.frame_locals(&frame_id)` for locals; project `DebugValueView` into DAP variables |
| `evaluate` | `handle.evaluate(Some(&frame_id), expression)` or `handle.evaluate(None, expression)` |
| `setExpression` / watch edit | `handle.update_watch(&watch_id, expression)` when adapter exposes persistent watches |
| `disconnect` | `handle.detach()` on the final handle clone; otherwise drop clones and report lifecycle errors honestly |

## DAP event mapping

| `DebugEvent` | DAP event |
|---|---|
| `ModuleLoaded` | `module` event (`reason = "new"`) |
| `ThreadStarted` | `thread` event (`reason = "started"`) |
| `Stopped` | `stopped` event; map `DebugStopReasonView::{Entry, Step, Breakpoint}` to DAP reason strings |
| `Continued` | `continued` event |
| `Exited` | `exited` and/or `terminated` event depending on adapter shutdown policy |
| `BreakpointChanged` | `breakpoint` event (`changed`/`new`/`removed` as available) |
| `Output` | `output` event with stdout/stderr/host category mapping |

Preserve the `DebugEvent` sequence ordering in the DAP pump. Late subscribers are future-only; the initial `DebugSessionAttach.events` receiver is the one that observes attach-time module/thread events.

## DTO projection guidance

Use the public view DTOs as the protocol boundary:

- `DebugPauseView` -> DAP stopped context/current source location;
- `DebugFrameView` -> DAP stack frame;
- `DebugValueView` -> DAP variable/evaluate result;
- `DebugBreakpointView` -> DAP breakpoint;
- `DebugWatchView` -> adapter watch rows;
- `DebugRunResultView` -> request response plus any emitted stop/exit event.

Do not expose raw `Variant` values through DAP. The `views` layer is already `Send + Sync + Clone + serde::Serialize` and is the intended transport surface.

## Lifecycle/error mapping

- `DebugAttachError::{Compile, Prepare}` -> failed `launch`/`attach` response with diagnostics.
- `DebugError::NotPaused` -> DAP request error for frame/variable/evaluate calls that require pause state.
- `DebugError::Evaluation { expression, message }` -> DAP evaluate failure response.
- `DebugError::Completed` / `SessionAlreadyDetached` -> terminated-session error response.
- `DebugError::OutstandingHandles { count }` -> adapter lifecycle bug; drop extra clones before claiming disconnect success.
- `DebugError::WorkerFailed { stage, message }` -> terminate the DAP session and surface an adapter error.

## Deferred DAP work

Out of scope for this workset and this handoff:

- the `oxvba-dap` crate/server process;
- JSON-RPC framing, sockets, stdio transport, VS Code packaging;
- DAP capability negotiation beyond the request/event map above;
- conditional breakpoints, hit conditions, exception breakpoints, pause, restart-frame, and edit-and-continue.

Those should be tracked in the future DAP workset. This document only maps the shipped `oxvba-debug` handle/event/view surface to that future adapter.

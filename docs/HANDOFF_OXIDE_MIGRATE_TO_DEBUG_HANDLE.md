# Handoff: migrate OxIde to `oxvba_debug::DebugSessionHandle`

Audience: DnaOxIde / `oxide-wf81` implementers.

Scope: this is a migration guide only. It does **not** claim the OxIde cockpit has been migrated in this repository.

## New dependency/import shape

Replace direct debugger consumption from `oxvba_host::DebugSession` / host debugger records with the `oxvba-debug` crate:

```rust
use oxvba_debug::{
    attach_debug_session, DebugAttachConfig, DebugEvent, DebugEventReceiver,
    DebugRunResultView, DebugSessionHandle,
};
use oxvba_host::{Engine, HostConfig};
```

`oxvba-host` no longer owns or re-exports the high-level debugger. Keep OxIde depending on both crates if it still needs `Engine` / project hosting, but source all debug handle, event, and DTO types from `oxvba_debug`.

## Attach/session ownership

Old shape:

- OxIde prepared an `oxvba_host::DebugSession<'engine>` or replayed stateless plans around one.
- The session was borrow-bound and `!Send`, so the cockpit had to add its own threading/Tauri bridge.

New shape:

```rust
let attach = attach_debug_session(engine, manifest, DebugAttachConfig::default())?;
let handle: DebugSessionHandle = attach.handle;
let events: DebugEventReceiver = attach.events;
```

Store `DebugSessionHandle` in OxIde managed state. It is `Clone + Send + Sync`; each command/invoke can clone the handle and call one method. Drop all clones or call `detach()` on the last clone to stop the worker.

## Adapter helper mapping

| OxIde helper / concern | New OxVba call |
|---|---|
| create debugger session | `attach_debug_session(engine, manifest, DebugAttachConfig::default())` |
| start/stop-on-entry flow | `handle.start()` and initial `DebugEvent::Stopped` / `DebugRunResultView::Paused` |
| continue | `handle.continue_execution()` |
| step into | `handle.step_into()` |
| step over | `handle.step_over()` |
| step out | `handle.step_out()` |
| set source breakpoint | `handle.set_source_breakpoint(module, file_line, enabled)` |
| enable/disable breakpoint | `handle.set_breakpoint_enabled(&breakpoint_id, enabled)` |
| clear breakpoint | `handle.clear_source_breakpoint(&breakpoint_id)` |
| list breakpoints | `handle.breakpoints()` |
| add watch | `handle.add_watch(expression)` |
| edit watch | `handle.update_watch(&watch_id, expression)` |
| remove watch | `handle.remove_watch(&watch_id)` |
| refresh watches | `handle.evaluate_watches()` |
| current pause model | `handle.current_pause()` |
| call stack | `handle.stack_frames()` |
| locals for a frame | `handle.frame_locals(&frame_id)` |
| immediate/evaluate expression | `handle.evaluate(Some(&frame_id), expression)` or `handle.evaluate(None, expression)` |
| subscribe to output/stop/exit changes | `attach.events` plus `handle.subscribe()` for later subscribers |
| async Tauri command ergonomic | enable `oxvba-debug/tokio` and call `*_async` variants |

## Retire OxIde-side replay plans

The new handle is stateful and worker-backed. OxIde should remove the W391-style stateless replay state:

- retire `stepPlan` and call the handle step/continue methods directly;
- retire `breakpointPlan` and keep only UI-level breakpoint intent plus returned `DebugBreakpointView` ids/status;
- retire `watchExpressions` replay and use returned `DebugWatchView` ids with `add_watch`, `update_watch`, `remove_watch`, and `evaluate_watches`.

OxIde may still keep UI projection state, but it should be derived from `DebugRunResultView`, `DebugPauseView`, `DebugBreakpointView`, `DebugWatchView`, and `DebugEvent` rather than replaying runtime execution.

## Source-line mapping

Remove OxIde's copied line-mapping implementation for debugger file lines. The compiler now emits structured source maps and `oxvba-debug` consumes them when binding breakpoints and projecting pause locations. OxIde should pass editor file lines to `set_source_breakpoint(module, file_line, enabled)` and render returned `DebugSourceLocationView` values.

## Event loop guidance

Use the initial receiver from `DebugSessionAttach` for attach-time events (`ModuleLoaded`, `ThreadStarted`, optional initial stop). Use `handle.subscribe()` for future-only receivers.

Handle these events in the cockpit:

- `DebugEvent::Stopped` -> update current frame/location and enabled controls;
- `DebugEvent::Continued` -> mark running;
- `DebugEvent::Exited` -> mark completed/detached;
- `DebugEvent::BreakpointChanged` -> refresh breakpoint rows;
- `DebugEvent::ModuleLoaded` -> populate module list;
- `DebugEvent::Output` -> append Debug.Print/stdout/stderr output;
- `DebugEvent::ThreadStarted` -> initialize thread display.

## Error and lifecycle expectations

- `DebugAttachError` means session creation failed; do not store a handle.
- `DebugError::OutstandingHandles { count }` means `detach()` was called while other handle clones still exist.
- `DebugError::SessionAlreadyDetached` means the worker is gone or the session was already detached.
- `DebugError::WorkerFailed { stage, message }` means the worker is unusable; discard the handle and offer reattach.

## Deferred OxIde work

This document does not implement OxIde UI commands, Tauri state wiring, panel rendering, or persistence. Those remain in `OxIde/oxide-wf81`; this handoff only identifies the shipped OxVba surface to consume.

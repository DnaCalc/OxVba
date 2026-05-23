# Handoff: OxVba Debug Handle v1

Date: 2026-05-23

Status: shipped in OxVba as the `oxvba-debug` crate. This handoff reconciles the workset scope, public migration path, and deferred downstream work.

## What shipped

- New workspace crate: `crates/oxvba-debug`.
- Raw debugger core moved out of the old host debugger facade into `oxvba_debug::DebugSessionCore`.
- Consumer-facing handle: `oxvba_debug::DebugSessionHandle`.
- Attach entry point: `oxvba_debug::attach_debug_session(engine, manifest, DebugAttachConfig)`.
- Transport-safe DTOs in `oxvba_debug::views` for pause, frames, values, breakpoints, watches, modules, exits, and run results.
- Sequenced event stream with stopped, continued, exited, output, breakpoint-changed, module-loaded, and thread-started events.
- Worker-thread command marshalling; the handle is `Clone + Send + Sync` and the core remains explicitly non-send/non-sync.
- COM apartment setup/reporting for the worker thread.
- Tokio-gated `*_async` handle methods and async event receive helpers.
- Compiler-emitted source maps consumed by `oxvba-debug` for file-line breakpoint binding and pause locations.
- Lifecycle/error behavior for attach failure, explicit detach, drop-all-handles shutdown, worker failure, reattach, and outstanding clones.
- Property, snapshot, stress, benchmark, and end-to-end scenario coverage.

## Public source-migration path

Old imports from the host debugger should migrate to `oxvba-debug`:

```rust
// old
// use oxvba_host::debugger::{DebugSession, DebugWatchRecord, ...};

// new
use oxvba_debug::{
    attach_debug_session, DebugAttachConfig, DebugEvent, DebugEventReceiver,
    DebugRunResultView, DebugSessionHandle,
};
```

`oxvba-host` remains the owner of `Engine`, `HostConfig`, project loading/runtime preparation, and narrow runtime debug primitives. It must not depend on `oxvba-debug`.

## Consumer flow

```rust
let attach = attach_debug_session(engine, manifest, DebugAttachConfig::default())?;
let handle = attach.handle;
let events = attach.events;

let paused = handle.start()?;
let breakpoint = handle.set_source_breakpoint("Module1", 5, true)?;
let run = handle.continue_execution()?;
let frames = handle.stack_frames()?;
handle.detach()?;
```

Use `attach.events` for startup events. Use `handle.subscribe()` for future-only event subscribers.

## Downstream handoff docs

- OxIde migration: `docs/HANDOFF_OXIDE_MIGRATE_TO_DEBUG_HANDLE.md`.
- Future DAP adapter: `docs/HANDOFF_OXVBA_DAP_FROM_DEBUG_HANDLE.md`.

These documents are guides only. OxIde migration and `oxvba-dap` are not implemented by this workset.

## Acceptance evidence

- Human-readable final matrix: `docs/evidence/oxvba-debug/acceptance.txt`.
- Machine-readable final matrix: `docs/evidence/oxvba-debug/acceptance.json`.
- Benchmark evidence: `docs/evidence/oxvba-debug/benchmarks/B13_HANDLE_LATENCY.md`.

## Deferred work

Deferred to downstream worksets/beads:

- OxIde cockpit migration (`OxIde/oxide-wf81`) consuming `DebugSessionHandle` and removing replay plans.
- `oxvba-dap` server crate and protocol implementation.
- CLI debug REPL.
- DAP-over-TCP / remote debugging.
- Conditional breakpoints, hit conditions, exception breakpoints, pause, restart-frame, and edit-and-continue.

No workset-scoped implementation follow-up is intended to remain only in chat; downstream items above are explicitly out of this workset boundary.

# OxVba Debug Handle Design

> [!CAUTION]
> **Future/historical design over a non-active crate.** Current system status is in `docs/ARCHITECTURE.md` §12; any debugger revival must conform to system clause `DEBUG-CORE-001`.

Status: B00 binding design for `docs/worksets/WORKSET_2026-05-23_OXVBA_DEBUG_HANDLE_ARCHITECTURE.md`.
Owner bead: `bd-00fz.1`.

This document freezes the public architecture for the `oxvba-debug` crate beads B01-B15. It supersedes the older direct-host wording in `OXVBA_DEBUGGER_CONTRACT_V1.md` where that document refers to `oxvba_host::DebugSession` as the public consumer surface; after this workset, consumers use `oxvba_debug::DebugSessionHandle`, while the stateful raw debugger lives as `oxvba_debug::DebugSessionCore`.

## 1. Goals and non-goals

Goals:
- Move debugger-domain ownership into a new `oxvba-debug` crate.
- Preserve the existing VM-backed debugger semantics while changing ownership from a borrow-bound host facade to an owned core.
- Provide a consumer-facing `DebugSessionHandle` that is `Send + Sync + Clone + 'static` without unsafe `Send` or `Sync` implementations.
- Centralize the worker-thread, event stream, lifecycle, COM-apartment, source-map, and async-wrapper architecture so OxIde, future `oxvba-dap`, and future CLI/test harnesses do not duplicate it.
- Keep `oxvba-host` free of a reverse dependency on `oxvba-debug`.

Non-goals:
- No DAP protocol crate in this workset.
- No OxIde adapter migration in this workset.
- No persistent debug worker pool.
- No conditional breakpoints, hit counts, exception breakpoints, edit-and-continue, DAP-over-TCP, or debugger UI.
- No source import compatibility shim such as `oxvba_host::debugger::*` re-exporting `oxvba-debug`.

## 2. Layer model and crate boundaries

```text
Layer 3: protocol / UI adapters
  future oxvba-dap, future oxvba-cli debug, OxIde oxide-wf81 migration
      |
      v
Layer 2: oxvba-debug::DebugSessionHandle
  Send + Sync + Clone handle, worker ownership, commands, events, async wrappers
      |
      v
Layer 1: oxvba-debug::DebugSessionCore
  stateful debugger core, explicitly !Send + !Sync, VM-backed runtime state
      |
      v
oxvba-host
  Engine, HostConfig, ProjectRuntimeSession preparation, narrow debug runtime primitives
      |
      v
oxvba-vm / oxvba-compiler / oxvba-runtime / oxvba-hal / oxvba-com
```

Dependency direction:
- `oxvba-debug -> oxvba-host` for runtime preparation and narrow debug VM operations.
- `oxvba-debug -> oxvba-vm` for VM stop/run result primitives.
- `oxvba-debug -> oxvba-runtime` for internal `Variant` projections in the core layer only.
- `oxvba-debug -> oxvba-compiler` for `ProjectManifest`, compiled project metadata, and compiler-emitted source maps.
- `oxvba-host` must not depend on `oxvba-debug`.

## 3. Ownership map

| Surface | Owner after this workset | Notes |
|---|---|---|
| `Engine`, `HostConfig`, project/runtime preparation | `oxvba-host` | unchanged owner |
| Low-level debug VM operations on `ProjectRuntimeSession` | `oxvba-host` | narrow API consumed by `oxvba-debug` |
| Stateful raw debug session | `oxvba-debug::DebugSessionCore` | owned, explicit `!Send`/`!Sync` |
| Breakpoint records and binding status | `oxvba-debug::breakpoints` | domain records, not transport DTOs |
| Watch records and evaluations | `oxvba-debug::watches` | domain records, not transport DTOs |
| Pause/frame state | `oxvba-debug::frames` | retains runtime detail; projected before crossing worker boundary |
| Public view DTOs | `oxvba-debug::views` | transport-ready, serde, `Send + Sync` |
| Handle, worker, command channel | `oxvba-debug::{handle, worker, command}` | Layer 2 |
| Event stream | `oxvba-debug::events` | sync-first event hub plus tokio wrapper under feature |
| Source-map application | `oxvba-debug::source_map` | compiler emits maps; debug crate consumes them |
| COM apartment initialization for debug workers | `oxvba-debug::com_apartment` | Windows-gated wrappers |

## 4. Public API contract

The top-level crate exports these families:

```rust
pub use core::{DebugCoreConfig, DebugCoreRunResult, DebugSessionCore};
pub use records::{
    DebugBreakpointRecord, DebugEvaluationRequest, DebugSessionCommandStatus,
    DebugWatchRecord,
};
pub use views::{
    DebugBreakpointView, DebugExitView, DebugFrameView, DebugModuleView,
    DebugPauseView, DebugRunResultView, DebugSourceLocationView,
    DebugStopReasonView, DebugValueView, DebugWatchView,
};
pub use events::{DebugEvent, DebugEventReceiver};
pub use errors::{DebugAttachError, DebugError};
pub use config::{
    DebugAttachConfig, DebugComApartment, DebugEventChannelMode,
    DebugOutputCaptureMode, DebugStartMode,
};

pub fn prepare_debug_session_core(
    engine: Arc<oxvba_host::Engine>,
    manifest: oxvba_compiler::ProjectManifest,
    config: DebugCoreConfig,
) -> Result<DebugSessionCore, DebugAttachError>;

pub fn attach_debug_session(
    engine: Arc<oxvba_host::Engine>,
    manifest: oxvba_compiler::ProjectManifest,
    config: DebugAttachConfig,
) -> Result<DebugSessionAttach, DebugAttachError>;

pub struct DebugSessionAttach {
    pub handle: DebugSessionHandle,
    pub events: DebugEventReceiver,
}

#[derive(Clone)]
pub struct DebugSessionHandle { /* Arc<HandleInner> */ }
```

`DebugSessionHandle` synchronous command surface:

```rust
impl DebugSessionHandle {
    pub fn start(&self) -> Result<DebugRunResultView, DebugError>;
    pub fn step_into(&self) -> Result<DebugRunResultView, DebugError>;
    pub fn step_over(&self) -> Result<DebugRunResultView, DebugError>;
    pub fn step_out(&self) -> Result<DebugRunResultView, DebugError>;
    pub fn continue_execution(&self) -> Result<DebugRunResultView, DebugError>;

    pub fn set_source_breakpoint(
        &self,
        module: &str,
        file_line: u32,
        enabled: bool,
    ) -> Result<DebugBreakpointView, DebugError>;
    pub fn set_breakpoint_enabled(
        &self,
        id: &DirectHostBreakpointId,
        enabled: bool,
    ) -> Result<DebugBreakpointView, DebugError>;
    pub fn clear_source_breakpoint(&self, id: &DirectHostBreakpointId) -> Result<(), DebugError>;
    pub fn breakpoints(&self) -> Result<Vec<DebugBreakpointView>, DebugError>;

    pub fn add_watch(&self, expression: &str) -> Result<DebugWatchView, DebugError>;
    pub fn update_watch(&self, id: &DirectHostWatchId, expression: &str) -> Result<DebugWatchView, DebugError>;
    pub fn remove_watch(&self, id: &DirectHostWatchId) -> Result<(), DebugError>;
    pub fn evaluate_watches(&self) -> Result<Vec<DebugWatchView>, DebugError>;

    pub fn current_pause(&self) -> Result<Option<DebugPauseView>, DebugError>;
    pub fn stack_frames(&self) -> Result<Vec<DebugFrameView>, DebugError>;
    pub fn frame_locals(&self, frame: &DirectHostStackFrameId) -> Result<Vec<DebugValueView>, DebugError>;
    pub fn evaluate(
        &self,
        frame: Option<&DirectHostStackFrameId>,
        expression: &str,
    ) -> Result<DebugValueView, DebugError>;

    pub fn subscribe(&self) -> DebugEventReceiver;
    pub fn session_id(&self) -> &DirectHostDebugSessionId;
    pub fn detach(self) -> Result<(), DebugError>;
}
```

A cooperative `pause` method is deliberately absent in v1. It must not be published as a stub until the VM can actually pause a running evaluation.

Under `feature = "tokio"`, every sync command has an `*_async` wrapper returning the same typed output. Async wrappers use the same worker and command serialization path; there is no async worker.

## 5. Core model

`DebugSessionCore` owns:
- `Arc<Engine>`;
- `ProjectManifest`;
- `ProjectRuntimeSession` prepared by `oxvba-host`;
- breakpoint and watch registries;
- compiler/debug source maps;
- last pause state and retained runtime values needed by existing debugger semantics.

`DebugSessionCore` is explicitly `!Send` and `!Sync`, for example via a private `PhantomData<Rc<()>>`. It is constructed inside the worker thread for handle sessions. The raw `prepare_debug_session_core` path exists for tests and in-process low-level embedding, but it still returns a non-sendable core.

## 6. Handle and worker model

- `attach_debug_session` spawns exactly one worker thread per session.
- The caller passes `Arc<Engine>` and an owned `ProjectManifest`; no borrowed engine lifetime crosses the worker boundary.
- The worker initializes the configured COM apartment before constructing `DebugSessionCore`.
- Commands cross a single sync `crossbeam_channel::Sender<DebugCommand>`.
- Every public sync method sends one command and blocks on its reply.
- Async methods send the same commands and await caller-side tokio oneshots.
- Commands serialize at the worker, matching the VM's single-threaded execution model.
- Only transport-ready view values and typed errors cross from worker to callers.
- The handle has no unsafe `Send`/`Sync` implementation; those traits must derive from concrete fields.
- Last `Arc<HandleInner>` drop triggers clean shutdown.

## 7. View DTO contract

All public view DTOs are:
- `Send + Sync + Clone + Debug`;
- `serde::Serialize + serde::Deserialize`;
- free of raw `Variant` or other non-transport runtime carriers.

`DebugValueView` is the value projection boundary. It carries presentation-safe fields such as `display_text`, `type_label`, `kind`, and optional raw diagnostic payload bytes when a bead explicitly adds them. It does not expose a live `Variant`.

`DebugPauseView`, `DebugBreakpointView`, `DebugWatchView`, and frame/value views use editor file lines after B08 source-map work. Until B08 lands, any temporary implementation must keep the acceptance claim scoped to the current runtime-line basis.

## 8. Event taxonomy

`DebugEvent` is the canonical event stream for DAP, OxIde, CLI, and tests:

- `Stopped { seq, session_id, reason, thread_id, frame_id, location }`
- `Output { seq, session_id, channel, text }`
- `Continued { seq, session_id, all_threads_continued }`
- `Exited { seq, session_id, exit_code }`
- `BreakpointChanged { seq, session_id, change, breakpoint }`
- `ModuleLoaded { seq, session_id, module }`
- `ThreadStarted { seq, session_id, thread_id }`

Rules:
- Every event carries a monotonic worker-assigned `seq`.
- If a command emits an event and returns a reply, the worker emits the event first and then sends the command reply.
- The `DebugSessionAttach.events` receiver is registered before startup events are emitted.
- Later `handle.subscribe()` receivers see future events only; no implicit replay.
- Default channel mode is `DebugEventChannelMode::Bounded(256)`.
- Bounded mode is drop-oldest and reports a typed lag/drop signal to slow subscribers; slow subscribers must not block the worker.
- `Unbounded` is opt-in for controlled embeddings that own the memory-growth risk.

## 9. Lifecycle and errors

Attach errors:
- Compile/prepare failure returns `Err(DebugAttachError)`.
- If attach fails, no usable handle is returned and no worker is left running.

Detach/drop:
- `detach(self)` consumes one handle clone.
- If other clones remain, it returns `DebugError::OutstandingHandles { count }` and does not pretend the session detached.
- If it owns the last strong reference, it sends shutdown and joins the worker.
- Dropping all handles triggers the same shutdown path idempotently.

Worker failure:
- The worker top level catches panics, records a failure state, and wakes/poisons pending/future commands with `DebugError::WorkerFailed { stage, message }`.
- Calls after normal completion return typed `DebugRunResultView::Exited` for the completing command and `DebugError::Completed` for later impossible stepping commands.
- In-flight commands during shutdown return `DebugError::SessionAlreadyDetached` rather than deadlocking.

Primary error variants:
- `NotPaused`
- `UnknownBreakpoint(DirectHostBreakpointId)`
- `UnknownWatch(DirectHostWatchId)`
- `UnknownFrame(DirectHostStackFrameId)`
- `Evaluation { expression, message }`
- `Completed`
- `UnsupportedCommand(&'static str)`
- `OutstandingHandles { count }`
- `SessionAlreadyDetached`
- `WorkerFailed { stage, message }`
- `Internal(String)` for unexpected errors that are also recorded diagnostically.

## 10. COM apartment policy

`DebugAttachConfig::com_apartment` values:
- `Sta`: Windows default. Worker calls `CoInitializeEx(NULL, COINIT_APARTMENTTHREADED)` and `CoUninitialize` on shutdown.
- `Mta`: Windows worker calls `CoInitializeEx(NULL, COINIT_MULTITHREADED)` and uninitializes on shutdown.
- `None`: no COM initialization; cross-platform test/default escape hatch for non-COM scenarios.

Placement rule: COM initialization belongs in the debug worker before core construction, not in every consumer.

Bounded STA claim: v1 STA support is valid for synchronous in-apartment COM work. Cross-apartment callbacks, connection-point sinks, or COM event-pump scenarios require a future pumped wait loop; until that lands, hosts that need those callbacks should use `Mta` or `None` unless they provide a compatible external pump.

Tests must verify apartment state from inside the worker thread. A caller-thread `CoGetApartmentType` check is not sufficient because COM apartment state is thread-local.

## 11. Source-map contract

B08 adds compiler-owned source maps to `CompiledProject`. The map is produced by the same lowering path that emits bytecode and procedure metadata; it is not a hardcoded preamble offset.

The debug crate wraps compiler maps as `DebugSourceMap` with:
- `file_to_runtime(module, file_line)`;
- `runtime_to_file(module, runtime_line)`;
- `nearest_executable_file_line(module, file_line)` for DAP-style breakpoint binding and unresolved explanations.

Mapping requirements:
- Dropped/non-executable source lines include `Attribute ...`, `Option Private Module`, class `Implements`, compiler-inserted helper lines, and empty/preamble-only modules as applicable.
- Preserved user lines include `Option Explicit`, `Option Compare`, `Option Base`, blanks, comments, and executable procedure body lines.
- Mappings are per module and independent across modules.
- For executable user lines, `runtime_to_file(file_to_runtime(N)) == N`.

Handle inputs accept editor file lines. Views and events report editor file lines. Breakpoint records keep enough binding status to explain unresolved file lines.

## 12. Output capture

B07 introduces a debug output tap at the host/VM boundary for `Debug.Print`, console stdout/stderr, and host diagnostic output. The tap must observe output without replacing or suppressing existing embedding callbacks. Captured output is emitted as `DebugEvent::Output` with a typed channel.

## 13. Async policy

The `tokio` feature adds caller-side ergonomic wrappers only:
- one async method per sync command;
- tokio-friendly event receiver wrapper over the same sequenced event stream;
- cancellation safe: dropping a future drops the reply receiver, and the worker discards the completed result without panic.

The worker remains synchronous and single-threaded.

## 14. Source migration story

This workset intentionally changes source imports:
- old raw path: `oxvba_host::debugger::*` / `Engine::prepare_debug_session(&manifest)`;
- new raw path: `oxvba_debug::prepare_debug_session_core(Arc<Engine>, manifest, DebugCoreConfig)`;
- preferred consumer path: `oxvba_debug::attach_debug_session(Arc<Engine>, manifest, DebugAttachConfig)` returning `DebugSessionHandle` plus an initial event receiver.

`oxvba-host` may retain a short migration note if helpful, but it must not re-export `oxvba-debug` and must not create an `oxvba-host -> oxvba-debug` edge.

Downstream handoffs:
- `docs/HANDOFF_OXIDE_MIGRATE_TO_DEBUG_HANDLE.md` maps OxIde adapter helpers to handle methods and retires the W391 stateless replay workaround.
- `docs/HANDOFF_OXVBA_DAP_FROM_DEBUG_HANDLE.md` maps DAP requests/events onto the handle/event stream.
- `docs/HANDOFF_OXVBA_DEBUG_HANDLE_v1.md` records final shipped behavior and deferred work at B15.

## 15. Implementation bead gates

- B01 creates the empty crate and workspace dependency shape.
- B02 encodes the test catalog as ignored stubs plus fixtures and Send/Sync static assertions.
- B03 moves the core and domain types with semantic regression tests.
- B04 adds transport-ready views and projections.
- B05 adds the handle worker and command marshalling without events.
- B06 defines event hub/subscription behavior.
- B07 wires event emissions and output capture.
- B08 adds compiler source maps and applies them on input/output paths.
- B09 adds COM apartment management.
- B10 adds tokio async wrappers.
- B11 hardens lifecycle and error propagation.
- B12 adds property and snapshot replay tests.
- B13 adds stress and benchmark baselines.
- B14 publishes downstream migration handoffs.
- B15 runs reference scenarios, writes acceptance evidence, and publishes v1 handoff.

## 16. B00 review status

Fresh-eyes design review checklist for B00:
- boundary direction prevents an `oxvba-host -> oxvba-debug` cycle;
- public API includes raw core and consumer handle entry points;
- no unsafe handle `Send`/`Sync` is permitted;
- core is explicitly `!Send`/`!Sync`;
- events, lifecycle, COM, async, source-map, errors, and migration are specified;
- test catalog maps every B02-B15 lane to concrete tests.

B00 approval means this file and `docs/spec/OXVBA_DEBUG_TEST_CATALOG.md` are internally consistent with the workset and can guide B01-B15 without hidden chat context.

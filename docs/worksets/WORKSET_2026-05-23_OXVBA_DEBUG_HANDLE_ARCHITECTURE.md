# Workset: OxVba Debug Handle Architecture (`oxvba-debug` crate)

Date: 2026-05-23
Status: planned
Source handoff: `../OxIde/docs/HANDOFF_W391_DEBUGGER_COCKPIT.md` (W391 review,
F1 / oxide-wf81 blocker); follow-up to `WORKSET_2026-05-07_DNAOXIDE_FULL_SCOPE_HOST_INTEGRATION_SUPPORT.md`.
Downstream consumers (planned): DnaOxIde (`OxIde/oxide-wf81`), `oxvba-dap`
(separate workset, layered on top), future `oxvba-cli debug`, future test
harnesses, future DAP-over-TCP remote-debugging.

## Purpose

OxVba's debug surface today (`oxvba_host::DebugSession<'engine>`) is a correct
stateful runtime debugger but exposes only one shape: a borrow-bound, `!Send`
stateful object that every consumer must wrap in its own threading bridge to
use across an integration boundary (Tauri invoke, DAP request loop, async
runtime, CLI REPL). The OxIde cockpit shipped a stateless-replay workaround
under W391; building `oxvba-dap` for VS Code / JetBrains / any DAP-compatible
editor would reinvent the same bridge with the same `unsafe impl Send`
justifications, the same COM-apartment story, and the same lifecycle plumbing
- in every consumer.

This workset stands up a new `oxvba-debug` crate as the long-term debugger
home. It owns both the raw stateful debugger core and the consumer-facing
`DebugSessionHandle`. The handle is `Send + Sync + 'static`, cheap to clone,
and owns a worker thread that holds explicitly `!Send` debug state. The crate
exposes the full step / breakpoint / watch / inspect surface plus a
first-class event stream (`Stopped` / `Output` / `Continued` / `Exited` /
...). Multiple consumers (OxIde in Tauri managed state, an in-process
embedder, a future `oxvba-dap` server) share one architecture instead of each
inventing the same scaffolding.

The "right interface shape" is the three-layer model:

```
Layer 3   protocol adapters (oxvba-dap, oxvba-cli debug, ...)   <-- separate worksets
              |
              v uses
Layer 2   DebugSessionHandle (Send + Sync handle + worker + events)  <-- this workset
              |
              v wraps a worker thread that owns
Layer 1   DebugSessionCore (moved from oxvba_host::DebugSession)  <-- this workset
```

`oxvba-dap` (Layer 3) is a distinct workset/crate layered on top of
`oxvba-debug`; this workset does **not** include DAP protocol work, but every
design decision here is made with DAP as a first-class consumer.

## Boundary

`oxvba-debug` owns:

- The raw stateful debugger core (`DebugSessionCore`), moved out of
  `oxvba-host::debugger` and made explicitly `!Send` / `!Sync`.
- All debugger-domain records and requests
  (`DebugWatchRecord/Evaluation/Status`,
  `DebugBreakpointRecord/BindingStatus/UnresolvedReason`,
  `DebugPauseState`, `DebugFrame*`, `DebugEvaluationRequest`,
  `DebugSessionCommandStatus`, core run-result types).
- The consumer-facing `DebugSessionHandle` (Send + Sync, Arc-clone, lifecycle).
- The worker thread, command marshalling channel, event broadcast stream, and
  COM apartment management.
- The public debug-domain DTOs (`DebugPauseView`, `DebugBreakpointView`,
  `DebugWatchView`, `DebugFrameView`, `DebugValueView`, `DebugEvent`, ...) -
  these become the canonical types every consumer reads.
- Debug source-map consumption for editor file lines <-> VM/runtime lines;
  the compiler emits the map and `oxvba-debug` applies it so every consumer
  benefits, not just OxIde.
- The async `*_async` surface (feature-gated tokio).
- The `prepare_debug_session_core(engine, manifest, config)` raw entry point
  and `attach_debug_session(engine, manifest, config)` handle entry point.

`oxvba-host` keeps:

- `Engine`, `HostConfig`, `ProjectRuntimeSession`, and runtime preparation
  (`compile_and_prepare_session`, plus the minimal public debug-runtime
  primitive methods needed by `oxvba-debug` to drive VM debug execution).
- `EmbeddedBuildRunHost`, `ProjectManifest` re-exports, all runtime-hosting
  surfaces unrelated to debug.
- No dependency on `oxvba-debug`. `oxvba-host::debugger` is deleted or reduced
  to a short migration note; it must not re-export `oxvba-debug`, because that
  would create the crate cycle this workset is explicitly removing.

`oxvba-vm` keeps the VM-internal primitives (`DebugStop`, `DebugStopReason`,
`DebugRunResult`) it already owns; `oxvba-debug` projects them into the public
DTOs above.

Out of scope (separate worksets):

- **`oxvba-dap`** - the DAP server crate on top of `oxvba-debug`.
- **OxIde migration** - tracked as `OxIde/oxide-wf81`; this workset only
  guarantees the handle is consumable from Tauri managed state.
- **DAP-over-TCP / remote debugging** - layers on top of `oxvba-dap`.
- **`oxvba-cli debug` REPL** - separate workset.
- Conditional breakpoints, hit counts, exception breakpoints, edit-and-continue
  - the architecture must not preclude them, but they're separate feature work
  on top of this surface.

## Status Terms

- `available`: exposed by `oxvba_host::DebugSession` today; moved or
  re-projected in `oxvba-debug` with no semantic change.
- `new-in-this-workset`: net-new surface created by `oxvba-debug` (handle,
  events, async, etc.).
- `moved`: type or function lives in `oxvba-debug` after this workset. Old
  `oxvba-host` imports are a source migration, not a re-export, because the
  new crate graph intentionally has no `oxvba-host -> oxvba-debug` edge.

## Architecture: layer boundaries and crate dependencies

```
                +-------------------+
                |   oxvba-debug     |   <-- new
                |   (Layer 1 core + |
                |    Layer 2 handle |
                |    + public DTOs) |
                +---------+---------+
                          |
                          v
                +---------+---------+
                |   oxvba-host      |
                |   (Engine, runtime|
                |    preparation +  |
                |    debug VM ops)  |
                +---------+---------+
                          |
                          v
                +---------+---------+
                |   oxvba-vm,       |
                |   oxvba-compiler, |
                |   oxvba-runtime,  |
                |   oxvba-hal       |
                +-------------------+
```

`oxvba-debug` depends on `oxvba-host`, `oxvba-vm`, `oxvba-runtime` (for
Variant types in core projections), and `oxvba-compiler` (for
`ProjectManifest` and source-map data). No reverse deps.

The host/debug boundary is intentionally narrow: `oxvba-host` prepares
`ProjectRuntimeSession` values and exposes low-level debug runtime operations
on that session; `oxvba-debug` owns debugger state, source mapping,
breakpoint/watch registries, pause/value projection, eventing, and handle
lifecycle.

## Crate-relocation map

| Surface | Today (oxvba-host) | After this workset |
|---|---|---|
| `Engine`, `HostConfig`, runtime preparation | `oxvba-host` | `oxvba-host` (unchanged owner) |
| Low-level debug VM ops on `ProjectRuntimeSession` | `oxvba-host` internals / `oxvba-host::debugger` callers | `oxvba-host` public narrow API consumed by `oxvba-debug` |
| `DebugSession<'engine>` (borrow-bound core) | `oxvba-host::debugger` | removed; replaced by owned `oxvba-debug::DebugSessionCore` |
| `DebugWatchRecord/Evaluation/Status` | `oxvba-host::debugger` | `oxvba-debug::watches` |
| `DebugBreakpointRecord/BindingStatus/UnresolvedReason` | `oxvba-host::debugger` | `oxvba-debug::breakpoints` |
| `DebugVariantPauseState`, `DebugFrameVariant*` | `oxvba-host::debugger` | `oxvba-debug::frames` |
| `HostDebugVariantRunResult`, `DebugSessionError`, `DebugEvaluationRequest`, `DebugVariantEvaluationResult`, `DebugSessionCommandStatus` | `oxvba-host::debugger` | `oxvba-debug::*` |
| Public view DTOs (Send+Sync projections) | n/a | `oxvba-debug::views` (new) |
| `DebugSessionHandle`, `attach_debug_session`, events, worker | n/a | `oxvba-debug` (new) |
| File-line <-> runtime-line source maps | partly in OxIde adapter / implicit compiler metadata | `oxvba-compiler` emits maps; `oxvba-debug::source_map` consumes them |

## Public API sketch (binding contract for downstream worksets)

```rust
// crates/oxvba-debug/src/lib.rs

use std::sync::Arc;

pub use core::{DebugSessionCore, DebugCoreConfig, DebugCoreRunResult};
pub use records::{DebugBreakpointRecord, DebugWatchRecord, DebugEvaluationRequest,
                  DebugSessionCommandStatus};
pub use views::{DebugPauseView, DebugBreakpointView, DebugWatchView,
                DebugFrameView, DebugValueView, DebugStopReasonView,
                DebugRunResultView, DebugExitView};
pub use events::{DebugEvent, DebugEventReceiver};
pub use errors::{DebugAttachError, DebugError};
pub use config::{DebugAttachConfig, DebugComApartment, DebugEventChannelMode,
                 DebugStartMode};

pub fn prepare_debug_session_core(
    engine: Arc<Engine>,
    manifest: ProjectManifest,
    config: DebugCoreConfig,
) -> Result<DebugSessionCore, DebugAttachError>;

pub fn attach_debug_session(
    engine: Arc<Engine>,
    manifest: ProjectManifest,
    config: DebugAttachConfig,
) -> Result<DebugSessionAttach, DebugAttachError>;

pub struct DebugSessionAttach {
    pub handle: DebugSessionHandle,
    // Created before the worker emits startup events, so attach-time
    // ModuleLoaded / ThreadStarted / Stopped events are observable.
    pub events: DebugEventReceiver,
}

#[derive(Clone)]
pub struct DebugSessionHandle { /* Arc<HandleInner> */ }

// No unsafe Send/Sync impl is permitted for the handle. The handle is Send +
// Sync only if its concrete fields (channels, atomics, mutex-protected join
// handle, immutable ids) make it so. `DebugSessionCore` is made explicitly
// !Send / !Sync (for example with a private `PhantomData<Rc<()>>`) and is
// constructed inside the worker thread.

impl DebugSessionHandle {
    // --- stepping
    pub fn start(&self) -> Result<DebugRunResultView, DebugError>;
    pub fn step_into(&self) -> Result<DebugRunResultView, DebugError>;
    pub fn step_over(&self) -> Result<DebugRunResultView, DebugError>;
    pub fn step_out(&self)  -> Result<DebugRunResultView, DebugError>;
    pub fn continue_execution(&self) -> Result<DebugRunResultView, DebugError>;
    // Cooperative pause is not part of v1; do not publish a stub that always
    // fails. Add it when the VM can actually pause a running evaluation.

    // --- breakpoints
    pub fn set_source_breakpoint(&self, module: &str, file_line: u32, enabled: bool)
        -> Result<DebugBreakpointView, DebugError>;
    pub fn set_breakpoint_enabled(&self, id: &DirectHostBreakpointId, enabled: bool)
        -> Result<DebugBreakpointView, DebugError>;
    pub fn clear_source_breakpoint(&self, id: &DirectHostBreakpointId)
        -> Result<(), DebugError>;
    pub fn breakpoints(&self) -> Result<Vec<DebugBreakpointView>, DebugError>;

    // --- watches
    pub fn add_watch(&self, expression: &str)
        -> Result<DebugWatchView, DebugError>;
    pub fn update_watch(&self, id: &DirectHostWatchId, expression: &str)
        -> Result<DebugWatchView, DebugError>;
    pub fn remove_watch(&self, id: &DirectHostWatchId) -> Result<(), DebugError>;
    pub fn evaluate_watches(&self) -> Result<Vec<DebugWatchView>, DebugError>;

    // --- frame / value inspection
    pub fn current_pause(&self) -> Result<Option<DebugPauseView>, DebugError>;
    pub fn stack_frames(&self) -> Result<Vec<DebugFrameView>, DebugError>;
    pub fn frame_locals(&self, frame: &DirectHostStackFrameId)
        -> Result<Vec<DebugValueView>, DebugError>;
    pub fn evaluate(&self, frame: Option<&DirectHostStackFrameId>, expression: &str)
        -> Result<DebugValueView, DebugError>;

    // --- events
    pub fn subscribe(&self) -> DebugEventReceiver;

    // --- lifecycle
    pub fn session_id(&self) -> &DirectHostDebugSessionId;
    pub fn detach(self) -> Result<(), DebugError>;  // consumes self

    // --- async wrappers (feature = "tokio")
    #[cfg(feature = "tokio")]
    pub async fn step_into_async(&self) -> Result<DebugRunResultView, DebugError>;
    // ... and so on for every command above.
}

pub enum DebugEvent {
    Stopped {
        seq: u64,
        session_id: DirectHostDebugSessionId,
        reason: DebugStopReasonView,
        thread_id: Option<u32>,            // future; today always None / 0
        frame_id: DirectHostStackFrameId,
        location: Option<DebugSourceLocationView>,
    },
    Output {
        seq: u64,
        session_id: DirectHostDebugSessionId,
        channel: DebugOutputChannel,        // Stdout | Stderr | Host (Debug.Print)
        text: String,
    },
    Continued {
        seq: u64,
        session_id: DirectHostDebugSessionId,
        all_threads_continued: bool,        // DAP-friendly
    },
    Exited {
        seq: u64,
        session_id: DirectHostDebugSessionId,
        exit_code: Option<i32>,
    },
    BreakpointChanged {
        seq: u64,
        session_id: DirectHostDebugSessionId,
        change: DebugBreakpointChangeKind,  // Added | Changed | Removed
        breakpoint: DebugBreakpointView,
    },
    ModuleLoaded {
        seq: u64,
        session_id: DirectHostDebugSessionId,
        module: DebugModuleView,
    },
    ThreadStarted {                         // forward-looking; today always for the primary thread
        seq: u64,
        session_id: DirectHostDebugSessionId,
        thread_id: u32,
    },
}

pub struct DebugAttachConfig {
    pub com_apartment: DebugComApartment,   // Sta (Windows default) | Mta | None
    pub event_channel: DebugEventChannelMode, // default Bounded(256); opt-in Unbounded
    pub start_mode: DebugStartMode,         // Manual | StopOnEntry
    pub output_capture: DebugOutputCaptureMode,
}

pub enum DebugError {
    NotPaused,
    UnknownBreakpoint(DirectHostBreakpointId),
    UnknownWatch(DirectHostWatchId),
    UnknownFrame(DirectHostStackFrameId),
    Evaluation { expression: String, message: String },
    Completed,
    UnsupportedCommand(&'static str),
    OutstandingHandles { count: usize },
    SessionAlreadyDetached,
    WorkerFailed { stage: &'static str, message: String },
    Internal(String),                       // unexpected; always recorded
}
```

All view types are `Send + Sync + Clone + Debug + serde::Serialize` so they
flow trivially through DAP JSON, Tauri IPC, or any other transport.

## Threading & COM apartment model

- Each `attach_debug_session` spawns exactly **one** worker thread.
- Callers pass `Arc<Engine>`, not `&Engine`. The worker constructs and owns
  `DebugSessionCore` inside the worker thread by calling `oxvba-host` runtime
  preparation APIs. No borrowed `Engine` lifetime is smuggled into a
  `'static` handle.
- `DebugSessionCore` owns `Arc<Engine>`, `ProjectManifest`,
  `ProjectRuntimeSession`, breakpoint/watch registries, source maps, and the
  last pause state. It is explicitly `!Send` / `!Sync`; only transport-ready
  view values and command replies cross the worker boundary.
- On Windows, if `config.com_apartment == Sta`, the worker calls
  `CoInitializeEx(NULL, COINIT_APARTMENTTHREADED)` on startup and
  `CoUninitialize` on shutdown. This is the **correct** placement of COM
  apartment management - the runtime knows its semantics, consumers don't have
  to. (Mta and None are also supported; tests cross-platform via None.)
- The v1 STA path is valid for synchronous in-apartment COM work. Any future
  cross-apartment callback, connection-point event sink, or COM event pump lane
  must replace blocking channel waits with a pumped wait loop
  (`MsgWaitForMultipleObjects`-style on Windows) so STA messages are serviced.
  Until that lands, hosts that need COM callbacks should use `Mta` or `None`
  unless they provide an external pump around the worker.
- Commands marshal through one sync worker channel
  (`crossbeam_channel::Sender<DebugCommand>`). Sync methods block on the reply;
  async methods send to the same worker and await a tokio oneshot on the
  caller side. There is one serialization path, not a separate async worker.
- Events are broadcast through an internal event hub. The initial receiver
  returned in `DebugSessionAttach` is registered before the worker emits
  startup events. Later `handle.subscribe()` calls receive future events only;
  replay is **not** automatic unless a future explicit replay feature is added.
- `DebugAttachConfig::default()` uses `DebugEventChannelMode::Bounded(256)`.
  Bounded mode is drop-oldest with a typed lag/drop signal for slow
  subscribers. `Unbounded` is available for controlled tests or embeddings that
  explicitly own the memory-growth risk.
- Every event carries a monotonic `seq`. When a command both emits an event and
  returns a reply, the worker sends the event first, then the reply. Tests
  assert that ordering by `seq` and by receiver availability, not by scheduler
  timing assumptions.
- Concurrent caller commands serialize at the channel; the worker processes
  them sequentially. This matches the VM's single-threaded execution model.
- Handle clones share one worker via `Arc<HandleInner>`. Last `Arc` drop
  triggers worker shutdown (clean detach).

## Testing strategy (anchors B02 + each subsequent bead)

Tests live at five levels, each owned by a specific bead's deliverables:

### Layer 1 - Core (`oxvba-debug::DebugSessionCore`) regression
Existing OxVba `DebugSession` tests move from `oxvba-host` to
`oxvba-debug/tests/core_*.rs` during the core-move bead (B03). Their semantics
are pinned: the moved core must preserve current stepping, breakpoint, watch,
pause, and retained-Variant behavior before handle work is layered on top.
`oxvba-host` keeps only runtime-preparation and debug-runtime primitive tests.

### Layer 2 - Handle behavior (`oxvba-debug/tests/handle_*.rs`)
Each handle method has a happy-path test that exercises:
- attach -> command -> assert returned view matches expectation
- assert the returned view types are `Send + Sync` (via `static_assertions`)
- assert handle methods serialize correctly across multiple caller threads
  (spawn N threads, each issues commands; record observed serialization).
- assert normal completion returns `DebugRunResultView::Exited`, not an error
  and not a fake pause.

### Layer 2 - Events (`oxvba-debug/tests/events_*.rs`)
For every event variant, drive an action that should produce it and assert
the subscriber receives the right event in the right order. Test:
- initial receiver returned from attach sees startup events
- late subscriber sees only future events
- multiple subscribers see the same stream
- slow subscriber doesn't block worker; bounded mode reports lag/drop state
- subscriber drop is safe; worker continues
- event ordering: worker assigns `Stopped` a lower `seq` before the command
  response completes

### Layer 2 - Lifecycle (`oxvba-debug/tests/lifecycle_*.rs`)
- attach failure (bad manifest, compile error): worker doesn't start; handle
  not returned; resources cleaned
- attach success then explicit `detach()` on the last handle clone: worker
  joins, no leak (verified by thread count and resource counters)
- attach then drop all handle clones: implicit detach
- attach, command in flight, then drop handle: in-flight command returns
  `SessionAlreadyDetached`
- worker panic propagates: all subsequent handle calls return
  `WorkerFailed { stage, ... }`; handle is unusable but doesn't deadlock
- re-attach after detach: independent session, fresh ids, no contamination

### Layer 2 - Concurrency / threading (`oxvba-debug/tests/concurrency_*.rs`)
- N caller threads issuing commands concurrently; observed serialization
  matches channel arrival order; no data races (verified under
  `RUSTFLAGS="-Z sanitizer=thread"` in CI)
- 100 concurrent sessions: each independent; no cross-talk
- compile-fail/static assertion tests:
  `assert_impl_all!(DebugSessionHandle: Send, Sync, Clone)`;
  `assert_not_impl_any!(DebugSessionCore: Send, Sync)`. No unsafe Send/Sync
  impl for the handle is allowed.

### Layer 2 - COM apartment (`oxvba-debug/tests/com_apartment_*.rs`)
- `com_apartment = Sta` on Windows: worker successfully initializes STA;
  worker-thread report verifies STA; CoUninitialize on shutdown
- `com_apartment = Mta`: worker-thread report verifies MTA
- `com_apartment = None`: no COM call (test runs on Linux too)
- multiple sessions: each worker independent apartment

### Layer 2 - Source mapping (`oxvba-debug/tests/source_map_*.rs`)
- bare source: editor lines map identity to runtime lines
- `Attribute ...`, `Option Private Module`, and class `Implements` lines are
  dropped and mapped as non-executable user lines
- `Option Explicit`, `Option Compare`, `Option Base`, blank lines, and comment
  lines are preserved with correct user-line identity
- compiler-inserted helper lines are marked non-user and never surface as
  editor locations
- property: `runtime_to_file(file_to_runtime(N)) == N` for executable user
  lines in the proc body
- cross-module: each module computed independently against its own source
- edge: empty module
- edge: module with only preamble (no proc)

### Layer 2 - Async surface (`oxvba-debug/tests/async_*.rs`, feature = "tokio")
- `step_into_async()` returns a Future resolving to the same
  `DebugRunResultView` as the sync `step_into()`
- concurrent async commands (5 spawned tasks) serialize at the worker
- cancellation: dropping the future before completion does not break the
  worker (next sync command succeeds)
- async event subscription uses the tokio wrapper over the same event sequence

### Layer 2 - Property / replay (`oxvba-debug/tests/property_*.rs`)
- proptest: random sequences of (attach, set_bp, step*, continue, watch ops,
  detach) - assert no panic, no deadlock, every error has a typed `DebugError`
- snapshot test: a fixed sequence on a fixed fixture yields the same
  serialized event/view log (regression pin)

### Layer 2 - Stress / performance (`oxvba-debug/benches/`, `oxvba-debug/tests/stress_*.rs`)
- 100 sequential sessions (attach+detach loop); assert no fd / thread leak
- 1000 sequential commands per session: assert latency stays bounded
- benchmark: `step_into` round-trip latency (target: < 1 ms for the
  thin-slice fixture); breakpoint-set round-trip (target: < 5 ms)
- memory-leak detection via `cargo-valgrind` (Linux CI) or process-RSS
  delta over 10k iterations

### End-to-end / reference flows (`oxvba-debug/tests/scenarios_*.rs`)
Two canonical scenarios that mirror real consumers, ensuring the contract
covers their needs:

**Scenario A - DAP-style flow** (mirrors what `oxvba-dap` will do):
1. attach and retain the initial `DebugEventReceiver`
2. setBreakpoints(Module1, [line 6])
3. on event Stopped(reason=Entry): assert frame_id, location, monotonic seq
4. continueExecution
5. on event Stopped(reason=Breakpoint): assert breakpoint id, line
6. stackTrace -> assert frame list with real ids
7. scopes(frame_id) -> not in this workset; locals via `frame_locals`
8. evaluate(frame_id, "answer") -> assert value
9. continueExecution
10. on event Exited: assert exit_code and command result `Exited`
11. detach

**Scenario B - OxIde cockpit flow** (mirrors what OxIde will do post-migration):
1. attach (project = thin-slice) and retain the initial event receiver
2. add_watch("answer")
3. set_source_breakpoint(Module1, 6, enabled=true)
4. continue_execution -> Stopped(Breakpoint) at line 6
5. step_over twice; assert current pause line advances
6. evaluate_watches -> assert "answer" value updated
7. set_breakpoint_enabled(bp, false); continue -> Exited (bp disabled, no stop)
8. detach

Both scenarios are stable and re-runnable; their evidence files become the
documentation of "this is what `oxvba-debug` does for real consumers."

## Beads

### B00 - Architecture design doc + cross-layer test catalog

Type: Design / governance

Goal:
  The crate's public API, layer boundaries, event taxonomy, COM apartment
  model, and complete cross-layer test catalog are written down and reviewed
  before any implementation begins.

Design:
  - `docs/spec/OXVBA_DEBUG_HANDLE_DESIGN.md`: full design doc covering layer
    diagram, public API (binding contract from this workset), event taxonomy,
    lifecycle, COM apartment policy, async surface, error model, source-map
    contract, downstream migration story (OxIde and `oxvba-dap` first), and
    the deliberate source-level migration away from `oxvba-host::debugger`.
  - `docs/spec/OXVBA_DEBUG_TEST_CATALOG.md`: the cross-layer test catalog
    (this document's "Testing strategy" section, expanded with concrete test
    names mapped to beads).

Tests:
  - None (design doc only).

Evidence:
  - The two design docs, reviewed and approved before B01.

Closure:
  - [ ] Design doc approved (public API frozen for B01-B15).
  - [ ] Test catalog approved (every test in B02-B15 traces to a catalog
        entry).

### B01 - `oxvba-debug` crate skeleton + workspace dep

Type: Infrastructure

Goal:
  Empty `oxvba-debug` crate exists in the workspace with dependencies on
  `oxvba-host`, `oxvba-vm`, `oxvba-runtime`, `oxvba-compiler`, `crossbeam-channel`,
  `serde`, and `static_assertions`; optional dependencies are feature-gated.
  `cargo build` and `cargo test` are green for the empty crate.

Design:
  - `crates/oxvba-debug/Cargo.toml`: `[package]`, deps, features
    (`tokio`, `proptest`, `bench`).
  - `crates/oxvba-debug/src/lib.rs`: empty module skeleton matching the
    design doc's module layout.
  - Add to workspace `Cargo.toml`.

Tests:
  - `cargo build -p oxvba-debug` (no behavior; just shape).
  - `cargo test -p oxvba-debug` (passes with zero tests).

Evidence:
  - Crate compiles in CI; trivial smoke `pub use` of the (empty) public
    types resolves.

Closure:
  - [ ] Crate skeleton in place; CI green.

### B02 - Test framework + scenario catalog implementation

Type: Infrastructure (testing)

Goal:
  Before any feature code lands, the test harness, the fixture catalog (one
  multi-module project with known statement line numbers, one bare-source
  project, one COM-bearing project for apartment tests), and the
  `static_assertions`-based compile-fail tests for Send/Sync are in place.
  The cross-layer test catalog (`OXVBA_DEBUG_TEST_CATALOG.md`) is encoded as
  empty `#[test]` stubs with `unimplemented!()` bodies, one per catalog
  entry, so progress can be tracked by green-ifying stubs.

Design:
  - `crates/oxvba-debug/tests/_shared.rs`: fixture loaders, project writers,
    common helpers (no test logic).
  - `crates/oxvba-debug/tests/fixtures/`: canonical fixtures (`thin_slice`,
    `multi_module_walkthrough`, `com_dispatch_smoke`, `bare_no_preamble`).
  - `crates/oxvba-debug/tests/handle_send_sync.rs`: compile-fail / type
    assertions (`assert_impl_all`, `assert_not_impl_any`) proving the handle is
    `Send + Sync` while `DebugSessionCore` is not.
  - Empty test stubs in each `handle_*.rs`, `events_*.rs`, `lifecycle_*.rs`,
    `concurrency_*.rs`, `com_apartment_*.rs`, `source_map_*.rs`,
    `async_*.rs`, `property_*.rs`, `stress_*.rs`, `scenarios_*.rs` file
    enumerating every catalog item.

Tests:
  - The Send/Sync compile-fail/static-assertion tests pass against the
    (empty) handle/core skeleton from B01.
  - All other tests panic on `unimplemented!()` and are gated by
    `#[ignore]` until their implementing bead lands - so CI stays green while
    the catalog is visible in `cargo test --list`.

Evidence:
  - `cargo test -p oxvba-debug -- --list` enumerates every catalog entry.
  - `OXVBA_DEBUG_TEST_CATALOG.md` updated with each test's owning bead.

Closure:
  - [ ] Fixture catalog in place.
  - [ ] Cross-layer test catalog encoded as enumerable stubs.
  - [ ] Send/Sync compile-fail/static-assertion tests green.
  - [ ] CI green (ignored stubs counted, not failing).

### B03 - Move the raw debug core and domain types into `oxvba-debug`

Type: Refactor / architecture

Goal:
  The existing borrow-bound `oxvba_host::DebugSession<'engine>` implementation
  becomes owned `oxvba_debug::DebugSessionCore`. All current debugger-domain
  types (`DebugWatchRecord/Evaluation/Status`,
  `DebugBreakpointRecord/BindingStatus/UnresolvedReason`,
  `DebugVariantPauseState` renamed to `DebugPauseState`,
  `DebugFrameVariant*`, core run results, `DebugSessionError`,
  `DebugEvaluationRequest`, `DebugVariantEvaluationResult`,
  `DebugSessionCommandStatus`) move with it. `oxvba-host` no longer contains a
  public debugger facade, and every existing debugger semantic test passes
  from the new crate.

Design:
  - `DebugSessionCore` owns `Arc<Engine>`, `ProjectManifest`, and
    `ProjectRuntimeSession`; it does not borrow `Engine`.
  - `DebugSessionCore` is explicitly `!Send` / `!Sync` so it cannot be moved
    out of the worker thread by accident.
  - `oxvba-host` exposes only the minimal debug-runtime primitive API needed by
    the moved core: start / continue / step / snapshot / set-breakpoints /
    procedure metadata access over `ProjectRuntimeSession`.
  - `Engine::prepare_debug_session` is removed from `oxvba-host`; the raw
    replacement is `oxvba_debug::prepare_debug_session_core(Arc<Engine>, ...)`.
    This is a deliberate source migration to keep the crate graph acyclic.
  - No semantic change to stepping, watch evaluation, breakpoint binding, or
    retained Variant pause data.

Tests:
  - Current `oxvba-host` debugger unit tests move to
    `oxvba-debug/tests/core_*.rs` and pass with equivalent assertions.
  - `oxvba-host` retains tests for the new low-level runtime primitive API.
  - `assert_not_impl_any!(DebugSessionCore: Send, Sync)` is green.

Evidence:
  - `cargo test -p oxvba-host` green.
  - `cargo test -p oxvba-debug core_` green for the core-move regression.

Closure:
  - [ ] `DebugSessionCore` and all listed debug-domain types live in
        `oxvba-debug`.
  - [ ] `oxvba-host` has no dependency on `oxvba-debug` and no public
        `oxvba-host::debugger` facade.
  - [ ] No semantic regression in the moved core.

### B04 - Public view DTOs + Send/Sync projection layer

Type: Feature

Goal:
  `oxvba-debug::views` exposes Send + Sync + Clone + serde::Serialize view
  types (`DebugPauseView`, `DebugBreakpointView`, `DebugWatchView`,
  `DebugFrameView`, `DebugValueView`, `DebugSourceLocationView`,
  `DebugStopReasonView`, `DebugModuleView`, `DebugRunResultView`,
  `DebugExitView`) that project the internal records / pause state into
  transport-ready DTOs - the canonical shape every consumer reads.

Design:
  - Each view excludes raw `Variant` (which is !Send / non-serializable);
    Variant values are projected to `DebugValueView { display_text, type_label,
    kind, raw_repr (optional Base64 bytes) }`.
  - Free function `view::pause_view_from_core(&DebugPauseState,
    &DebugSourceMap) -> DebugPauseView` (and similarly for run result,
    breakpoint, watch).

Tests:
  - For each view, a round-trip through serde_json yields the same value.
  - For each view, `assert_impl_all!(View: Send, Sync, Clone, Debug,
    serde::Serialize, serde::Deserialize)`.
  - Projection tests against the fixture pause states from B02.

Evidence:
  - View module documented; serde round-trip tests green.

Closure:
  - [ ] Every view type Send + Sync + Clone + serde-able.
  - [ ] Projections from core types pinned by tests.

### B05 - `DebugSessionHandle` skeleton + worker + command marshalling

Type: Feature

Goal:
  `attach_debug_session(Arc<Engine>, ...)` spawns a worker that constructs and
  owns `DebugSessionCore`; the returned `DebugSessionHandle` is Send + Sync,
  cheap to clone, and exposes the synchronous command surface (every stepping /
  breakpoint / watch / inspect method from the design doc). Commands marshal
  via `crossbeam_channel`; responses return typed replies including normal
  completion (`DebugRunResultView::Exited`).
  **No events yet** (B06 + B07).

Design:
  - `handle.rs`: `DebugSessionHandle { inner: Arc<HandleInner> }`; no unsafe
    Send/Sync impl; `Drop` for `HandleInner` signals worker shutdown.
  - `worker.rs`: worker loop owns the core, processes `DebugCommand` enum,
    sends `Result<DebugReply, DebugError>` via oneshot.
  - `command.rs`: `DebugCommand` enum with one variant per public method.
  - Every handle method: send command, await oneshot, return view.

Tests (from B02 catalog, now implemented):
  - `handle_step_into.rs`, `handle_step_over.rs`, `handle_step_out.rs`,
    `handle_continue.rs`: each calls the method on a fresh handle and
    asserts the returned `DebugRunResultView` matches the equivalent
    `DebugSessionCore::start + step_*` flow.
  - `handle_breakpoint_set.rs`, `handle_breakpoint_toggle.rs`,
    `handle_breakpoint_clear.rs`: each exercises real OxVba binding semantics.
  - `handle_watch_*.rs`: add/update/remove/evaluate.
  - `handle_inspect.rs`: current_pause, frame_locals, evaluate.
  - `handle_completion.rs`: continuing past completion returns
    `DebugRunResultView::Exited`, and later stepping returns a typed
    `DebugError::Completed`.
  - `concurrency_serialization.rs`: 8 caller threads issuing commands;
    response order matches send order at the channel.

Evidence:
  - Every handle method has a green happy-path test.
  - Concurrency test asserts serialization.

Closure:
  - [ ] Every public command method implemented and pinned by tests.
  - [ ] Handle Send + Sync verified by B02 static assertions without unsafe
        Send/Sync.
  - [ ] No data races (thread-sanitizer pass).

### B06 - Event taxonomy + subscription stream

Type: Feature

Goal:
  `DebugEvent` enum (Stopped / Output / Continued / Exited / BreakpointChanged
  / ModuleLoaded / ThreadStarted) is defined; `attach_debug_session` returns an
  initial `DebugEventReceiver` that is subscribed before startup events, and
  `handle.subscribe() -> DebugEventReceiver` returns future events for later
  subscribers. The channel mode defaults to `Bounded(256)` with explicit
  drop-oldest / typed lag semantics; `Unbounded` is opt-in for controlled
  embeddings that accept memory-growth risk.

Design:
  - `events.rs`: `DebugEvent` enum + `DebugEventReceiver` over a small
    sync-first event hub. With `tokio`, async subscribers get a tokio-friendly
    wrapper over the same sequence stream.
  - The worker holds the broadcast `Sender` and emits events at known points
    (B07 wires the emission sites).
  - Every event has monotonic `seq` assigned by the worker.
  - `DebugAttachConfig::default().event_channel ==
    DebugEventChannelMode::Bounded(256)` is part of the public contract.

Tests:
  - `events_initial_receiver_from_attach.rs`: receiver returned from attach
    observes attach-time `ModuleLoaded`, `ThreadStarted`, and entry `Stopped`
    events.
  - `events_late_subscriber_future_only.rs`: `handle.subscribe()` after attach
    receives future events only.
  - `events_multi_subscriber.rs`: 3 subscribers receive identical streams.
  - `events_slow_subscriber_bounded.rs`: bounded channel + slow subscriber
    drops oldest with a typed lag/drop signal, worker not blocked.
  - `events_default_channel_mode.rs`: default config uses bounded capacity 256.
  - `events_subscriber_drop_safe.rs`: subscriber dropped mid-stream; worker
    continues; remaining subscribers unaffected.
  - `events_ordering.rs`: event `seq` and receiver availability prove the
    worker emitted `Stopped` before completing the command response.

Evidence:
  - Event delivery tests green; ordering pinned.

Closure:
  - [ ] `subscribe()` works; multi-consumer semantics correct.
  - [ ] Initial attach receiver, late subscribers, and configurable channel
        modes tested.

### B07 - Worker emits events at the right times

Type: Feature

Goal:
  The worker emits the right `DebugEvent` at every relevant state
  transition: `Stopped` after entry pause / step / breakpoint hit;
  `Continued` on resume; `Exited` on completion; `Output` for `Debug.Print`
  and runtime stdio; `BreakpointChanged` for set_enabled / clear;
  `ModuleLoaded` at attach for each module; `ThreadStarted` for the
  primary VBA thread.

Design:
  - Each handle command that changes state has a corresponding event emission
    in the worker after the state change.
  - `Output` is captured through an explicit debug output tap introduced at the
    host/VM boundary in this bead. It must observe `Debug.Print`, console
    stdout/stderr, and host diagnostic output without replacing or suppressing
    the embedding's existing callbacks.
  - The Variant `Stopped` location uses the same line mapping as
    `current_pause`.

Tests (each event variant has a "what produces it" test):
  - `events_stopped_on_entry.rs`: attach -> Stopped(Entry).
  - `events_stopped_on_breakpoint.rs`: continue to breakpoint -> Stopped(Breakpoint).
  - `events_stopped_on_step.rs`: step_into -> Stopped(Step).
  - `events_continued.rs`: continue_execution -> Continued, followed by
    Stopped or Exited as appropriate.
  - `events_exited.rs`: continue past last instruction -> Exited.
  - `events_output_debug_print.rs`: Debug.Print "hello" -> Output(Host, "hello").
  - `events_breakpoint_changed.rs`: add / enable-toggle / remove each produce
    BreakpointChanged with the correct change kind.
  - `events_module_loaded.rs`: attach to 2-module project -> 2 ModuleLoaded.

Evidence:
  - Every event variant produced by a real action; pinned by tests.

Closure:
  - [ ] Every event variant emitted at the right time.
  - [ ] Subscriber tests in B06 still green after wiring.

### B08 - Compiler source maps in the projection layer

Type: Feature (compiler + debug; compiler-emitted source maps consumed by
`oxvba-debug`)

Goal:
  The handle's view projections (file_line in `DebugSourceLocationView`,
  in `DebugBreakpointView`, in `DebugPauseView.current_location`) carry
  **editor file lines**. Inputs (`set_source_breakpoint(file_line)`) accept
  editor file lines and the worker converts to the VM/runtime line basis using
  compiler-emitted source maps before binding breakpoints. This bead explicitly
  includes `oxvba-compiler` work: `CompiledProject` must carry structured
  source-map data produced by the same lowering path that emits bytecode and
  procedure metadata.

Design:
  - Add compiler-owned source-map data to `CompiledProject`: for each module,
    map editor source lines to lowered/runtime lines and back, including
    dropped lines (`Attribute ...`, `Option Private Module`, class
    `Implements`), preserved lines (`Option Explicit`, `Option Compare`,
    `Option Base`, comments, blanks), and compiler-inserted helper lines.
  - `oxvba-debug::source_map::DebugSourceMap` wraps the compiler map with
    debugger-specific helpers: `file_to_runtime(module, file_line)`,
    `runtime_to_file(module, runtime_line)`, and
    `nearest_executable_file_line(module, file_line)` where DAP-style
    breakpoint binding needs a stable unresolved/bound explanation.
  - The mapping is not a hardcoded preamble offset. It is derived from the same
    lowering pass that produces bytecode and procedure metadata.
  - Wired into view projection (B04) and command marshalling (B05/B07).
  - OxIde's `crates/oxide-oxvba` mapping (currently lives there) is the
    direct ancestor; this bead supersedes it. OxIde removes its copy in
    its follow-up migration bead (out of scope here).

Tests:
  - All `source_map_*.rs` stubs from B02 implemented:
    bare identity, Attribute-only dropped, Attribute+Option Explicit where
    only Attribute is dropped, Attribute+Option Explicit+Option Compare Text+
    Option Base 1 where the option lines are preserved, Option Private Module
    dropped, class Implements dropped, blanks preserved, comments preserved,
    compiler-inserted helper lines marked non-user, inverse property
    `runtime_to_file(file_to_runtime(N)) == N` for mapped executable user
    lines, multi-module independent, edge cases (empty module, preamble-only
    module).
  - Snapshot: thin-slice statements at file lines 6/7/8 (Dim / `=` /
    Debug.Print) bind through the handle when caller passes file lines.

Evidence:
  - `cargo test -p oxvba-compiler` green for source-map emission tests.
  - All source-map catalog tests green.
  - Snapshot of bound vs. unresolved by file line.

Closure:
  - [ ] `CompiledProject` exposes structured source-map data for every module.
  - [ ] Mapping applied uniformly on input + output paths.
  - [ ] Every catalog scenario green.

### B09 - COM apartment management for the worker

Type: Feature

Goal:
  `DebugAttachConfig::com_apartment` (Sta / Mta / None) is honored on the
  worker: on Windows, the worker calls `CoInitializeEx` with the matching
  flag on startup and `CoUninitialize` on shutdown. Multiple sessions are
  independent. The cross-platform tests run with `None`.

Design:
  - `com_apartment.rs`: thin wrappers around the Windows COM APIs,
    `#[cfg(windows)]` gated; the no-op `None` path works on all platforms.
  - The worker initializes COM before constructing the core (so
    IDispatch creation inside the core sees the right apartment).
  - V1 STA support is scoped to synchronous in-apartment COM work. This bead
    documents, but does not implement, the required future pumped wait loop for
    cross-apartment callbacks / COM event sinks. That future lane must not
    broaden STA claims until the worker loop services Windows messages while
    waiting for commands.
  - Tests verify the apartment from inside the worker via a test-only
    `DebugCommand::ReportWorkerApartment` / public test helper. A test-thread
    `CoGetApartmentType` call alone is not acceptable evidence because COM
    apartment state is thread-local.
  - Teardown is best-effort but logged on failure.

Tests:
  - `com_apartment_sta_init.rs` (Windows): worker reports STA after init.
  - `com_apartment_mta_init.rs` (Windows): worker reports MTA after init.
  - `com_apartment_none.rs` (cross-platform): no COM call; works on Linux.
  - `com_apartment_multi_session.rs` (Windows): two sessions, two
    independent apartments.

Evidence:
  - COM init/uninit verified from the worker thread; no apartment leaks across
    sessions.

Closure:
  - [ ] STA / MTA / None all supported.
  - [ ] STA support statement explicitly bounded to sync in-apartment COM until
        a pumped wait loop lands.
  - [ ] Cross-platform CI (Windows + Linux at least) covers the
        apartment-relevant subset on each.

### B10 - Async surface (`*_async` variants, feature = "tokio")

Type: Feature

Goal:
  For every sync handle method there's an `*_async` variant returning the same
  typed output as the sync method. Async subscribers can use a tokio-friendly
  receiver over the same sequenced event stream. Sync callers (OxIde in Tauri
  commands) and async callers (`oxvba-dap` server) both work from the same
  handle.

Design:
  - `async_handle.rs`, `#[cfg(feature = "tokio")]`: each method sends the
    command into the same worker channel and awaits a tokio oneshot.
  - The worker stays sync (no internal tokio runtime); async is purely a
    caller-side ergonomic.
  - Cancellation: dropping the future drops the oneshot receiver; the worker
    discards the result when it finishes processing (no panic; logged).

Tests (`async_*.rs`, only run with `--features tokio`):
  - `async_step_into.rs`: `step_into_async().await` equals `step_into()` for
    `DebugRunResultView`.
  - `async_concurrent.rs`: 5 spawned tasks concurrent; serialized at worker.
  - `async_cancellation.rs`: drop the future; next sync command on the same
    handle still works; worker not poisoned.
  - `async_event_stream.rs`: tokio wrapper receives the same sequenced events
    as the sync receiver.

Evidence:
  - All async tests green with `--features tokio`.

Closure:
  - [ ] Async wrappers exist for every command.
  - [ ] Cancellation safe; no worker poisoning.

### B11 - Lifecycle, detach, and error propagation

Type: Feature

Goal:
  Attach failure / detach / drop-all-handles / mid-session worker error are
  all handled deterministically. Resources don't leak. Errors propagate as
  typed `DebugError`.

Design:
  - `attach_debug_session` returns `Err(DebugAttachError)` if the compile /
    prepare fails; no worker is left running.
  - `handle.detach()` consumes one handle clone, sends a Shutdown command, and
    joins the worker only when it owns the last strong reference. If other
    clones exist it returns `DebugError::OutstandingHandles { count }` rather
    than pretending the session is detached.
  - Last `Arc<HandleInner>` drop triggers Shutdown automatically (idempotent).
  - In-flight commands when Shutdown arrives return
    `Err(SessionAlreadyDetached)`.
  - Worker panic: caught by a top-level `catch_unwind`, recorded in
    `HandleInner::failure_state`; all future handle calls return
    `Err(WorkerFailed { stage, message })`.

Tests (`lifecycle_*.rs` from B02, implemented):
  - `lifecycle_attach_failure.rs`: bad manifest -> error; no worker thread
    leaks (verified via thread count delta).
  - `lifecycle_explicit_detach.rs`: detach() joins cleanly when called on the
    last handle clone; returns `OutstandingHandles` when clones remain.
  - `lifecycle_drop_implicit_detach.rs`: dropping all handles joins worker.
  - `lifecycle_drop_with_command_in_flight.rs`: in-flight command returns
    SessionAlreadyDetached without panic.
  - `lifecycle_worker_panic.rs`: inject a worker panic (test-only command);
    subsequent calls return WorkerFailed; no deadlock.
  - `lifecycle_reattach.rs`: detach, attach again; fresh session ids; no
    state leak.

Evidence:
  - Resource counters (thread count, fd count) stable across many
    attach/detach cycles.

Closure:
  - [ ] All lifecycle scenarios pinned.
  - [ ] No leaked workers, fds, COM apartments.

### B12 - Property-based + snapshot replay tests

Type: Testing

Goal:
  proptest exercises random handle-command sequences against the
  multi-module fixture; assert no panics, all errors are typed, no
  deadlocks. A snapshot test pins one canonical sequence's serialized
  event log against a known-good baseline (regression pin).

Design:
  - `property_random_sequences.rs`: generator produces sequences of
    handle commands (with arbitrary but valid arguments); harness runs
    each; asserts safety invariants.
  - `property_snapshot.rs`: fixed sequence; serialized event log compared
    against committed baseline; mismatch = test failure (intentional, to
    catch unexpected semantic drift).

Tests:
  - `property_random_sequences.rs`
  - `property_snapshot.rs`

Evidence:
  - proptest minimum N runs (e.g. 256) green.
  - Snapshot file committed under `tests/snapshots/`.

Closure:
  - [ ] proptest green.
  - [ ] Snapshot baseline committed; regression sentinel in place.

### B13 - Concurrency / stress / performance benchmarks

Type: Testing + benches

Goal:
  100 concurrent sessions stable, 1k sequential commands per session,
  per-command latency bounded. Benchmarks committed for ongoing perf
  tracking.

Design:
  - `stress_concurrent_sessions.rs`: spawn N=100 sessions, each runs a
    short scenario, all complete; assert no thread/fd leak.
  - `stress_sequential_commands.rs`: 1000 step_overs in a row; assert
    bounded total time and no leak.
  - `benches/handle_latency.rs`: criterion benches for step_into,
    set_source_breakpoint, evaluate_watches.

Tests:
  - `stress_concurrent_sessions.rs`, `stress_sequential_commands.rs`.
  - Criterion benches publishable in CI.

Evidence:
  - Benchmark summaries committed under `docs/evidence/oxvba-debug/benchmarks/`.
    Raw `target/criterion/` output remains build output and is not committed.

Closure:
  - [ ] Stress tests stable in CI (3 runs without flake).
  - [ ] Benchmark baseline captured.

### B14 - Migration guide + downstream-impact handoff

Type: Documentation

Goal:
  Two handoff docs in the OxVba repo that downstream consumers (OxIde,
  future `oxvba-dap`) reference verbatim during their migration / build:

  1. `docs/HANDOFF_OXIDE_MIGRATE_TO_DEBUG_HANDLE.md` - for OxIde's
     `oxide-wf81` follow-up. "From `oxvba_host::DebugSession` to
     `oxvba_debug::DebugSessionHandle` step by step." Maps each existing
     OxIde adapter helper to its new handle method; retires
     stepPlan/breakpointPlan/watchExpressions; removes OxIde's line-mapping
     copy (now compiler-emitted and consumed through `oxvba-debug`).

  2. `docs/HANDOFF_OXVBA_DAP_FROM_DEBUG_HANDLE.md` - for the future
     `oxvba-dap` workset. "Build a DAP server on top of `DebugSessionHandle`."
     Maps each DAP request type to handle methods, each handle event to a
     DAP event.

Tests:
  - None (docs).

Evidence:
  - Both docs reviewed; ready for downstream worksets to consume.

Closure:
  - [ ] OxIde migration doc complete.
  - [ ] `oxvba-dap` builder's-guide doc complete.

### B15 - Acceptance: end-to-end scenarios + handoff

Type: Acceptance

Goal:
  The two reference scenarios (DAP-style flow and OxIde cockpit flow) run
  end-to-end against the implemented `oxvba-debug`; all B02-B13 tests green;
  the design doc, test catalog, and downstream handoffs are finalized.

Design:
  - `scenarios_dap_style_flow.rs`: scenario A from the test catalog.
  - `scenarios_oxide_cockpit_flow.rs`: scenario B from the test catalog.
  - Re-run the full `oxvba-debug` test matrix + `oxvba-host` regression on
    Windows + Linux CI.
  - Write `docs/HANDOFF_OXVBA_DEBUG_HANDLE_v1.md` summarizing what shipped,
    what's deferred (DAP, OxIde migration, future features), and the
    source-migration path from old `oxvba-host` debugger imports.

Tests:
  - Both scenario tests green.
  - Full crate test matrix green on Windows + Linux.

Evidence:
  - `docs/evidence/oxvba-debug/acceptance.{txt,json}`.
  - `docs/HANDOFF_OXVBA_DEBUG_HANDLE_v1.md`.

Closure:
  - [ ] Both reference scenarios green.
  - [ ] Full CI matrix green.
  - [ ] Handoff doc published.
  - [ ] OxIde unblocked to start `oxide-wf81` migration.
  - [ ] `oxvba-dap` unblocked to start its build.

## Migration / downstream impact

- **OxIde** (`OxIde/oxide-wf81`): rescope from "find a way to pin a !Send
  session" to "consume `DebugSessionHandle` in Tauri managed state."
  Removes ~1k LOC of replay/plan machinery; gains conditional breakpoints,
  hit counts, edit-and-continue substrate for free as those features land.
- **`oxvba-dap`** (separate planned workset): now a focused DAP-protocol
  implementation, not a debug-runtime reinvention. Each DAP request maps to
  a handle method; each handle event maps to a DAP event. Estimated
  ~1.5-3k LOC for a first-pass DAP server.
- **`oxvba-cli debug`** (future): an interactive REPL on the handle.
- **Test harnesses** (future): spin up handles concurrently to assert
  debug behavior at scale.
- **Future remote debugging**: DAP-over-TCP layered on `oxvba-dap`.

## Backward compatibility

This workset deliberately changes the crate ownership boundary instead of
preserving the old imports by re-export:

- `oxvba-host` does **not** depend on `oxvba-debug`; therefore
  `oxvba-host::debugger::*` re-exports are not available.
- Direct raw access moves from `Engine::prepare_debug_session(&manifest)` to
  `oxvba_debug::prepare_debug_session_core(Arc<Engine>, manifest, config)`.
- Consumer-facing integrations should prefer
  `oxvba_debug::attach_debug_session(...)` and `DebugSessionHandle`.
- Existing in-repo tests and downstream handoff docs must migrate imports in
  the same workset cycle so no required follow-up is left only in chat.

The compatibility guarantee is semantic, not import-path compatibility: the
moved core must preserve current debugger behavior while making `oxvba-debug`
the single public debug surface.

## Out-of-scope (anchored here to avoid scope creep)

- `oxvba-dap` crate + DAP protocol implementation (separate workset).
- OxIde's adapter rewrite (`OxIde/oxide-wf81` follow-up).
- A persistent debug worker pool (one worker per session is the model here;
  pooling is a future optimization).
- Conditional breakpoints, hit counts, exception breakpoints, edit-and-continue
  (the architecture must not preclude them but they're separate feature work).
- DAP-over-TCP / remote debugging.
- A debugger UI shell other than DnaOxIde.

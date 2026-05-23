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

This workset stands up a new `oxvba-debug` crate whose `DebugSessionHandle` is
the single consumer-facing API: `Send + Sync + 'static`, cheap to clone, owning
a worker thread that holds the `!Send` debug state, exposing the full step /
breakpoint / watch / inspect surface plus a first-class event stream
(`Stopped` / `Output` / `Continued` / `Exited` / ...). Multiple consumers
(OxIde in Tauri managed state, an in-process embedder, a future `oxvba-dap`
server) share one architecture instead of each inventing the same scaffolding.

The "right interface shape" is the three-layer model:

```
Layer 3   protocol adapters (oxvba-dap, oxvba-cli debug, ...)   <-- separate worksets
              |
              v uses
Layer 2   DebugSessionHandle (Send + Sync handle + worker + events)  <-- this workset
              |
              v wraps a worker thread that owns
Layer 1   DebugSessionCore (today's oxvba_host::DebugSession)   <-- mostly unchanged
```

`oxvba-dap` (Layer 3) is a distinct workset/crate layered on top of
`oxvba-debug`; this workset does **not** include DAP protocol work, but every
design decision here is made with DAP as a first-class consumer.

## Boundary

`oxvba-debug` owns:

- The consumer-facing `DebugSessionHandle` (Send + Sync, Arc-clone, lifecycle).
- The worker thread, command marshalling channel, event broadcast stream, and
  COM apartment management.
- The public debug-domain DTOs (`DebugPauseView`, `DebugBreakpointView`,
  `DebugWatchView`, `DebugFrameView`, `DebugValueView`, `DebugEvent`, ...) -
  these become the canonical types every consumer reads.
- The file-line <-> compiled-source-line mapping (currently in OxIde; moves
  here so all consumers benefit, not just OxIde).
- The async `*_async` surface (feature-gated tokio).
- The `attach_debug_session(engine, manifest, config) -> DebugSessionHandle`
  entry point.

`oxvba-host` keeps:

- `Engine`, `HostConfig`, `prepare_debug_session(&manifest)` (today's raw
  stateful core, used as the worker's internal core). The borrowing
  `DebugSession<'engine>` stays in `oxvba-host` to avoid a circular dep
  (`oxvba-debug` depends on `oxvba-host` for `Engine`; we don't want the
  reverse).
- `EmbeddedBuildRunHost`, `ProjectManifest` re-exports, all runtime-hosting
  surfaces unrelated to debug.

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

- `available`: exposed by `oxvba_host::DebugSession` today; re-projected in
  `oxvba-debug` with no semantic change.
- `new-in-this-workset`: net-new surface created by `oxvba-debug` (handle,
  events, async, etc.).
- `moved`: type or function lives in `oxvba-debug` after this workset; old
  location may re-export for one release before being deleted.

## Architecture: layer boundaries and crate dependencies

```
                +-------------------+
                |   oxvba-debug     |   <-- new
                |   (Layer 2 +      |
                |    public DTOs)   |
                +---------+---------+
                          |
                          v
                +---------+---------+
                |   oxvba-host      |   <-- unchanged
                |   (Engine, core,  |
                |    runtime)       |
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
Variant types in view projections), and `oxvba-compiler` (for
`ProjectManifest`). No reverse deps.

## Crate-relocation map

| Surface | Today (oxvba-host) | After this workset |
|---|---|---|
| `Engine`, `HostConfig`, `prepare_debug_session` | `oxvba-host` | `oxvba-host` (unchanged) |
| `DebugSession<'engine>` (the borrow-bound core) | `oxvba-host::debugger` | `oxvba-host::debugger` (kept; the worker holds one) |
| `DebugWatchRecord/Evaluation/Status` | `oxvba-host::debugger` | `oxvba-debug::watches` (re-exported from `oxvba-host` for one release) |
| `DebugBreakpointRecord/BindingStatus/UnresolvedReason` | `oxvba-host::debugger` | `oxvba-debug::breakpoints` (re-exported) |
| `DebugVariantPauseState`, `DebugFrameVariant*` | `oxvba-host::debugger` | `oxvba-debug::frames` (re-exported) |
| `HostDebugVariantRunResult`, `DebugSessionError`, `DebugEvaluationRequest`, `DebugVariantEvaluationResult`, `DebugSessionCommandStatus` | `oxvba-host::debugger` | `oxvba-debug::*` (re-exported) |
| Public view DTOs (Send+Sync projections) | n/a | `oxvba-debug::views` (new) |
| `DebugSessionHandle`, `attach_debug_session`, events, worker | n/a | `oxvba-debug` (new) |
| File-line <-> compiled-source-line mapping | (currently in OxIde adapter) | `oxvba-debug::line_mapping` (move) |

## Public API sketch (binding contract for downstream worksets)

```rust
// crates/oxvba-debug/src/lib.rs

pub use views::{DebugPauseView, DebugBreakpointView, DebugWatchView,
                DebugFrameView, DebugValueView, DebugStopReasonView};
pub use events::{DebugEvent, DebugEventReceiver};
pub use errors::{DebugAttachError, DebugError};
pub use config::{DebugAttachConfig, DebugComApartment, DebugEventChannelMode};

pub fn attach_debug_session(
    engine: &Engine,
    manifest: ProjectManifest,
    config: DebugAttachConfig,
) -> Result<DebugSessionHandle, DebugAttachError>;

#[derive(Clone)]
pub struct DebugSessionHandle { /* Arc<HandleInner> */ }

// SAFETY: the !Send DebugSessionCore lives exclusively on the worker thread,
// which is created in `attach_debug_session` and joined on shutdown. Callers
// communicate only through the channel; no field of the handle's Inner is ever
// dereferenced off the worker thread.
unsafe impl Send for DebugSessionHandle {}
unsafe impl Sync for DebugSessionHandle {}

impl DebugSessionHandle {
    // --- stepping
    pub fn step_into(&self) -> Result<DebugPauseView, DebugError>;
    pub fn step_over(&self) -> Result<DebugPauseView, DebugError>;
    pub fn step_out(&self)  -> Result<DebugPauseView, DebugError>;
    pub fn continue_execution(&self) -> Result<DebugPauseView, DebugError>;
    pub fn pause(&self) -> Result<DebugPauseView, DebugError>;  // future: cooperative pause

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
    pub async fn step_into_async(&self) -> Result<DebugPauseView, DebugError>;
    // ... and so on for every command above.
}

pub enum DebugEvent {
    Stopped {
        session_id: DirectHostDebugSessionId,
        reason: DebugStopReasonView,
        thread_id: Option<u32>,            // future; today always None / 0
        frame_id: DirectHostStackFrameId,
        location: Option<DebugSourceLocationView>,
    },
    Output {
        session_id: DirectHostDebugSessionId,
        channel: DebugOutputChannel,        // Stdout | Stderr | Host (Debug.Print)
        text: String,
    },
    Continued {
        session_id: DirectHostDebugSessionId,
        all_threads_continued: bool,        // DAP-friendly
    },
    Exited {
        session_id: DirectHostDebugSessionId,
        exit_code: Option<i32>,
    },
    BreakpointChanged {
        session_id: DirectHostDebugSessionId,
        breakpoint: DebugBreakpointView,
    },
    ModuleLoaded {
        session_id: DirectHostDebugSessionId,
        module: DebugModuleView,
    },
    ThreadStarted {                         // forward-looking; today always for the primary thread
        session_id: DirectHostDebugSessionId,
        thread_id: u32,
    },
}

pub struct DebugAttachConfig {
    pub com_apartment: DebugComApartment,   // Sta (Windows default) | Mta | None
    pub event_channel: DebugEventChannelMode, // Bounded(n) | Unbounded
    pub stop_on_entry: bool,
    pub source_policy: EmbeddedExecutionSourcePolicy,
}

pub enum DebugError {
    NotPaused,
    UnknownBreakpoint(DirectHostBreakpointId),
    UnknownWatch(DirectHostWatchId),
    UnknownFrame(DirectHostStackFrameId),
    Evaluation { expression: String, message: String },
    SessionAlreadyDetached,
    WorkerFailed { stage: &'static str, message: String },
    Internal(String),                       // unexpected; always recorded
}
```

All view types are `Send + Sync + Clone + Debug + serde::Serialize` so they
flow trivially through DAP JSON, Tauri IPC, or any other transport.

## Threading & COM apartment model

- Each `attach_debug_session` spawns exactly **one** worker thread.
- That worker owns the `Engine` (or holds it from the caller, lifetime-pinned
  via the handle's Arc) and the `DebugSessionCore<'engine>`. The core never
  crosses thread boundaries.
- On Windows, if `config.com_apartment == Sta`, the worker calls
  `CoInitializeEx(NULL, COINIT_APARTMENTTHREADED)` on startup and
  `CoUninitialize` on shutdown. This is the **correct** placement of COM
  apartment management - the runtime knows its semantics, consumers don't have
  to. (Mta and None are also supported; tests cross-platform via None.)
- Commands marshal through a `crossbeam_channel::unbounded::<DebugCommand>()`
  (sync) or `tokio::sync::mpsc` (async path). Each command carries a
  `oneshot::Sender<Result<DebugReply, DebugError>>` for the response.
- Events are broadcast through `tokio::sync::broadcast` (or `crossbeam`-based
  multi-producer multi-consumer fanout for the sync-only build). Subscribers
  joining late get future events; replay is **not** automatic (consumers that
  need an event log keep their own).
- Concurrent caller commands serialize at the channel; the worker processes
  them sequentially. This matches the VM's single-threaded execution model.
- Handle clones share one worker via `Arc<HandleInner>`. Last `Arc` drop
  triggers worker shutdown (clean detach).

## Testing strategy (anchors B02 + each subsequent bead)

Tests live at five levels, each owned by a specific bead's deliverables:

### Layer 1 - Core (`oxvba-host::DebugSession`) regression
Existing OxVba `DebugSession` tests stay in `oxvba-host/tests/`. After the
type-move bead (B03), they import from `oxvba-debug::*` or via re-export, but
their semantics are pinned: no behavior change at this layer is allowed by
this workset.

### Layer 2 - Handle behavior (`oxvba-debug/tests/handle_*.rs`)
Each handle method has a happy-path test that exercises:
- attach -> command -> assert returned view matches expectation
- assert the returned view types are `Send + Sync` (via `static_assertions`)
- assert handle methods serialize correctly across multiple caller threads
  (spawn N threads, each issues commands; record observed serialization).

### Layer 2 - Events (`oxvba-debug/tests/events_*.rs`)
For every event variant, drive an action that should produce it and assert
the subscriber receives the right event in the right order. Test:
- single subscriber sees all events from attach onward
- multiple subscribers see the same stream
- slow subscriber doesn't block worker (event channel either drops oldest or
  applies back-pressure; tested both modes)
- subscriber drop is safe; worker continues
- event ordering: Stopped before any subsequent command response

### Layer 2 - Lifecycle (`oxvba-debug/tests/lifecycle_*.rs`)
- attach failure (bad manifest, compile error): worker doesn't start; handle
  not returned; resources cleaned
- attach success then explicit `detach()`: worker joins, no leak (verified by
  thread count and resource counters)
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
- compile-fail test: `assert_impl_all!(DebugSessionHandle: Send, Sync, Clone)`;
  `assert_not_impl_any!(DebugSessionCore<'_>: Send)` (negation pin)

### Layer 2 - COM apartment (`oxvba-debug/tests/com_apartment_*.rs`)
- `com_apartment = Sta` on Windows: worker successfully initializes STA;
  CoUninitialize on shutdown; test asserts via `CoGetApartmentType`
- `com_apartment = Mta`: MTA init verified
- `com_apartment = None`: no COM call (test runs on Linux too)
- multiple sessions: each worker independent apartment

### Layer 2 - Line mapping (`oxvba-debug/tests/line_mapping_*.rs`)
- bare source (no preamble): offset = 0; file_line == oxvba_line
- single `Attribute VB_Name = "..."`: offset = 1
- `Attribute` + `Option Explicit`: offset = 2 (canonical thin-slice)
- combinations: `Attribute` + `Option Explicit` + `Option Compare Text` +
  `Option Base 1` + `Option Private Module`: offset = 5
- blank lines preserved (not counted in offset)
- comment lines (`'`, `Rem `): preserved
- property: `oxvba_to_file(file_to_oxvba(N)) == N` for any N in the proc body
- cross-module: each module computed independently against its own source
- edge: empty module
- edge: module with only preamble (no proc)

### Layer 2 - Async surface (`oxvba-debug/tests/async_*.rs`, feature = "tokio")
- `step_into_async()` returns a Future resolving to the same value as the
  sync `step_into()`
- concurrent async commands (5 spawned tasks) serialize at the worker
- cancellation: dropping the future before completion does not break the
  worker (next sync command succeeds)
- async event subscription using `tokio::sync::broadcast::Receiver`

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
1. attach
2. setBreakpoints(Module1, [line 6])
3. on event Stopped(reason=Entry): assert frame_id, location
4. continueExecution
5. on event Stopped(reason=Breakpoint): assert breakpoint id, line
6. stackTrace -> assert frame list with real ids
7. scopes(frame_id) -> not in this workset; locals via `frame_locals`
8. evaluate(frame_id, "answer") -> assert value
9. continueExecution
10. on event Exited: assert exit_code
11. detach

**Scenario B - OxIde cockpit flow** (mirrors what OxIde will do post-migration):
1. attach (project = thin-slice)
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
    lifecycle, COM apartment policy, async surface, error model, downstream
    migration story (OxIde and `oxvba-dap` first), backward-compat strategy
    for the type-move (re-exports, deprecation window).
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
  `oxvba-host`, `oxvba-vm`, `oxvba-runtime`, `oxvba-compiler`. `cargo build`
  and `cargo test` green for the empty crate.

Design:
  - `crates/oxvba-debug/Cargo.toml`: `[package]`, deps, two features
    (`tokio`, `proptest`).
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
    assertions (`assert_impl_all`, `assert_not_impl_any`).
  - Empty test stubs in each `handle_*.rs`, `events_*.rs`, `lifecycle_*.rs`,
    `concurrency_*.rs`, `com_apartment_*.rs`, `line_mapping_*.rs`,
    `async_*.rs`, `property_*.rs`, `stress_*.rs`, `scenarios_*.rs` file
    enumerating every catalog item.

Tests:
  - The Send/Sync compile-fail/static-assertion tests pass against the
    (empty) handle skeleton from B01.
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

### B03 - Move debug-domain DTOs into `oxvba-debug`

Type: Refactor

Goal:
  `DebugWatchRecord/Evaluation/Status`, `DebugBreakpointRecord/BindingStatus/UnresolvedReason`,
  `DebugVariantPauseState`, `DebugFrameVariant*`, `HostDebugVariantRunResult`,
  `DebugSessionError`, `DebugEvaluationRequest`, `DebugVariantEvaluationResult`,
  `DebugSessionCommandStatus` live in `oxvba-debug`; `oxvba-host` re-exports
  them under their old paths for one release for downstream backward compat;
  every existing OxVba test still passes.

Design:
  - Move types module-by-module; each move keeps a `pub use` re-export in
    `oxvba-host::debugger` with `#[deprecated(note = "use oxvba_debug::*")]`.
  - `DebugSession<'engine>` itself **stays** in `oxvba-host` (it's the
    Engine-borrowing core; moving it would force a circular dep). Its method
    signatures continue to refer to the (now-relocated) types via re-export.
  - No semantic change.

Tests:
  - All existing `oxvba-host` tests pass unchanged.
  - New `oxvba-debug` tests assert each re-exported path resolves identically.

Evidence:
  - `cargo test -p oxvba-host` green.
  - `cargo test -p oxvba-debug` green for the type-move regression.

Closure:
  - [ ] All listed types live in `oxvba-debug`.
  - [ ] `oxvba-host` re-exports compile and emit deprecation warnings only
        for direct out-of-tree consumers.
  - [ ] No semantic regression.

### B04 - Public view DTOs + Send/Sync projection layer

Type: Feature

Goal:
  `oxvba-debug::views` exposes Send + Sync + Clone + serde::Serialize view
  types (`DebugPauseView`, `DebugBreakpointView`, `DebugWatchView`,
  `DebugFrameView`, `DebugValueView`, `DebugSourceLocationView`,
  `DebugStopReasonView`, `DebugModuleView`) that project the internal
  records / pause state into transport-ready DTOs - the canonical shape
  every consumer reads.

Design:
  - Each view excludes raw `Variant` (which is !Send / non-serializable);
    Variant values are projected to `DebugValueView { display_text, type_label,
    kind, raw_repr (optional Base64 bytes) }`.
  - Free function `view::pause_view_from_core(&DebugVariantPauseState,
    &LineMapping) -> DebugPauseView` (and similarly for breakpoint, watch).

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
  `attach_debug_session` spawns a worker that owns the
  `DebugSessionCore<'engine>`; the returned `DebugSessionHandle` is Send +
  Sync, cheap to clone, and exposes the synchronous command surface (every
  stepping / breakpoint / watch / inspect method from the design doc).
  Commands marshal via `crossbeam_channel`; responses via `oneshot`.
  **No events yet** (B06 + B07).

Design:
  - `handle.rs`: `DebugSessionHandle { inner: Arc<HandleInner> }`; unsafe
    Send/Sync impls with documented safety invariant; `Drop` for `HandleInner`
    signals worker shutdown.
  - `worker.rs`: worker loop owns the core, processes `DebugCommand` enum,
    sends `Result<DebugReply, DebugError>` via oneshot.
  - `command.rs`: `DebugCommand` enum with one variant per public method.
  - Every handle method: send command, await oneshot, return view.

Tests (from B02 catalog, now implemented):
  - `handle_step_into.rs`, `handle_step_over.rs`, `handle_step_out.rs`,
    `handle_continue.rs`: each calls the method on a fresh handle and
    asserts the returned `DebugPauseView` matches the equivalent
    `DebugSession::start_variants + step_*_variants` flow.
  - `handle_breakpoint_set.rs`, `handle_breakpoint_toggle.rs`,
    `handle_breakpoint_clear.rs`: each exercises real OxVba binding semantics.
  - `handle_watch_*.rs`: add/update/remove/evaluate.
  - `handle_inspect.rs`: current_pause, frame_locals, evaluate.
  - `concurrency_serialization.rs`: 8 caller threads issuing commands;
    response order matches send order at the channel.

Evidence:
  - Every handle method has a green happy-path test.
  - Concurrency test asserts serialization.

Closure:
  - [ ] Every public command method implemented and pinned by tests.
  - [ ] Handle Send + Sync verified by B02 static assertions.
  - [ ] No data races (thread-sanitizer pass).

### B06 - Event taxonomy + subscription stream

Type: Feature

Goal:
  `DebugEvent` enum (Stopped / Output / Continued / Exited / BreakpointChanged
  / ModuleLoaded / ThreadStarted) is defined; `handle.subscribe() ->
  DebugEventReceiver` returns a multi-consumer broadcast receiver; subscribers
  joining late receive future events only (no automatic replay); the channel
  mode is configurable (Bounded(n) for back-pressure or drop-oldest, or
  Unbounded).

Design:
  - `events.rs`: `DebugEvent` enum + `DebugEventReceiver` (wraps
    `tokio::sync::broadcast::Receiver` when `tokio` feature is on, otherwise
    a `crossbeam_channel::Receiver` from a fanout).
  - The worker holds the broadcast `Sender` and emits events at known points
    (B07 wires the emission sites).

Tests:
  - `events_subscribe_before_attach.rs`: subscriber created before any event;
    receives all events.
  - `events_multi_subscriber.rs`: 3 subscribers receive identical streams.
  - `events_slow_subscriber_bounded.rs`: bounded channel + slow subscriber
    drops oldest, worker not blocked.
  - `events_subscriber_drop_safe.rs`: subscriber dropped mid-stream; worker
    continues; remaining subscribers unaffected.
  - `events_ordering.rs`: `Stopped` event delivered to subscriber before the
    next command response (deterministic ordering).

Evidence:
  - Event delivery tests green; ordering pinned.

Closure:
  - [ ] `subscribe()` works; multi-consumer semantics correct.
  - [ ] Configurable channel mode tested in both bounded and unbounded modes.

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
  - `Output` is captured from the VM's stdout/stderr/host collectors.
  - The Variant `Stopped` location uses the same line mapping as
    `current_pause`.

Tests (each event variant has a "what produces it" test):
  - `events_stopped_on_entry.rs`: attach -> Stopped(Entry).
  - `events_stopped_on_breakpoint.rs`: continue to breakpoint -> Stopped(Breakpoint).
  - `events_stopped_on_step.rs`: step_into -> Stopped(Step).
  - `events_continued.rs`: continue_execution -> Continued.
  - `events_exited.rs`: continue past last instruction -> Exited.
  - `events_output_debug_print.rs`: Debug.Print "hello" -> Output(Host, "hello").
  - `events_breakpoint_changed.rs`: set_enabled(false) -> BreakpointChanged.
  - `events_module_loaded.rs`: attach to 2-module project -> 2 ModuleLoaded.

Evidence:
  - Every event variant produced by a real action; pinned by tests.

Closure:
  - [ ] Every event variant emitted at the right time.
  - [ ] Subscriber tests in B06 still green after wiring.

### B08 - Line-basis mapping in the projection layer

Type: Feature (moves the mapping from OxIde into `oxvba-debug`)

Goal:
  The handle's view projections (file_line in `DebugSourceLocationView`,
  in `DebugBreakpointView`, in `DebugPauseView.current_location`) carry
  **editor file lines**, not OxVba's compiled-source-line basis. Inputs
  (`set_source_breakpoint(file_line)`) accept editor file lines and the
  worker converts to OxVba's basis before calling `set_source_breakpoint`
  on the core.

Design:
  - `line_mapping.rs`: `LineMapping` struct built from a `ProjectManifest`
    at attach time: per-module count of leading `attribute ` / `option `
    lines (the rule `oxvba_compiler::lower_module_source` applies).
  - `LineMapping::file_to_oxvba(module, file_line)` and
    `LineMapping::oxvba_to_file(module, oxvba_line)`.
  - Wired into view projection (B04) and command marshalling (B05).
  - OxIde's `crates/oxide-oxvba` mapping (currently lives there) is the
    direct ancestor; this bead supersedes it. OxIde removes its copy in
    its follow-up migration bead (out of scope here).

Tests:
  - All `line_mapping_*.rs` stubs from B02 implemented:
    bare (offset 0), Attribute-only (1), Attribute+Option Explicit (2),
    Attribute+Option Explicit+Option Compare Text+Option Base 1 (4),
    blanks preserved, comments preserved, inverse property
    `oxvba_to_file(file_to_oxvba(N)) == N`, multi-module independent,
    edge cases (empty module, preamble-only module).
  - Snapshot: thin-slice statements at file lines 6/7/8 (Dim / `=` /
    Debug.Print) bind through the handle when caller passes file lines.

Evidence:
  - All line-mapping catalog tests green.
  - Snapshot of bound vs. unresolved by file line.

Closure:
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
  - Teardown is best-effort but logged on failure.

Tests:
  - `com_apartment_sta_init.rs` (Windows): assert via
    `CoGetApartmentType` that the worker is STA.
  - `com_apartment_mta_init.rs` (Windows): MTA init verified.
  - `com_apartment_none.rs` (cross-platform): no COM call; works on Linux.
  - `com_apartment_multi_session.rs` (Windows): two sessions, two
    independent apartments.

Evidence:
  - COM init/uninit verified by tests; no apartment leaks across sessions.

Closure:
  - [ ] STA / MTA / None all supported.
  - [ ] Cross-platform CI (Windows + Linux at least) covers the
        apartment-relevant subset on each.

### B10 - Async surface (`*_async` variants, feature = "tokio")

Type: Feature

Goal:
  For every sync handle method there's an `*_async` variant returning
  `impl Future<Output = Result<View, DebugError>>`. Async subscribers can
  use a `tokio::sync::broadcast::Receiver` directly. Sync callers (OxIde
  in Tauri commands) and async callers (`oxvba-dap` server) both work
  from the same handle.

Design:
  - `async_handle.rs`, `#[cfg(feature = "tokio")]`: each method spawns the
    command into the worker channel and awaits a tokio oneshot.
  - The worker stays sync (no internal tokio runtime); async is purely a
    caller-side ergonomic.
  - Cancellation: dropping the future drops the oneshot receiver; the worker
    discards the result when it finishes processing (no panic; logged).

Tests (`async_*.rs`, only run with `--features tokio`):
  - `async_step_into.rs`: `step_into_async().await` equals `step_into()`.
  - `async_concurrent.rs`: 5 spawned tasks concurrent; serialized at worker.
  - `async_cancellation.rs`: drop the future; next sync command on the same
    handle still works; worker not poisoned.
  - `async_event_stream.rs`: tokio broadcast Receiver compatible.

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
  - `handle.detach()` consumes `self`, sends a Shutdown command, joins the
    worker, returns `Ok(())` (or the underlying detach error).
  - Last `Arc<HandleInner>` drop triggers Shutdown automatically (idempotent).
  - In-flight commands when Shutdown arrives return
    `Err(SessionAlreadyDetached)`.
  - Worker panic: caught by a top-level `catch_unwind`, recorded in
    `HandleInner::failure_state`; all future handle calls return
    `Err(WorkerFailed { stage, message })`.

Tests (`lifecycle_*.rs` from B02, implemented):
  - `lifecycle_attach_failure.rs`: bad manifest -> error; no worker thread
    leaks (verified via thread count delta).
  - `lifecycle_explicit_detach.rs`: detach() joins cleanly.
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
  - Benchmark numbers committed under `target/criterion/`.

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
     stepPlan/breakpointPlan/watchExpressions; removes OxIde's line
     mapping (now in `oxvba-debug`).

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
  end-to-end against a freshly-published `oxvba-debug`; all B02-B13 tests
  green; the design doc, test catalog, and downstream handoffs are
  finalized.

Design:
  - `scenarios_dap_style_flow.rs`: scenario A from the test catalog.
  - `scenarios_oxide_cockpit_flow.rs`: scenario B from the test catalog.
  - Re-run the full `oxvba-debug` test matrix + `oxvba-host` regression on
    Windows + Linux CI.
  - Write `docs/HANDOFF_OXVBA_DEBUG_HANDLE_v1.md` summarizing what shipped,
    what's deferred (DAP, OxIde migration, future features), and the
    backward-compat re-export deprecation timeline.

Tests:
  - Both scenario tests green.
  - Full crate test matrix green on Windows + Linux.

Evidence:
  - `target/oxvba-debug-acceptance.{txt,json}`.
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

For one release after this workset lands:
- `oxvba-host::debugger::*` re-exports all moved types with
  `#[deprecated(note = "use oxvba_debug::*")]`.
- Direct `Engine::prepare_debug_session` continues to work for power users
  who want raw stateful access.
- The legacy raw API and the new handle API coexist.

After one release, the deprecated re-exports are removed; `oxvba_debug` is
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

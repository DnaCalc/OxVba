# IP-07 Event Parity Design Lock Resolutions

Date: 2026-03-20
Status: resolved
Scope: formal resolution of EPD-01 through EPD-05 design lock decisions from WORKSET_2026-03-08_EVENTS_PARITY_CLOSURE.md

## EPD-01: Subscription key model

**Decision:** Hybrid owner+binding key using `(owner_handle: i32, binding_handle: i32)` encoded as `i64`.

**Rationale:**
- Owner identity is the `ObjectHandle` of the sink instance that declares `WithEvents`.
- Binding identity is the compiler-assigned binding handle for each `WithEvents` variable declaration.
- The combined key provides owner-scoped isolation: each sink instance can have multiple `WithEvents` bindings, and each binding independently tracks its current source.
- The encoding uses `withevents_binding_key(owner, binding)` in the VM interpreter.

**Implementation evidence:**
- `interpreter.rs:1649-1651` (key encoding)
- `interpreter.rs:1657` (owner extraction from key)
- `formal_event_runtime_withevents_binding_intrinsics_are_owner_scoped` (test)

**Status:** implemented and proved.

## EPD-02: Ordering model

**Decision:** Deterministic sorted ordering by `ObjectHandle` value for owner iteration during `RaiseEvent` dispatch.

**Rationale:**
- VBA 7.1 dispatches to `WithEvents` handlers in the order sinks were created, which in practice is deterministic for a given execution.
- OxVBA sorts matching sink owners by their `ObjectHandle` value before iterating. This produces deterministic, reproducible ordering across VM and JIT paths.
- Multiple handlers for the same source+event are dispatched in subscription order within each owner.

**Implementation evidence:**
- `interpreter.rs:2007` (withevents_matching_owners sorts by ObjectHandle)
- `formal_event_runtime_raiseevent_dispatches_to_withevents_handlers_in_stable_order` (test)

**Status:** implemented and proved.

## EPD-03: Reentrancy/DoEvents policy

**Decision:** Single-level dispatch with deterministic higher-arity rejection.

**Rationale:**
- The current event dispatch path does not permit unbounded reentrancy. Event handlers execute inline during `RaiseEvent` dispatch through the runtime owner-iteration intrinsics.
- Higher-arity events (beyond the currently supported zero/one-argument subset) are rejected deterministically rather than silently dropping arguments.
- DoEvents-style yielding during event dispatch is not yet in scope; the current model is synchronous dispatch-to-completion.
- When reentrancy support is needed (e.g., for Office-style event models where handlers may raise further events), it will be added as an explicit extension rather than an implicit behavior.

**Implementation evidence:**
- `engine.rs:287-353` (dispatch_host_event_into_runtime — synchronous inline dispatch)
- Higher-arity rejection proved in host event ingress tests

**Status:** resolved as synchronous dispatch-to-completion with deterministic arity rejection. Reentrancy extension deferred.

## EPD-04: Host-event ingress contract

**Decision:** Canonical engine entrypoint `dispatch_host_event_into_runtime` with source-instance-aware routing and argument marshaling through `RuntimeValue`.

**Rationale:**
- Host-raised events use the same `EventDispatcher` subscription model as compiler-emitted `RaiseEvent` dispatch, unified through `EventSourceKey` triples.
- The engine entrypoint validates arity (currently 0 or 1 arguments), resolves handler symbols from the event dispatcher, and invokes each handler with a guard check for source-instance routing.
- Arguments are marshaled as `RuntimeValue` values through the standard slot mechanism.
- COM event callbacks enter through `poll_and_dispatch_next_com_event_callback` which normalizes the callback payload into the same engine dispatch path.

**Implementation evidence:**
- `engine.rs:240-353` (host event subscription, dispatch, runtime invocation)
- `engine.rs:409-460` (COM event callback polling)
- Host event ingress end-to-end tests across both VB_PredeclaredId and VB_GlobalNamespace exposure modes

**Status:** implemented and proved for the zero/one-argument subset.

## EPD-05: COM parity tiering

**Decision:**
- **COM-EVT-A (required):** Dispatch-style connection-point event callbacks through `IConnectionPoint` with `IDispatch` sink interface. This is the standard COM event model used by VBA `WithEvents` with COM objects.
- **COM-EVT-B (tiered):** Advanced COM event scenarios (custom source interfaces, aggregation, cross-apartment marshaling) are explicitly deferred with deterministic diagnostics. These are not required for VBA 7.1 Office-style parity.

**Rationale:**
- VBA's COM event model uses `IConnectionPointContainer` / `IConnectionPoint` with `IDispatch`-based sink interfaces. This is COM-EVT-A.
- The current implementation has connection-point subscription/unsubscription infrastructure in `windows_connection_point.rs` and callback polling in the engine.
- COM-EVT-B scenarios (custom interfaces, non-IDispatch sinks) are outside VBA 7.1's late-bound event model and are deferred.

**Implementation evidence:**
- `windows_connection_point.rs` (connection-point subscription infrastructure)
- `engine.rs:355-397` (COM event subscription/unsubscription)
- `engine.rs:493+` (COM event callback dispatch into runtime)
- End-to-end COM event tests with OxVba.TestEventServer fixture

**Status:** COM-EVT-A infrastructure is in place; COM-EVT-B explicitly deferred with documented tiering.

# Workset: Events Runtime Closure under Host Project + HAL Split

Date: 2026-03-08  
Status: in-progress (EVR1-EVR3 baseline implemented; closure-pass reconciliation updated on 2026-03-08)  
Scope: complete executable event-runtime behavior with Host Project as semantic authority, full HAL service provisioning, and COM as transport adapter lane.

Continuation note:
- The full parity completion track now continues in `WORKSET_2026-03-08_EVENTS_PARITY_CLOSURE.md`.

## 0. Implementation update (2026-03-08)

Completed in this cycle:
- EVR1 baseline: `RaiseEvent` now lowers to deterministic handler-call dispatch in `compile_project(...)` for known `WithEvents` bindings.
- EVR2 baseline: project compile now emits stable event dispatch bindings (`source project/module/event -> lowered handler symbol`) derived from `WithEvents` declarations + handler prefix conventions + declared class events.
- EVR3 baseline: host runtime now owns a deterministic non-COM event dispatcher/subscription map and exposes host-event dispatch lookup API (`Engine::dispatch_host_event(...)`), hydrated from compiled project event bindings.

Residual scope (still open in this workset):
- true runtime `Set`/reassignment lifecycle semantics for `WithEvents` variables (subscribe/unsubscribe transitions),
- argument-shape/signature parity for full VBA event callback semantics,
- host-event ingress executing handler call paths directly (current baseline resolves deterministic handler target set).
  - deterministic reassignment/clear transition probes are now executable (`formal_event_runtime_withevents_reassignment_rebinds_non_default_instances_deterministically`, `formal_event_runtime_withevents_clear_then_rebind_updates_dispatch_membership`); residual parity remains around full sink-instance graph semantics.

## 1. Why this continuation exists

The event story now has:
- project-aware compile-time legality for `WithEvents`/`Implements`/`RaiseEvent`,
- canonical diagnostics governance,
- explicit architecture decision: Host Project owns semantic surface; HAL remains mandatory service contract; COM is adapter lane.

The remaining gap is runtime execution parity and host integration behavior across platforms.

This workset continues from `WORKSET_2026-03-07_EVENTS_STORY_COMPLETION.md` and narrows execution to the remaining runtime closure path.

## 2. Normative model lock

1. **Host Project semantic plane (authoritative):**
   - Defines host-visible globals, types, and event signatures visible to user projects.
   - Drives compile/bind semantics independent of transport.

2. **HAL service plane (mandatory):**
   - Host provides full HAL capability suite by profile/policy (`FileSystemIo`, `TimeLocale`, `ProcessEnv`, `UiInteraction`, `EventPump`, etc.).
   - Event model work must not bypass HAL policy gating or profile limits.

3. **Transport adapters (replaceable):**
   - Non-COM bridge is first-class for cross-platform host execution.
   - COM bridge is a Windows adapter lane (`COM-EVT-A` then `COM-EVT-B`).

## 3. Workset objectives

1. Make class-event runtime semantics executable and deterministic.
2. Implement host-raised event routing without COM dependency.
3. Preserve full HAL capability/policy governance in event flows.
4. Validate cross-platform behavior with DNA VbCalc pathfinder harness.
5. Keep COM event support as adapter workstream, not semantic blocker for non-COM hosts.

## 4. Phase plan

### EVR1 - Runtime event IR + dispatcher substrate

Deliverables:
- Introduce explicit runtime event dispatch instructions/ops for `RaiseEvent`.
- Implement subscription graph in host runtime (`assign`, `reassign`, `clear`, teardown).
- Wire deterministic ordering rules into dispatcher execution.

Checks:
- unit tests for graph state transitions and dispatch order,
- VM/JIT parity tests for event-heavy fixtures.

### EVR2 - Host Project-driven root/event binding

Deliverables:
- Bind host root/predeclared objects via Host Project metadata, not COM assumptions.
- Resolve handler targets using Host Project event metadata.
- Enforce deterministic diagnostics for missing event target/state mismatch.

Checks:
- project integration tests with source + host project graphs,
- deterministic diagnostics for missing bindings.

### EVR3 - Non-COM host bridge runtime lane (cross-platform baseline)

Deliverables:
- Add/lock non-COM event ingress path (`host -> runtime event queue -> handler dispatch`).
- Implement lifecycle-safe subscribe/unsubscribe behavior from `WithEvents` assignment transitions.
- Ensure path works across Windows/Linux/macOS/WASM profile constraints.

Checks:
- host integration fixtures for each runtime class family,
- capability-denial behavior asserted under restrictive policies.

### EVR4 - HAL event/service conformance in event flows

Deliverables:
- Assert `EventPump` and related capability checks across event dispatch points.
- Ensure event handlers invoking file/time/process operations still route through HAL gates.
- Add coverage rows and obligations for event+HAL interactions.

Checks:
- policy preset matrix tests (`strict-ci`, deterministic modes, interactive-dev),
- formal obligations for event lifecycle and policy invariants.

### EVR5 - DNA VbCalc cross-platform pathfinder lane

Deliverables:
- Minimal pathfinder harness exercising:
  - Host Project load,
  - root object injection,
  - non-COM event pump,
  - handler execution and deterministic teardown.
- Scenario corpus: control-click/change events, reassignment, object release.

Checks:
- reproducible run scripts and evidence markdown,
- parity assertions across at least Windows + Linux baseline lane.

### EVR6 - COM adapter continuation (non-blocking for non-COM parity)

Deliverables:
- `COM-EVT-A`: dispatch-style connection-point callbacks (blocking only for COM adapter claim).
- `COM-EVT-B`: non-dispatch path support or explicit deterministic unsupported policy.

Checks:
- Windows-only adapter lanes with stable diagnostics,
- no silent fallback between COM event paths.

## 5. Deliverable artifacts

- `docs/evidence/conformance/events/EVENTS_RUNTIME_RUN_<runid>.md`
- `docs/evidence/conformance/events/EVENTS_RUNTIME_RUN_<runid>.csv`
- `docs/evidence/conformance/events/lanes/EVR1_*.md` ... `EVR6_*.md`
- updates to:
  - `docs/evidence/divergences/DIV-0004.md`
  - `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`
  - `docs/evidence/formal/obligations.csv`
  - `docs/evidence/formal/FEATURE_OBLIGATION_COVERAGE_V1.csv`

## 6. Exit criteria

1. `WithEvents` reassignment and `RaiseEvent` dispatch execute deterministically at runtime.
2. Non-COM host event routing is executable and evidence-backed cross-platform.
3. Event flows respect HAL service/capability policy in all tested lanes.
4. DNA VbCalc pathfinder lane validates Host Project semantic ownership.
5. `DIV-0004` is closed or narrowed to explicitly documented residual scope.
6. COM adapter claims are explicitly tiered (`COM-EVT-A` and `COM-EVT-B`).

## 7. Initial command scaffold

```powershell
# Compiler + host runtime event semantics
cargo test -p oxvba-compiler compile_project_ -- --nocapture
cargo test -p oxvba-host event -- --nocapture

# Governance and diagnostics sync
./scripts/check-governance.ps1
```

# Workset: Event Parity Closure (Design + Implementation)

Date: 2026-03-08  
Status: in-progress (baseline closure pass completed; parity residuals bounded)  
Scope: complete OxVBA class/event runtime behavior to practical VBA parity across non-COM and COM adapter lanes, starting from EVR1-EVR3 baseline.

Continuation note:
- The integrated terminal run plan is captured in `WORKSET_2026-03-08_FULL_EVENTS_PARITY_NONCOM_COM.md`.

## 0. 2026-03-08 closure-pass update

Completed in this pass:
- `DIV-0003` baseline mismatch closed (project-aware Implements legality + deterministic runtime prefixed-member execution evidence).
- runtime deterministic event baseline remains executable:
  - static `RaiseEvent` handler ordering (module-level mapping),
  - host-event binding hydration and lookup.
- conformance/deferred registers were reconciled for post-baseline status (`CCT-040/041`, `ODG-038/039`).
- formal event obligations were extended for runtime limit probes and event-closure tracking consistency.

Bounded residuals:
- deterministic `WithEvents` reassignment/clear transitions are now executable; full sink-instance graph parity remains open (`DIV-0004` / `ODG-039`),
- advanced multi-interface oracle edge matrix remains open (`ODG-038`),
- COM event adapter parity lanes remain staged.

## 1. Baseline and gap

Current implemented baseline:
- compile-time legality for `WithEvents` / `Implements` / `RaiseEvent`,
- deterministic event dispatch binding extraction at project compile time,
- runtime non-COM dispatcher substrate and host-event mapping API.

Remaining parity gap:
- per-instance subscription identity and cleanup behavior,
- full `RaiseEvent` callback argument/signature parity,
- host bridge event ingress semantics and reentrancy rules,
- COM adapter parity lanes (`COM-EVT-A` required, `COM-EVT-B` explicitly tiered).

Tracked closures:
- `DIV-0003` (closed), `DIV-0004` (open),
- `ODG-038`, `ODG-039`,
- event/runtime conformance topics (`CCT-040`, `CCT-041`).

## 2. Parity target definition

Parity target for this work item means:
1. `WithEvents` subscriptions are runtime-instance-driven and update on assignment transitions.
2. `RaiseEvent` dispatches through runtime subscription graph with deterministic ordering rules.
3. Handler invocation and error routing follow deterministic VBA-compatible behavior in supported subset.
4. Host-raised events route through the same runtime subscription model (not a separate semantic path).
5. Non-COM hosts are first-class and complete for semantic behavior; COM is adapter transport.

## 3. Design lock decisions required up front

`EPD-01` Subscription key model:
- object identity key shape and lifetime ownership (host token, runtime object id, or hybrid).
- **Resolved 2026-03-20:** hybrid `(owner_handle, binding_handle)` as i64 key. See [WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md](WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md).

`EPD-02` Ordering model:
- deterministic ordering for multiple handlers and reassignment edge cases.
- **Resolved 2026-03-20:** sorted by ObjectHandle value; subscription order within owner. See [WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md](WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md).

`EPD-03` Reentrancy/DoEvents policy:
- allowed nested dispatch depth and deterministic error/abort behavior.
- **Resolved 2026-03-20:** synchronous dispatch-to-completion with deterministic arity rejection; reentrancy deferred. See [WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md](WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md).

`EPD-04` Host-event ingress contract:
- canonical engine entrypoint for host-raised events and argument marshaling policy.
- **Resolved 2026-03-20:** `dispatch_host_event_into_runtime` with source-instance-aware routing. See [WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md](WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md).

`EPD-05` COM parity tiering:
- closure bar for `COM-EVT-A` claim; explicit unsupported/deferral policy for `COM-EVT-B`.
- **Resolved 2026-03-20:** COM-EVT-A required (IConnectionPoint/IDispatch sinks); COM-EVT-B deferred. See [WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md](WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md).

## 4. Execution phases

### EVP1 - Normative event semantics design lock

Deliverables:
- event-state machine spec for `WithEvents` variable lifecycle (`unbound -> bound -> rebound -> cleared -> terminated`),
- handler matching and ordering rules,
- diagnostic/error code matrix for runtime event failures.

Gate:
- approved design appendix and clause mappings in PMR/HOST specs.

### EVP2 - Runtime subscription graph implementation

Deliverables:
- runtime event graph keyed by instance identity + event name,
- subscribe/unsubscribe transitions driven by assignment operations,
- teardown hooks on scope end and `Class_Terminate`.

Gate:
- executable state-transition tests covering assignment/reassignment/clear/teardown.

### EVP3 - `RaiseEvent` runtime parity completion

Deliverables:
- dispatch path uses runtime graph (not static rewrite fallback),
- event argument routing and signature checks in supported subset,
- deterministic failure diagnostics for missing/invalid targets.

Gate:
- deterministic runtime ordering tests and argument-shape fixtures pass.

### EVP4 - Host event ingress + non-COM parity lane

Deliverables:
- canonical host-event ingress API wired into same graph/dispatch path,
- cross-platform host harness fixtures (Windows/Linux/macOS; WASM where profile allows),
- policy-aware behavior under HAL capability restrictions.

Gate:
- host ingress/egress parity fixtures pass across required profiles.

### EVP5 - COM adapter parity lanes

Deliverables:
- `COM-EVT-A`: dispatch-style connection-point callbacks complete and evidence-backed,
- `COM-EVT-B`: implemented or explicitly unsupported with deterministic diagnostics and foldback plan.

Gate:
- Windows COM lane artifacts + stable diagnostic/error mapping.

### EVP6 - Formal, conformance, and oracle foldback

Deliverables:
- event runtime conformance lanes updated (`EV-L0..EV-L6`),
- formal obligations for event graph safety and lifecycle invariants,
- oracle captures for ambiguous ordering/edge behavior where required.

Gate:
- updated conformance/formal evidence set and deferred gate statuses reconciled.

### EVP7 - Closure and claim lift

Deliverables:
- close or explicitly re-scope `DIV-0003`/`DIV-0004`,
- close `ODG-038`/`ODG-039` (or move to explicit bounded residuals),
- update spec claim tier to parity-ready scope.

Gate:
- closure report + governance checks + integrated host/runtime test sweep.

## 5. Artifact and tracking outputs

- `docs/evidence/conformance/events/EVENTS_PARITY_RUN_<runid>.md`
- `docs/evidence/conformance/events/EVENTS_PARITY_RUN_<runid>.csv`
- `docs/evidence/conformance/events/lanes/EVP*_*.md`
- `docs/evidence/formal/obligations.csv` updates for event runtime invariants
- divergence and deferred gate updates for `DIV-0003/0004`, `ODG-038/039`

## 6. Exit criteria

1. Runtime `WithEvents` assignment/reassignment semantics are executable and deterministic.
2. `RaiseEvent` dispatch uses runtime subscriptions with stable ordering and error behavior.
3. Host-raised event path is unified with runtime event model and works cross-platform.
4. Required COM adapter parity lane (`COM-EVT-A`) is complete and evidence-backed.
5. Event divergences/deferred gates are closed or explicitly bounded with approved residual scope.
6. Parity claim text in specs is updated to reflect actual implemented scope.

## 7. Starter command set

```powershell
# Event-focused compiler and host lanes
cargo test -p oxvba-compiler event -- --nocapture
cargo test -p oxvba-host formal_event_runtime_ -- --nocapture

# Governance drift + docs obligations
./scripts/check-governance.ps1
```

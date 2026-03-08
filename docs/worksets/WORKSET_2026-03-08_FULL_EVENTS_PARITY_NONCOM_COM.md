# Workset: Full Events Parity Run (Non-COM + Windows COM)

Date: 2026-03-08  
Status: planned (single integrated run)  
Scope: complete VBA-parity event behavior for:
- non-COM lane (internal class events + Host Project/class-library style hosting), and
- Windows COM lane (typelib -> binder -> runtime -> event callback lifecycle).

## 1. Terminal outcome (claim to unlock)

At closure, OxVBA may claim:
1. Non-COM event parity for practical VBA class-event semantics:
   - `WithEvents` assignment/reassignment/clear lifecycle,
   - `RaiseEvent` dispatch against runtime subscription graph,
   - deterministic ordering and teardown behavior.
2. Windows COM event parity for supported transport tiers:
   - `COM-EVT-A`: connection-point + `IDispatch` event callbacks (required),
   - `COM-EVT-B`: source-interface/vtable callback path (required to implement or deterministically defer with explicit unsupported policy).
3. Type-library event metadata is executable end-to-end:
   - compile/bind resolution,
   - runtime signature mapping,
   - stable diagnostics for mismatch paths.

## 2. Current baseline and gap

Implemented baseline:
- project-aware legality diagnostics for `WithEvents`/`RaiseEvent`/`Implements`,
- deterministic compile-time event binding extraction,
- host dispatcher substrate for non-COM handler lookup.

Remaining parity gap:
- full sink-instance subscription graph semantics on `Set` transitions (deterministic binding token transitions are executable baseline),
- instance-level identity semantics for `WithEvents` bindings,
- host ingress path executing handlers through the same runtime graph,
- COM callback lifecycle completion,
- typelib-driven event signature/runtime argument mapping parity.

Open trackers this run is meant to close:
- `DIV-0004` / `ODG-039` (full sink-instance graph + reassignment semantics),
- `ODG-038` (advanced `Implements` edge-matrix oracle refresh),
- COM event parity residuals currently staged in event worksets.

## 3. Design locks required up-front

`DL-01` Event identity model:
- canonical source identity key (`project/module/default-instance-or-object-id/event-name`) and sink binding key.

`DL-02` Subscription state machine:
- `unbound -> bound -> rebound -> cleared -> terminated`,
- explicit side effects for `Set x = y`, `Set x = Nothing`, scope exit, `Class_Terminate`.

`DL-03` Ordering contract:
- deterministic ordering for same-event multi-sink dispatch under reassignment and reentrancy.

`DL-04` Host ingress contract:
- one canonical host event ingress API path that reuses runtime subscription graph.

`DL-05` COM era split policy:
- required behavior in `COM-EVT-A`,
- implemented or deterministic unsupported behavior in `COM-EVT-B` with no silent fallback.

`DL-06` Argument shape contract:
- event argument marshaling rules (arity, optional/default behavior, ByRef/ByVal policy subset).

## 4. Single-run ladder (work items)

## Phase A - Specification and contracts

`WI-A01` Publish normative event runtime spec:
- add `docs/spec/EVENT_RUNTIME_PARITY_SPEC_V1.md`.
- include lifecycle state machine, ordering, teardown, diagnostics matrix.

`WI-A02` Publish host bridge event contract:
- add `docs/spec/HOST_EVENT_BRIDGE_SPEC_V1.md`.
- define subscribe/unsubscribe/dispatch ingress contract and error surface.

`WI-A03` Publish COM event bridge contract:
- add `docs/spec/COM_EVENT_BRIDGE_SPEC_V1.md`.
- split `COM-EVT-A` vs `COM-EVT-B`, policy, fallback constraints.

`WI-A04` Publish typelib-event binding contract:
- add `docs/spec/TYPELIB_EVENT_BINDING_SPEC_V1.md`.
- map typelib event/source-interface metadata to binder/runtime call shapes.

`WI-A05` Clause catalog + conformance topics alignment:
- update PMR/HOST/COM clause catalogs and `CONFORMANCE_CHECK_TOPICS.csv` for event runtime clauses.

## Phase B - Non-COM language/runtime parity

`WI-B01` Runtime subscription graph implementation:
- introduce runtime-owned graph keyed by source-instance identity + sink binding + event.

`WI-B02` `WithEvents` assignment transition wiring:
- on assignment/reassignment, unhook old source and hook new source deterministically.

`WI-B03` `WithEvents` clear/teardown wiring:
- `Set ... = Nothing`, scope exit, and termination paths clean subscriptions.

`WI-B04` `RaiseEvent` runtime dispatch completion:
- dispatch through runtime graph (not static-only lowering model).

`WI-B05` Dispatch ordering and reentrancy rules:
- enforce stable ordering and deterministic nested dispatch limits.

`WI-B06` Event argument routing parity subset:
- support declared signatures for implemented subset; reject unsupported shapes with stable diagnostics.

`WI-B07` VM/JIT parity for event-heavy flows:
- guarantee event semantics equivalent across VM and JIT paths.

## Phase C - Host Project/class-library style non-COM parity

`WI-C01` Host Project semantic authority wiring:
- host-provided project metadata drives root/event exposure for user projects.

`WI-C02` Unified ingress path:
- host-raised events route via single engine ingress -> runtime graph -> handler invocation.

`WI-C03` HAL policy/capability integration:
- event callbacks invoking File/Time/Process/etc remain HAL-governed with deterministic policy errors.

`WI-C04` Code-behind/document-style routing:
- host object -> VBA handler mapping for class-library style host model and document-like modules.

`WI-C05` Cross-platform pathfinder harness:
- create/extend `DNA VbCalc`-style minimal host harness for Windows/Linux/macOS lanes.

## Phase D - COM parity from typelib to runtime

`WI-D01` Typelib event metadata ingestion:
- binder/runtime metadata includes event source interfaces, dispids, argument shapes.

`WI-D02` `COM-EVT-A` connection-point subscription lifecycle:
- discover source, `Advise`/`Unadvise`, deterministic token ownership and cleanup.

`WI-D03` `COM-EVT-A` callback routing:
- `IDispatch::Invoke` event callbacks mapped to runtime event ingress and handler execution.

`WI-D04` COM error + diagnostic mapping:
- stable error taxonomy for missing connection points, failed advise/unadvise, signature mismatch, callback failures.

`WI-D05` `COM-EVT-B` source-interface path:
- implement vtable/source-interface callback path; if not feasible, lock explicit unsupported policy + deterministic diagnostics + no fallback.

`WI-D06` Runtime coexistence model:
- internal/non-COM and COM event sources share semantic runtime graph and ordering policy.

## Phase E - Controlled test server + external oracle strategy

`WI-E01` Controlled COM event test server v1:
- extend/add in-repo COM fixture server (`OxVba.TestEventSource`) with:
  - methods, properties,
  - dispinterface events (`COM-EVT-A`),
  - source-interface events (`COM-EVT-B`) when available.

`WI-E02` Registrationless deterministic lane:
- mandatory CI lane using controlled server for repeatable event callback tests.

`WI-E03` Registered external lane:
- optional lane for installed external server matrix.

`WI-E04` Excel oracle lane (Windows, optional but first-class evidence):
- use `Excel.Application` event probes as oracle candidate for behavior comparison.
- capture ordering/reassignment/signature behaviors for ambiguous edges.

`WI-E05` Oracle foldback updates:
- reconcile outcomes into `DIV-0004`, `ODG-038`, `ODG-039` and PMR/COM spec notes.

## Phase F - Formal obligations and conformance system

`WI-F01` Formal obligation expansion:
- append new obligations for:
  - subscription graph safety invariants,
  - reassignment lifecycle invariants,
  - teardown no-leak invariants,
  - COM subscribe/unsubscribe lifecycle invariants,
  - VM/JIT event equivalence invariants.

`WI-F02` Feature coverage index update:
- ensure event domains remain depth score 3 with explicit obligation IDs.

`WI-F03` Event conformance lane suite:
- add/refresh `EV-L0..EV-L8`:
  - `EV-L0` legality/diagnostics,
  - `EV-L1` internal runtime lifecycle,
  - `EV-L2` non-COM host ingress/egress,
  - `EV-L3` HAL policy behavior in callbacks,
  - `EV-L4` COM-EVT-A callback lifecycle,
  - `EV-L5` COM-EVT-B lane or explicit unsupported lane,
  - `EV-L6` VM/JIT parity,
  - `EV-L7` stress/reentrancy/cleanup,
  - `EV-L8` oracle replay/foldback.

`WI-F04` Governance scripts and sync checks:
- add validation checks for new diagnostics, clauses, and lane artifacts in `scripts/check-governance.ps1` chain.

## Phase G - Closure

`WI-G01` Integrated gate run:
- run compiler/host/com lanes + governance + formal lane set for this workset.

`WI-G02` Divergence/deferred closure:
- close `DIV-0004` and reconcile `ODG-039` if parity is achieved,
- close `ODG-038` after multi-interface oracle refresh.

`WI-G03` Claim update:
- update hosting/PMR/COM docs to reflect achieved parity and exact residual scope (if any).

## 5. Spec/doc files to add or update

Add:
- `docs/spec/EVENT_RUNTIME_PARITY_SPEC_V1.md`
- `docs/spec/HOST_EVENT_BRIDGE_SPEC_V1.md`
- `docs/spec/COM_EVENT_BRIDGE_SPEC_V1.md`
- `docs/spec/TYPELIB_EVENT_BINDING_SPEC_V1.md`

Update:
- `docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md`
- `docs/spec/COM_EARLY_BINDING_TYPELIB_CONFORMANCE_V1.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.csv`
- `docs/evidence/diagnostics/PMR_EVENT_DIAGNOSTICS_V1.csv` (and generated snippets)
- `docs/evidence/formal/obligations.csv`
- `docs/evidence/formal/FEATURE_OBLIGATION_COVERAGE_V1.csv`
- `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`
- `docs/evidence/divergences/DIV-0004.md`

## 6. Test plan (comprehensive)

Compiler/binder:
- legality matrix for `WithEvents`/`RaiseEvent`/`Implements`,
- event signature and handler-shape diagnostics,
- typelib event metadata binding tests.

Host/runtime non-COM:
- assignment/reassignment/clear lifecycle matrix,
- multi-source/multi-sink ordering cases,
- teardown and reentrancy stress cases,
- VM/JIT parity corpus.

Host Project/class-library style:
- root object + host project event routes,
- document-like code-behind scenarios,
- HAL policy-deny and capability-deny callback behaviors.

COM lanes (Windows):
- registrationless controlled server lane (required),
- registered external lane (optional),
- Excel oracle lane (optional but preferred for parity confidence).

Failure-mode tests:
- missing source event / handler mismatch,
- advise/unadvise failure mapping,
- callback signature mismatch,
- unsupported `COM-EVT-B` path (if deferred) is explicit and deterministic.

## 7. Artifacts this run must emit

- `docs/evidence/conformance/events/EVENTS_FULL_PARITY_RUN_<runid>.md`
- `docs/evidence/conformance/events/EVENTS_FULL_PARITY_RUN_<runid>.csv`
- `docs/evidence/conformance/events/lanes/EV-L*_*.md`
- `docs/evidence/conformance/events/lanes/EV-L*_*.csv`
- `docs/evidence/conformance/oracle_captures/events_<runid>/...`
- updated divergence and deferred-gate rows for event topics
- updated formal obligation and coverage files

## 8. Exit criteria

1. Non-COM event runtime parity achieved for supported VBA class-event subset.
2. Host Project/class-library style event routing is unified with runtime graph and cross-platform evidence-backed.
3. `COM-EVT-A` is complete and validated under Windows controlled lane.
4. `COM-EVT-B` is implemented or explicitly deferred with deterministic unsupported behavior and approved residual scope.
5. Typelib-to-runtime event mapping is implemented and validated with diagnostics coverage.
6. Event formal obligations and conformance lanes are deep, uniform, and governance-checked.
7. `DIV-0004` and related deferred gates are closed or tightly bounded with explicit approval text.

## 9. Starter command skeleton

```powershell
# Core language/runtime event lanes
cargo test -p oxvba-compiler event -- --nocapture
cargo test -p oxvba-host formal_event_runtime_ -- --nocapture

# Existing COM conformance orchestration
./scripts/run-com-conformance.ps1

# Governance and evidence consistency
./scripts/check-governance.ps1
```

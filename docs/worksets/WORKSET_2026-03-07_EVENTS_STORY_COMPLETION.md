# Workset: Events Story Completion (Language + Host + COM)

Date: 2026-03-07  
Status: in-progress (EVT1/EVT2 compiler semantics completed on 2026-03-07)  
Scope: complete the OxVBA events story end-to-end, including class event semantics, host object event hookup, and COM event bridge behavior.

## 1. Why this workset now

Current state is strong but intentionally gated:
- `WithEvents`, `Implements`, and `RaiseEvent` still use deterministic PMR diagnostic gates in several paths.
- Divergences are explicitly tracked (`DIV-0003`, `DIV-0004`) with known expected VBA behavior.
- Host/project execution infrastructure and COM client foundations are in place, so implementation can proceed without architectural blockers.

This workset closes that remaining event-model gap and converts it into executable, evidence-backed behavior.

## 2. Objectives

1. Complete class-event language semantics in project-aware compile/runtime paths.
2. Complete `Implements` coverage and naming constraints for executable class contracts.
3. Add deterministic host-object event hookup contracts for embedded hosts (Excel-like role).
4. Implement COM event bridge baseline with explicit era split and policy controls.
5. Add conformance, oracle, and formal lanes specific to event semantics.
6. Close event divergences (`DIV-0003`, `DIV-0004`) or reclassify with explicit non-goal rationale.
7. Publish a stable operational and diagnostics contract for embedders.

## 2.1 Implementation update (2026-03-07)

Completed in this cycle (compiler/binder closure):
- Removed deterministic gate diagnostics for `WithEvents`, `Implements`, and `RaiseEvent` from single-module resolve/compile paths.
- Added project-aware event diagnostics:
  - `PMR-E-WITHEVENTS-MODULE-KIND`
  - `PMR-E-IMPLEMENTS-MODULE-KIND`
  - `PMR-E-IMPLEMENTS-INTERFACE-NOT-FOUND`
  - `PMR-E-IMPLEMENTS-MEMBER-MISSING`
  - `PMR-E-RAISEEVENT-MODULE-KIND`
  - `PMR-E-RAISEEVENT-UNDECLARED`
- Added project validation semantics for:
  - `WithEvents` module-kind legality,
  - `Implements` module-kind legality + interface existence + member coverage,
  - `RaiseEvent` class-only + declared-event enforcement.
- Added/updated compiler tests for legal and illegal paths; `cargo test -p oxvba-compiler` is green.

Still pending for full workset closure:
- Runtime subscription graph and executable event dispatch semantics (`EVT3`).
- Embedded host event bridge and code-behind routing (`EVT4`).
- COM event bridge implementation and lanes (`EVT5`/`EVT6`).

## 3. Explicit scope boundaries

### In scope

- `WithEvents` legality and handler-binding semantics.
- `RaiseEvent` legality, dispatch, ordering, and reassignment behavior.
- `Implements` executable interface-member coverage in class modules.
- Host object to module event routing contract (document-like code-behind included).
- COM event bridge baseline:
  - connection point discovery,
  - advise/unadvise lifecycle,
  - handler invocation and deterministic error routing.

### Out of scope (for this workset)

- Full Forms designer/user control event surfaces.
- Non-Windows COM parity claims.
- Rich ABI-complete custom marshaling for every external automation signature.

## 4. COM event eras framing (execution policy)

To avoid ambiguity, we explicitly split COM event support into two eras/tracks:

1. `COM-EVT-A` (required in this workset):
   - Automation event sinks via connection points + dispatch-style event invocation.
   - Baseline expected for practical VBA interoperability and host integration.

2. `COM-EVT-B` (planned follow-up in this workset, non-blocking only if explicitly deferred with evidence):
   - Non-dispatch/custom source-interface event paths (vtable-style source interfaces and related adapter mechanics).
   - Implement where feasible; otherwise keep as explicit deferred track with test scaffolding and diagnostics.

Default completion rule:
- `COM-EVT-A` is blocking for workset closure.
- `COM-EVT-B` may be deferred only with documented evidence and unblock plan.

## 5. Phase plan

### Phase EVT1 - Semantic contract lock for class events

Deliverables:
- Update PMR class clauses and conformance mapping for:
  - `WithEvents` module-kind legality,
  - handler-prefix binding and handler target legality,
  - `RaiseEvent` constraints and event declaration binding,
  - `Implements` member coverage and prefix requirements.
- Add deterministic diagnostic families for event-specific failures.

Checks:
- Clause catalog updates + drift checks.
- Unit tests for parser/binder legality matrix.

### Phase EVT2 - Compiler/binder implementation closure

Deliverables:
- Replace current diagnostic-only gates for in-scope valid event patterns.
- Preserve stable diagnostics for true invalid patterns.
- Implement `Implements` coverage checks and emitted bind metadata needed by runtime dispatch.

Checks:
- Compiler tests for legal and illegal `WithEvents`/`RaiseEvent`/`Implements` cases.
- `compile_project` and source-compile parity tests.

### Phase EVT3 - Runtime event graph and dispatch semantics

Deliverables:
- Runtime event subscription graph:
  - assignment/reassignment behavior,
  - deterministic handler ordering,
  - lifecycle tie-in with class initialize/terminate and object release.
- `RaiseEvent` runtime dispatch using declared event metadata.

Checks:
- Deterministic ordering fixtures (including reassignment).
- VM/JIT parity tests for event flows.

### Phase EVT4 - Embedded host event contract and code-behind routing

Deliverables:
- Host-facing event bridge contract:
  - register root/document-bound objects,
  - map object identity to module/code-behind target,
  - subscribe/unsubscribe hooks with deterministic lifecycle semantics.
- Minimal host adapter tests that exercise host-raised events into VBA handlers.

Checks:
- Integration tests with `ProjectManifest` project lanes.
- Deterministic error handling under missing handler/missing object scenarios.

### Phase EVT5 - COM-EVT-A implementation (blocking)

Deliverables:
- COM event connection baseline on Windows:
  - event source discovery,
  - connection point advise/unadvise wiring,
  - dispatch-style event callback routing into VBA handlers.
- Stable COM event diagnostic and error-code mapping.

Checks:
- Windows COM conformance lane with controlled event server fixtures.
- Replay-stability checks across repeated advise/unadvise cycles.

### Phase EVT6 - COM-EVT-B implementation/fallback policy

Deliverables:
- Implement non-dispatch source-interface event path where feasible, or:
- lock explicit unsupported policy with deterministic diagnostics and conformance fixtures.

Checks:
- Capability matrix records and explicit claim-tier statuses.
- No silent fallback from unsupported event-paths.

### Phase EVT7 - Conformance, oracle, and formal foldback

Deliverables:
- Event-specific lanes and fixture sets:
  - language-only event semantics,
  - embedded host event routing,
  - COM event callback behavior.
- Oracle probes for ambiguous ordering/binding details.
- Formal lane additions for event graph invariants and lifecycle safety.

Checks:
- Conformance lane pass report and refreshed evidence artifacts.
- Deferred oracle/formal register updates (if any).

### Phase EVT8 - Closure gate

Deliverables:
- Integrated gate run for event tracks.
- Divergence closure write-up:
  - close `DIV-0003` and `DIV-0004`, or provide explicit scoped deferral with unblock path.
- Documentation closure in PMR/COM/HAL docs + diagnostics taxonomy.

Checks:
- Integrated gate pass.
- Docs sync and drift checks pass.

## 6. Proposed diagnostics taxonomy additions

Language/binder:
- `PMR-E-EVENT-WITHEVENTS-ILLEGAL-CONTEXT`
- `PMR-E-EVENT-HANDLER-PREFIX-MISMATCH`
- `PMR-E-EVENT-RAISEEVENT-UNDECLARED`
- `PMR-E-IMPLEMENTS-MEMBER-MISSING`
- `PMR-E-IMPLEMENTS-PREFIX-MISMATCH`

Runtime/host:
- `PMR-E-EVENT-DISPATCH-TARGET-MISSING`
- `PMR-E-EVENT-SUBSCRIPTION-STATE-INVALID`

COM bridge:
- `COM-E-EVENT-CONNECTIONPOINT-MISSING`
- `COM-E-EVENT-ADVISE-FAILED`
- `COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH`
- `COM-E-EVENT-PATH-UNSUPPORTED`

## 7. Conformance lanes and artifact model

### Lane map

- `EV-L0`: legality and diagnostics matrix (`WithEvents`/`Implements`/`RaiseEvent`).
- `EV-L1`: runtime event ordering and reassignment (language-only).
- `EV-L2`: embedded host object event routing.
- `EV-L3`: COM-EVT-A callback routing and lifecycle.
- `EV-L4`: COM-EVT-B path or deterministic unsupported policy.
- `EV-L5`: VM/JIT parity for event-heavy corpus.
- `EV-L6`: robustness/stress lane (rapid subscribe/unsubscribe/error routing).

### Artifacts

- `docs/evidence/conformance/events/EVENTS_CONFORMANCE_RUN_<runid>.md`
- `docs/evidence/conformance/events/EVENTS_CONFORMANCE_RUN_<runid>.csv`
- `docs/evidence/conformance/events/lanes/EVENTS_<lane>_<runid>.md`
- `docs/evidence/conformance/events/lanes/EVENTS_<lane>_<runid>.csv`
- optional logs under `docs/evidence/conformance/events/lanes/*.log`

## 8. Formal lane policy for this workset

- Formal lanes run every cycle where event-runtime behavior is modified.
- Formal failures are non-blocking unless memory-safety soundness concerns appear.
- Deferred formal items must be tracked in `docs/evidence/formal/DEFERRED_GATES.md`.

Candidate event invariants:
- subscription graph does not leak handlers after unsubscribe/release,
- handler dispatch ordering remains deterministic under reassignment,
- event callback failures route through deterministic error state transitions.

## 9. Risk register

1. Risk: event ordering mismatches against VBA in reassignment edge cases.
   - Mitigation: explicit oracle probes + deterministic lane fixtures.
2. Risk: COM callback lifecycle leaks or stale subscriptions.
   - Mitigation: advise/unadvise stress lanes and lifecycle invariants.
3. Risk: hidden fallback between event paths masks unsupported behavior.
   - Mitigation: explicit policy and diagnostics; no silent fallback.
4. Risk: host bridge contract drifts from PMR semantics.
   - Mitigation: clause-linked host integration tests and doc drift checks.

## 10. Exit criteria

1. Valid `WithEvents`/`RaiseEvent`/`Implements` patterns execute in project-aware lanes.
2. Invalid patterns fail with stable diagnostics in documented taxonomy.
3. Embedded host event hookup works for root/document-bound object flows.
4. `COM-EVT-A` lane is implemented and evidence-backed on Windows.
5. `COM-EVT-B` has either implemented coverage or explicit deterministic defer record and unblock plan.
6. Event conformance lanes `EV-L0..EV-L6` are operational with artifacts.
7. Event divergences (`DIV-0003`, `DIV-0004`) are closed or explicitly re-scoped with evidence.

## 11. Initial command skeleton

```powershell
# Compiler + host event semantics
cargo test -p oxvba-compiler event_
cargo test -p oxvba-host event_

# Integration/event suite
./scripts/run-project-integration-suite.ps1 -CasePattern INTP-00[89]

# COM event lanes (Windows)
./scripts/run-com-early-conformance.ps1

# Full local validation path
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal -NoArtifacts
```

## 12. Dependencies and unblock notes

Dependencies already present:
- PMR project graph substrate,
- COM early-binding/type-library substrate,
- host/HAL policy and diagnostics infrastructure.

Known blockers: none architectural.  
Work complexity is primarily semantic and integration-heavy, not blocked by missing foundations.

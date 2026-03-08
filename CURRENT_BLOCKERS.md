# Current Blockers (Events Parity Closure)

Date: 2026-03-08
Run context: full events parity closure (non-COM + Windows COM)

## Blocker entries

### BLK-EVT-001: Runtime subscription graph execution model not yet available
- Title: Replace compile-time owner fanout with runtime-owned subscription graph dispatch.
- Impact:
  - Blocks full closure of `DIV-0004` sink-instance parity scope.
  - Blocks workset items `WI-B01`..`WI-B05` terminal semantics (true runtime subscribe/unsubscribe graph, arbitrary lifetime handling, non-bounded dispatch).
  - Prevents full parity claim for dynamic sink lifetimes beyond compile-time owner candidate approximation.
- Why blocked:
  - Current bytecode/VM model does not have a first-class event-dispatch instruction that can:
    - enumerate runtime subscriptions by source/event,
    - invoke handler targets dynamically with sink-owner identity,
    - preserve deterministic ordering and reentrancy semantics without compile-time fanout codegen.
- Exact unblocking steps:
  1. Approve runtime event IR shape (`DispatchEvent`-style instruction and metadata contract).
  2. Implement VM/JIT dynamic handler dispatch over runtime-owned subscription map.
  3. Rewire `RaiseEvent` lowering to emit runtime dispatch op instead of static owner fanout wrappers.
  4. Add lifecycle tests for dynamic creation/release/reassignment without compile-time owner bounds.

### BLK-COM-001: COM event callback parity lane requires dedicated transport completion
- Title: Complete Windows COM event callback lifecycle (`COM-EVT-A` and `COM-EVT-B`/explicit defer).
- Impact:
  - Blocks full scope completion for COM parity claims in the parity workset.
  - Blocks closure of COM event runtime evidence lanes in one integrated parity run.
- Why blocked:
  - Current run has non-COM/internal event semantics advanced, but full COM callback transport completion still needs:
    - connection-point lifecycle integration (`Advise`/`Unadvise`) in runtime event graph lane,
    - deterministic callback argument mapping from typelib event metadata,
    - explicit `COM-EVT-B` implementation or formal deterministic unsupported closure.
- Exact unblocking steps:
  1. Lock COM event bridge contract for callback ingress (`COM-EVT-A` required, `COM-EVT-B` implementation/defer decision).
  2. Implement callback ingress wiring into unified runtime event dispatch model.
  3. Add controlled COM event fixture lane and deterministic CI coverage for callback lifecycle + failure mapping.
  4. Reconcile oracle evidence and update divergence/deferred gates.

## Structured summary

- Blocker IDs/titles:
  - `BLK-EVT-001` — Runtime subscription graph execution model not yet available.
  - `BLK-COM-001` — COM event callback parity lane requires dedicated transport completion.
- Impact by milestone/phase:
  - Non-COM full parity closure: blocked at runtime dynamic graph architecture (`WI-B01`..`WI-B05` terminal closure).
  - COM parity closure: blocked at callback transport + fixture/evidence completion (`WI-D02`..`WI-D06`, `WI-E01`..`WI-E03`).
- Exact unblocking steps:
  - Approve dynamic event IR + runtime execution path.
  - Approve COM callback bridge policy for `COM-EVT-A/B`.
  - Implement + validate in CI/oracle lanes.
- Suggestions/questions for user:
  - Decide whether to prioritize `BLK-EVT-001` (dynamic runtime event IR) first, then fold COM on top, or run both in parallel tracks.
  - Confirm preferred `COM-EVT-B` policy target for this run: full implementation now vs explicit deterministic defer.

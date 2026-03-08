# Current Blockers (Events Parity Closure)

Date: 2026-03-08  
Run context: full events parity closure (non-COM + Windows COM)

## Status update

### BLK-EVT-001: Runtime subscription graph execution model
- Status: resolved in current run.
- Resolution summary:
  - Removed compile-time bounded owner fanout from `RaiseEvent` lowering.
  - Added runtime owner-iteration intrinsics:
    - `__oxvba_withevents_first_owner(source, binding)`
    - `__oxvba_withevents_next_owner()`
  - Wrapper lowering now iterates runtime owner bindings dynamically and dispatches handlers with sink-owner identity.
  - Added/updated compiler/optimizer/VM/host tests to lock deterministic behavior.

## Active blocker entries

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

- Active blocker IDs/titles:
  - `BLK-COM-001` — COM event callback parity lane requires dedicated transport completion.
- Impact by milestone/phase:
  - Non-COM dynamic owner dispatch path: unblocked and implemented.
  - COM parity closure: blocked at callback transport + fixture/evidence completion (`WI-D02`..`WI-D06`, `WI-E01`..`WI-E03`).
- Exact unblocking steps:
  - Approve COM callback bridge policy for `COM-EVT-A/B`.
  - Implement + validate callback lanes in CI/oracle evidence.
- Suggestions/questions for user:
  - Confirm preferred `COM-EVT-B` policy target for this run: full implementation now vs explicit deterministic defer.

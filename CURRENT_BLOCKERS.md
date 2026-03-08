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
- Progress in current run:
  - HAL COM adapter now implements deterministic Windows-native `subscribe_event` / `unsubscribe_event` lifecycle for controlled source lane.
  - Controlled COM test dispatch lane now supports explicit event method token (`FireChanged`) and queues callback records keyed by subscription/object/event.
  - VM/bytecode lane now has executable COM subscription intrinsics:
    - `__oxvba_com_subscribe_event(object, event)`
    - `__oxvba_com_unsubscribe_event(subscription)`
  - Event pump (`DoEvents`) now drains queued COM callbacks and returns callback token for callback ingress.
  - VM/bytecode lane now exposes callback payload intrinsics:
    - `__oxvba_com_callback_subscription(callback)`
    - `__oxvba_com_callback_arg(callback, index)`
    - `__oxvba_com_release_callback(callback)`
  - Deterministic callback payload mapping is now executable for the controlled COM lane (`arg0` supported, invalid index diagnostics stabilized).
  - Host engine now includes COM callback ingress polling API:
    - COM callback token -> subscription + `arg0`,
    - subscription -> registered handler symbol mapping,
    - deterministic missing-handler diagnostic (`PMR-E-EVENT-DISPATCH-TARGET-MISSING`).
  - Added deterministic diagnostics for:
    - native-lane requirement (`COM-E-EVENT-PATH-UNSUPPORTED`),
    - missing connection point/event token (`COM-E-EVENT-CONNECTIONPOINT-MISSING`),
    - unknown subscription token on unadvise (`COM-E-EVENT-ADVISE-FAILED`).
- Why blocked:
  - Current run has non-COM/internal event semantics advanced and COM lifecycle substrate implemented, but full COM callback transport completion still needs:
    - callback token ingress path into actual handler procedure execution path (ingress mapping exists; callback -> direct procedure invocation is not yet wired),
    - typelib-driven callback argument shape mapping from COM metadata into handler signature enforcement beyond current controlled lane,
    - explicit `COM-EVT-B` implementation or formal deterministic unsupported closure.
  - Immediate architecture blocker for first bullet:
    - host/runtime lane currently executes fixed project entrypoint and lacks an in-runtime "invoke procedure by symbol in existing instance graph" primitive,
    - without that primitive, callback ingress can map to handler symbol but cannot dispatch into live project state with VBA-parity object identity semantics.
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

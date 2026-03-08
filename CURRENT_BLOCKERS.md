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
  - Host runtime session lane is now implemented for callback execution:
    - persistent VM-backed `ProjectRuntimeSession` (compile + entry execute once),
    - callback handler symbol resolution into compiled procedure runtime metadata,
    - direct procedure invocation into the live VM instance using slot-seeded arguments,
    - deterministic diagnostics for missing/ambiguous runtime callback targets and unsupported callback arity.
  - COM callback payload contract is extended beyond fixed `arg0`:
    - HAL COM callback lane now exposes deterministic callback arity lookup (`event_callback_arity`),
    - callback payload storage now carries argument vectors with deterministic index diagnostics,
    - host callback ingress now fetches full callback argument vectors and enforces exact handler signature arity at runtime (`PMR-E-EVENT-CALLBACK-SIGNATURE-MISMATCH`).
  - `COM-EVT-B` now has explicit deterministic unsupported closure in controlled Windows lane:
    - subscribing source-interface token path returns deterministic unsupported diagnostic:
      - `COM-E-EVENT-PATH-UNSUPPORTED: source-interface COM event callbacks (COM-EVT-B) are unsupported in current lane`.
  - Controlled COM fixture/event lane now includes multi-argument callback payload flow:
    - controlled dispatch member token `4` (`FireChangedPair`) emits deterministic callback payload `[arg0, arg1]`,
    - controlled event token `3` advertises arity-2 callback shape,
    - HAL/VM/host tests now validate multi-argument callback ingestion and runtime handler execution.
  - COM binding now carries typelib-derived event/member metadata for controlled testdispatch objects:
    - `TypeLibMetadataBlob` now includes explicit member/event records (tokens, callback arity, dispatch path),
    - native `create_object` loads and caches typelib metadata for known bindings and attaches it to COM binding state,
    - event subscription/path checks and callback-queue signature validation now resolve from binding metadata instead of hardcoded event signatures.
  - Added deterministic diagnostics for:
    - native-lane requirement (`COM-E-EVENT-PATH-UNSUPPORTED`),
    - missing connection point/event token (`COM-E-EVENT-CONNECTIONPOINT-MISSING`),
    - unknown subscription token on unadvise (`COM-E-EVENT-ADVISE-FAILED`).
- Why blocked:
  - Current run has non-COM/internal event semantics advanced, COM lifecycle substrate implemented, callback->handler runtime invocation wired, and deterministic COM-EVT-B unsupported policy in place.
  - Remaining parity closure still needs:
    - registered/external evidence expansion for callback bridge coverage beyond controlled fixture lane.
- Exact unblocking steps:
  1. Add registered/external COM event fixture coverage for callback lifecycle + failure mapping (`WI-E03`).
  2. Reconcile oracle evidence and update divergence/deferred gates.

## Structured summary

- Active blocker IDs/titles:
  - `BLK-COM-001` — COM event callback parity lane requires dedicated transport completion.
- Impact by milestone/phase:
  - Non-COM dynamic owner dispatch path: unblocked and implemented.
  - COM parity closure: callback transport + arity/signature enforcement + controlled multi-arg fixture lane implemented with explicit COM-EVT-B unsupported policy, and controlled lane now binds event/member shape through typelib metadata; blocked at registered/evidence completion (`WI-E03`).
- Exact unblocking steps:
  - Implement + validate registered/external callback lanes in CI/oracle evidence.
- Suggestions/questions for user:
  - Confirm which registered COM library lane to prioritize for external parity evidence first (Excel, ScriptControl-style, or another deterministic candidate).

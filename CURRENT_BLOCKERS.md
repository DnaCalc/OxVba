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
  - Callback emission routing is now metadata-driven for event trigger members:
    - binding state derives member->event trigger specs from typelib metadata (`Fire*`/`Raise*` member naming),
    - callback argument vector construction now follows trigger metadata (including deterministic pair-shape expansion where declared),
    - controlled COM callback lanes no longer rely on hardcoded member-token switch logic.
  - Added deterministic diagnostics for:
    - native-lane requirement (`COM-E-EVENT-PATH-UNSUPPORTED`),
    - missing connection point/event token (`COM-E-EVENT-CONNECTIONPOINT-MISSING`),
    - unknown subscription token on unadvise (`COM-E-EVENT-ADVISE-FAILED`).
  - Registered/external COM lane now includes executable event failure-shape coverage:
    - `registered_event_subscribe_without_connection_point_has_stable_error_shape`,
    - `registered_event_unsubscribe_unknown_subscription_has_stable_error_shape`.
  - Registered-mode event callback success lane is now executable and scriptable:
    - ignored test `registered_event_callback_success_when_event_capable_server_is_configured`,
    - strict success mode via env contract:
      - `OXVBA_REGISTERED_EVENT_REQUIRE_SUCCESS=1`,
      - `OXVBA_REGISTERED_EVENT_TOKEN`,
      - `OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER`,
      - `OXVBA_REGISTERED_EVENT_TRIGGER_ARG`,
    - script lane `scripts/run-com-registered-events.ps1` (`L2E`) and orchestrator support in `scripts/run-com-conformance.ps1 -IncludeRegisteredEventLane`.
  - Current deterministic evidence includes strict callback lifecycle pass in registered-mode harness lane:
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestDispatch_20260308T174736Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_LOG_OxVba.TestDispatch_20260308T174736Z.txt`.
  - Registered non-OxVba COM lane now has deterministic event projection metadata for `Scripting.Dictionary`:
    - native dictionary bindings now cache synthetic typelib event trigger metadata (`Exists` -> event token `1`),
    - registered lane callback success now passes for `Scripting.Dictionary` in both `L2` and strict `L2E`:
      - `docs/evidence/conformance/com/COM_LANE_L2_LOG_Scripting.Dictionary_20260308T190000Z.txt`,
      - `docs/evidence/conformance/com/COM_LANE_L2E_LOG_Scripting.Dictionary_20260308T190000Z.txt`.
  - Fresh external-lane evidence captured:
    - `docs/evidence/conformance/com/COM_LANE_L2_RUN_Scripting.Dictionary_20260308T174630Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2_LOG_Scripting.Dictionary_20260308T174630Z.txt`.
  - Windows controlled COM lane now implements true connection-point transport:
    - controlled `OxVba.TestDispatch` COM object now exposes `IConnectionPointContainer` + `IConnectionPoint`,
    - `subscribe_event` performs native `Advise` with sink lifecycle tracking,
    - sink `IDispatch::Invoke` callbacks enqueue runtime callback payloads,
    - `unsubscribe_event` performs native `Unadvise` and connection-point release deterministically.
  - Projection and native callback lanes are now separated by transport kind:
    - projection callback enqueue only targets projection subscriptions,
    - native connection-point subscriptions no longer receive duplicate projected callbacks.
  - Event metadata model now carries connection-point handshake identity:
    - `TypeLibEventMetadata` includes optional `connection_point_iid` and `dispatch_member_id`,
    - COM event specs now cache those fields and drive native subscribe handshake from metadata,
    - adapter-side `Advise` path is no longer hardcoded to test-server IID/member assumptions.
  - Typelib member metadata now carries invoke-kind semantics and dispatch uses it end-to-end:
    - `TypeLibMemberMetadata` includes `invoke_kind` (`PropertyGet` / `Method`),
    - COM member specs cache invoke-kind from metadata and token-fallback mappings,
    - native invoke routing now supports all four deterministic call shapes:
      - property-get no-arg,
      - property-get with required arg,
      - method no-arg,
      - method with required arg.
  - Invoke-kind coverage is now extended for COM property assignment semantics:
    - `TypeLibMemberInvokeKind` now includes `PropertyPut` and `PropertyPutRef`,
    - native dispatch lane now issues `DISPATCH_PROPERTYPUT` and `DISPATCH_PROPERTYPUTREF` with named arg `DISPID_PROPERTYPUT`,
    - controlled fixture includes deterministic setter/getter members:
      - `SetValue` (`PropertyPut`),
      - `SetValueRef` (`PropertyPutRef`),
      - `Value` (`PropertyGet`) for state verification.
    - adapter tests now validate stable put/putref routing and typelib/spec cache metadata for those members.
  - Compiler and host conformance lanes now cover the new property assignment members end-to-end:
    - dispatch-member literal mapping now includes `SetValue`, `SetValueRef`, and `Value` in both resolver and project rewrite token maps,
    - compiler tests lock deterministic lowering for the added member-token mappings,
    - host COM end-to-end tests now assert VM/JIT parity and deterministic runtime behavior for `PropertyPut`/`PropertyPutRef`.
  - Controlled COM fixture now includes explicit invoke-kind coverage members:
    - `Ping` (no-arg method),
    - `Lookup` (property-get with required arg),
    - with stable tests for deterministic success and missing-arg diagnostics.
  - Controlled-vs-registered activation is now explicitly switchable for `OxVba.TestDispatch`:
    - HAL honors `OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH=1` to bypass in-process fixture activation and require `CLSIDFromProgID` + `CoCreateInstance`,
    - conformance script lanes can forward this mode (`-ForceRegisteredTestDispatch`) for true external-server probing.
  - External true-registration probe captured and archived:
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestDispatch_20260308T193727Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_LOG_OxVba.TestDispatch_20260308T193727Z.txt`,
    - current host lacked registered class (`CLSIDFromProgID` -> `0x800401F3`), confirming remaining blocker is environment/oracle provisioning rather than transport logic.
  - Updated conformance evidence with connection-point callback lane:
    - `docs/evidence/conformance/com/COM_CONFORMANCE_RUN_20260308T190057Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2B_RUN_20260308T190057Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestDispatch_20260308T190057Z.md`.
- Why blocked:
  - Current run closes controlled-lane connection-point callback ingress and deterministic lifecycle (`Advise`/`Unadvise`) in Windows native mode.
  - Remaining strict parity closure is now limited to external evidence and ingestion:
    - populate real external typelib event metadata with connection-point IID/member IDs,
    - validate true connection-point callback evidence on at least one external registered COM server beyond the controlled in-process test server.
  - Forced external activation probe currently fails in this environment because no registered `OxVba.TestDispatch` class is available.
- Exact unblocking steps:
  1. Provision at least one event-capable registered COM server in the validation environment (either register `OxVba.TestDispatch` externally or select a deterministic third-party server).
  2. Populate external typelib metadata with connection-point IID/member identity (per chosen external server lane).
  3. Capture reproducible external true connection-point callback evidence (with `-ForceRegisteredTestDispatch` where applicable) and fold into conformance docs.
  4. Reconcile oracle evidence and update divergence/deferred gates.

## Structured summary

- Active blocker IDs/titles:
  - `BLK-COM-001` — COM event callback parity lane requires dedicated transport completion.
- Impact by milestone/phase:
  - Non-COM dynamic owner dispatch path: unblocked and implemented.
  - COM parity closure: callback transport + arity/signature enforcement + controlled multi-arg fixture lane implemented with explicit COM-EVT-B unsupported policy, and controlled + non-OxVba registered metadata-projection lanes now pass; blocked at true connection-point transport completion.
- Exact unblocking steps:
  - Implement + validate true connection-point callback transport in CI/oracle evidence (metadata-projection success lanes are covered in controlled and non-OxVba registered configurations).
- Suggestions/questions for user:
  - Confirm which registered COM library lane to prioritize for external parity evidence first (Excel, ScriptControl-style, or another deterministic candidate).

# Current Blockers

Date: 2026-03-11  
Run context: active parity/compliance execution plus in-progress feature worklist execution pass

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

### BLK-RUNTIME-VALUE-MODEL-001: Runtime value-model migration
- Status: resolved in current run.
- Resolution summary:
  - VM/register/host execution is now value-first end to end:
    - register storage persists `RuntimeValue`,
    - public VM/JIT/host execution APIs are semantic-snapshot first,
    - `snapshot_slots(...)` survives only as an explicit compatibility projection.
  - The interpreter loop no longer executes through the old raw slot-helper vocabulary:
    - core compare/boolean/jump/increment lanes now read/write semantic runtime values,
    - the wider loop now uses explicit legacy-projection helpers over `RuntimeValue` where scalar compatibility is still intentional,
    - `CopySlot` now preserves full `RuntimeValue` shape instead of collapsing through the integer lane.
  - The owned runtime `Variant` bridge now honestly covers the current scalar/error subset:
    - `Empty`,
    - `Null`,
    - `ErrorCode`,
    - `I32`,
    - `Bool`.
  - The dynamic-object protocol blocker that followed this migration is now also resolved:
    - native project class methods, properties, and default-member dispatch all execute on the shared dynamic-object protocol.

## Active blocker entries

### BLK-COM-IDISPATCH-001: Late-bound COM parity remains below VBA/Excel `IDispatch` behavior
- Impact:
  - Blocks `IP-03` Windows late-bound COM client parity.
  - Blocks full closure of `HAL-DYN-008` and parts of `IP-09` declare/marshaling parity.
  - Blocks downstream property/default-member closure work in `IP-02`.
- Current state:
  - `oxvba-com` invoke transport now carries per-argument name and omission metadata,
  - bytecode `IntrinsicDispatchInvokeHost` now preserves per-argument slot/name metadata,
  - VM host invoke construction now forwards that metadata into `ComInvokeRequest`,
  - Windows native adapter now supports general named-argument `DISPPARAMS` packing for member-known method/property-get lanes,
  - member-known property-put/property-putref lanes now canonicalize fully named/indexed arguments so the property value does not depend on caller argument order,
  - expression-form `DispatchInvoke(...)` assignments now preserve named trailing COM arguments instead of rejecting the statement outright,
  - omitted-argument metadata now survives the invoke request and yields deterministic required-argument faults,
  - controlled `IDispatch` variant roundtrips now cover `VT_NULL` and `VT_ERROR` in addition to the existing scalar subset,
  - controlled/native result conversion now also accepts `VT_I2` and `VT_UI2` into the current integer-token lane,
  - invoke failure translation now distinguishes real `ArgErr` presence from the previous synthetic `arg_err=0` fallback,
  - controlled `DISP_E_EXCEPTION` lanes now preserve bounded `EXCEPINFO` source/description/scode details in the adapter-fault surface,
  - explicit `DispatchInvoke(obj, 0, name := value)` now routes through authoritative default-member metadata when the binding exposes one,
  - natural late-bound default-member calls with named arguments remain compile-time blocked because that syntax still does not recover authoritative default-member identity before lowering,
  - broad object/interface-pointer and full `VARIANT`/`SAFEARRAY` marshalling remain below parity target,
  - `Invoke` result fidelity still lacks the broader `VarResult` surface and richer external automation `ExcepInfo`/argument-fault coverage required for Office-style automation parity.
- Exact unblock steps:
  - recover authoritative default-member identity for natural late-bound syntax and non-metadata-backed bindings,
  - complete full `VARIANT`/object/`SAFEARRAY` marshalling,
  - complete broader external `Invoke` error/result fidelity beyond the controlled exception/argument-fault subset.
- Recommendation:
  - continue with the late-bound COM completion workset together with the shared dynamic-object protocol/value-carrier workset so broader `VARIANT`/object/`SAFEARRAY` marshalling lands on the right runtime contract.

### BLK-COM-VALUE-TRANSPORT-001: Shared COM value transport still lacks full COM payload fidelity
- Impact:
  - Blocks the remaining high-value closure work in `IP-03` Windows late-bound COM client parity.
  - Blocks practical SAFEARRAY/object/string COM transport and therefore parts of `IP-04` COM extraction and `IP-09` marshaling parity.
- Current state:
  - `oxvba-com` now exposes an executable generic dynamic-object protocol API (`DynamicCallRequest`, `DynamicMemberSelector`, `DynamicCallKind`, `DynamicEventPayload`) with conversions from the current COM request/payload structs,
  - `oxvba-com` now owns a first semantic carrier slice via `ComValue`,
  - `oxvba-com` now also owns the extracted Windows `VARIANT`/one-dimensional `SAFEARRAY` translation bridge for the currently supported scalar/string/array subset,
  - `oxvba-com` now classifies Invoke-owned Windows result `VARIANT`s into either semantic `ComValue` results or dispatch-capable object pointers before HAL-owned binding state is applied,
  - `oxvba-com` now also owns shared Windows COM invoke failure and `EXCEPINFO` capture types/helpers, reducing the remaining wire/error mechanics left in HAL,
  - the canonical runtime-value `IDispatch::Invoke` helper for the semantic COM request path now also lives in `oxvba-com`, while HAL retains only object-handle resolve/bind state around that call,
  - `ComInvokeArg.value` and `ComCallbackPayload.args` no longer use raw `i32` tokens at the shared COM boundary,
  - VM `DispatchInvoke` construction now preserves `Empty`/`Null`/`CVErr(...)`/array-intent shape and runtime strings into that carrier instead of flattening them before the COM boundary,
  - `SafeArray` carrier values can now preserve owned semantic element payloads instead of only length/dimension shape,
  - Windows COM invoke/result translation now maps that carrier to and from `VARIANT` for the supported subset, including BSTR string arguments/results, and callback payload polling returns the same carrier family,
  - Windows COM invoke/result translation now also supports owned one-dimensional `VT_ARRAY | VT_VARIANT` payloads end to end on the helper and controlled `EchoVariant` invoke lanes,
  - native late-bound COM argument marshalling now clears temporary `VARIANT` invoke arguments after dispatch so BSTR-backed calls do not leak adapter-owned allocations,
  - `ComValue` now preserves `ObjectHandle(...)` semantically instead of degrading it back into plain integers before the COM boundary,
  - Windows native COM argument marshalling now resolves `ObjectHandle(...)` through adapter-owned binding state and emits `VT_DISPATCH` with balanced `AddRef`/`VariantClear` ownership for native COM-backed objects,
  - Windows native COM invoke result conversion now binds `VT_DISPATCH` results back into adapter-owned object handles on the runtime-value path instead of discarding them into the legacy scalar lane,
  - Windows native COM invoke result conversion now also binds `VT_UNKNOWN` results back into adapter-owned object handles when the returned interface exposes `IDispatch`,
  - the runtime value model itself is now semantic/value-first, but COM wire translation still only covers the currently supported subset,
  - length-only array intent still falls back to the old placeholder integer projection because only owned semantic array payloads can be marshalled honestly today,
  - broader interface-pointer result forms that do not expose `IDispatch` still do not traverse the shared runtime-facing carrier,
  - callback ingress now preserves the shared carrier at the COM boundary, but broader multi-dimensional/non-`VT_VARIANT` SAFEARRAY, object/interface, and external automation payload fidelity remain partial.
- Exact unblock steps:
  - extend the first `ComValue` slice into the full canonical OxVba-side external-call carrier for:
    - broader non-`IDispatch` object/interface-pointer result forms and identity roundtrip,
    - broader SAFEARRAY ranks/element vartypes and external automation payloads,
    - broader scalar/variant categories,
  - thread the new dynamic-object protocol and expanded carrier through compiler bytecode, VM host invoke construction, callback transport, and host runtime ingestion without making raw COM wire structs the VM/compiler value model,
  - move `VARIANT`/`BSTR`/`SAFEARRAY`/interface-pointer translation into `oxvba-com`,
  - continue extracting the remaining dispatch/object-binding-specific binding and state seams out of `standard.rs`,
  - align COM-backed object calls to the shared late-bound object protocol instead of preserving a COM-special runtime lane,
  - contract HAL toward delegation/bootstrap once the new carrier is in place,
  - then reopen practical SAFEARRAY/object/string late-bound COM work on top of that carrier.
- Recommendation:
  - treat the next implementation slice as the work defined in `docs/worksets/WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md`, not another adapter-local patch.

### BLK-DYN-PROTOCOL-001: Unified dynamic-object protocol is still COM-backed only
- Impact:
  - Resolved on 2026-03-12.
- Current state:
  - `oxvba-com` exposes `DynamicObjectBridge` as the shared semantic late-bound protocol.
  - COM-backed calls still route through `HalComDynamicBridge`.
  - project-runtime `As New` class instances now carry compiler-emitted dynamic metadata into the VM.
  - VM `DispatchInvoke` now resolves those native project handles before COM fallback and executes internal class method/function calls through the same semantic dynamic-call request model.
- Exact unblock steps:
  - none for this blocker.
- Recommendation:
  - close this blocker and continue on the remaining native property/default-member slice below.

### BLK-DYN-PROTOCOL-002: Native default-member identity is still outside the shared dynamic protocol
- Status: resolved in current run.
- Resolution summary:
  - `compile_project(...)` now parses member-level `Attribute <Member>.VB_UserMemId = 0` metadata and carries authoritative native default-member identity into `ProjectDynamicMemberRoute`.
  - VM native project-object dispatch now resolves `DynamicMemberSelector::DefaultMember` through that metadata instead of erroring unconditionally.
  - Native project-class method/function/property/default-member calls now all execute on the same shared semantic dynamic-call protocol before any COM fallback.
  - Added end-to-end host coverage for:
    - native `Property Get` / `Property Let` dispatch through explicit `DispatchInvoke(...)`,
    - native default-member dispatch through explicit `DispatchInvoke(obj, 0, ...)`,
    - stateful `As New` class construction with `Class_Initialize`.

### BLK-COM-BOUNDARY-001: Final oxvba-com extraction is blocked on the remaining live Windows COM execution seam
- Impact:
  - Blocks IP-04 final COM ownership extraction from HAL.
  - Blocks IP-05 early-binding completion, IP-06 server/export parity, and part of IP-08 hosting parity.
- Current state:
  - shared transport/types, deterministic typelib catalog logic, supported Windows wire/value/invoke helpers, generic callback/subscription runtime state, metadata cache ownership, known-member/event policy, metadata-driven ComBinding assembly, activation-time binding insertion, bound-dispatch lookup/rebinding, DISPID cache mutation, object release bookkeeping, subscription callback-pruning, event transport-choice resolution, and bound/unbound COM invoke-policy planning now live materially in oxvba-com,
  - oxvba-hal::standard no longer owns the high-level default-member/direct-DISPID/member-spec routing rules, but it still owns the live Windows IDispatch execution seam and the public COM-facing HAL contract,
  - the remaining work is HAL rebinding/contraction plus movement of the last execution/lifecycle authority behind an oxvba-com surface,
  - forcing closure early would freeze a still-transitional contract.
- Exact unblock steps:
  - continue moving the remaining Windows client contract authority out of standard.rs:
    - resolved-member DISPID lookup/cache update and raw IDispatch execution routing,
    - final object-handle resolve/bind lifecycle around COM invoke results,
    - public HAL COM contract contraction/rebinding,
  - contract the public HAL COM surface down to delegation/bootstrap seams over oxvba-com,
  - continue late-bound/property/reference-facade parity work on top of that contracted boundary.
- Recommendation:
  - keep using the runtime-protocol, reference-facade, and COM extraction worksets as the cleanup spine; the remaining blocker is now live execution/contract rebinding, not planning/event/binding-table authority.



### BLK-PROP-001: Property/default-member intent model is not yet end-to-end executable
- Impact:
  - Blocks `IP-02` VBA property model and default-member semantics.
  - Blocks part of `IP-06` COM server/export parity and `IP-08` Office-style hosting parity.
- Current state:
  - property get/put/putref lanes exist in parts of the COM client path,
  - but there is no fully closed end-to-end model for `Property Get/Let/Set`, `Set` vs `Let`, default-member resolution source of truth, and call-vs-value context parity.
- Exact unblock steps:
  - close the late-bound invoke transport gap,
  - lock runtime/binder property intent transport,
  - implement and test default-member and `Set`/`Let` semantics across compiler/runtime/host/COM.
- Recommendation:
  - treat this as the first major semantic closure track after the late-bound COM transport redesign.

### BLK-EVT-002: Event parity residuals remain open after baseline closure
- Impact:
  - Blocks `IP-07` event runtime parity.
  - Blocks part of `IP-08` host project / Office-style hosting parity.
- Current state:
  - baseline event runtime work is stronger, but open residuals remain:
    - `DIV-0004`
    - `ODG-038`
    - `ODG-039`
    - remaining COM adapter parity lanes
- Exact unblock steps:
  - finish unified host ingress behavior,
  - close remaining COM callback/event transport residuals,
  - resolve or bound the remaining divergence/oracle topics.
- Recommendation:
  - continue after host/object/event ingress and COM transport work are stable.

### BLK-ORACLE-001: Oracle closure depends on unfinished implementation areas and external captures
- Impact:
  - Blocks `IP-10` oracle/differential parity closure.
  - Prevents full parity claims for `IP-03`, `IP-05`, `IP-07`, and `IP-09`.
- Current state:
  - deferred oracle structure exists and some probes are captured,
  - but required Office/host differential captures cannot close meaningfully while the underlying behavior is still unfinished.
- Exact unblock steps:
  - finish the feature work for the affected areas,
  - run the remaining Office/host capture matrix,
  - fold results back into claim docs and divergence registers.
- Recommendation:
  - do not spend oracle effort ahead of core feature closure except for targeted ambiguity resolution.

### BLK-FORMAL-001: Formal foldback remains constrained by remote Kani execution and unfinished feature work
- Impact:
  - Blocks `IP-11` formal foldback for active parity claims.
  - Blocks final umbrella closure for `IP-01`.
- Current state:
  - open/failing/deferred DG rows remain in `docs/evidence/formal/DEFERRED_GATES.md`,
  - some lanes require remote Linux/Kani execution,
  - other lanes cannot close honestly until the underlying feature behavior is finished.
- Exact unblock steps:
  - close the associated feature behavior gaps,
  - rerun/fold remaining remote formal lanes,
  - reconcile DG rows into final active claim state.
- Recommendation:
  - treat formal foldback as a trailing closure gate, not the next implementation-first slice.

## Closed blocker entries

### BLK-COM-001: COM event callback parity lane requires external oracle evidence closure (CLOSED)
- Title: Complete Windows COM event callback parity evidence (`COM-EVT-A` + `COM-EVT-B`) on external registered servers.
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
  - `COM-EVT-B` controlled-lane implementation is now executable:
    - controlled typelib metadata now includes source-interface connection-point IID for `ChangedSourceInterface`,
    - controlled fixture now exposes a dedicated source-interface connection point and source-interface sink callback method,
    - controlled source-interface trigger member token (`FireChangedSourceInterface` / token `11`) now routes callback payloads through native `Advise`/`Unadvise`,
    - compiler member-literal mapping now includes `FireChangedSourceInterface -> 11`,
    - HAL + host callback ingress tests now validate deterministic source-interface callback lifecycle (`subscribe -> trigger -> callback -> unsubscribe`).
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
  - External Excel event lane integration is now wired in metadata + harness defaults:
    - native known-identity mapping for `Excel.Application` / `excel.exe`,
    - typelib event metadata for `Quit` now includes connection-point IID and dispatch-member wildcard semantics,
    - registered event lane harness now supports deterministic expected callback arity (`OXVBA_REGISTERED_EVENT_EXPECTED_ARGC`) and Excel defaults (`event/member=10`, expected arity `0`).
  - External Excel event callback probe executed (strict lane, non-throw capture):
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_Excel.Application_20260308T202040Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_LOG_Excel.Application_20260308T202040Z.txt`.
  - Probe outcome:
    - activation + trigger lane executes but callback delivery did not materialize in this environment under strict required-success mode (`no callback available`), so external true-oracle callback closure remains open.
  - Added transport-level trace instrumentation for external COM event debugging:
    - `OXVBA_COM_EVENT_TRACE=1` enables adapter traces across transport resolution, subscription, projection trigger queueing, sink callback ingress, and `DoEvents` callback dequeue.
    - Registered-event script lane exposes this as `-EnableTrace`.
  - Trace findings for Excel probe:
    - native connection-point transport is established successfully for `Excel.Application` (`resolve-transport ... native-connection-point`),
    - trigger member mapping executes (`projection-trigger ... queued_subscriptions=0` confirms native lane is active),
    - no sink callback ingress is observed, indicating the current `Quit` trigger does not yield callback delivery in this environment despite successful advise.
  - Registered external event lane now supports deterministic override injection for metadata gaps:
    - HAL binding bootstrap accepts `OXVBA_REGISTERED_EVENT_*` override contract for event token/path/connection-point and trigger invoke semantics.
    - Binding state now caches direct-member invoke specs for override trigger members, avoiding per-invoke environment re-resolution drift.
    - Registered event scripts now expose override controls:
      - `EventPath` / `OXVBA_REGISTERED_EVENT_PATH`,
      - `ConnectionPointIid` / `OXVBA_REGISTERED_EVENT_CONNECTION_POINT_IID`,
      - `DispatchMember` / `OXVBA_REGISTERED_EVENT_DISPATCH_MEMBER`,
      - `TriggerRequiresArg` / `OXVBA_REGISTERED_EVENT_TRIGGER_REQUIRES_ARG`,
      - `TriggerInvokeKind` / `OXVBA_REGISTERED_EVENT_TRIGGER_INVOKE_KIND`.
  - Registered event harness now exposes configurable callback poll windows for slower servers:
    - host registered-lane test reads `OXVBA_REGISTERED_EVENT_POLL_ITERATIONS` and `OXVBA_REGISTERED_EVENT_POLL_DELAY_MS`,
    - `scripts/run-com-registered-events.ps1` and `scripts/run-com-conformance.ps1` surface these as `PollIterations` and `PollDelayMs`.
  - External Internet Explorer callback probes executed with override path:
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_InternetExplorer.Application_20260308T213000Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_InternetExplorer.Application_20260308T213200Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_InternetExplorer.Application_20260308T214000Z.md`.
  - Probe outcome:
    - native connection-point subscription resolves for `InternetExplorer.Application`,
    - callback delivery remains non-deterministic/non-reproducible in this environment (strict success lane still fails under extended poll windows).
- Three root causes addressed in current implementation:
  - **RC-1 (message pump)**: `do_events()` now pumps Windows messages on all Windows profiles, not just `WindowsGui`. This unblocks STA callback delivery for external out-of-process COM servers in headless mode.
  - **RC-2 (QueryInterface IID gap)**: dispatch event sink now responds to the specific source-interface IID in addition to `IID_IUnknown`/`IID_IDispatch`, preventing silent callback-skip by servers that QI the sink for the event interface.
  - **RC-3 (no deterministic external server)**: dedicated `OxVba.TestEventServer` COM server created at `tools/OxVba.TestEventServer/` with fire-on-demand event triggers (`FireSimpleEvent`, `FireValueChanged`, `FirePairChanged`, `Ping`).
  - HAL typelib metadata mapping added for `OxVba.TestEventServer` with full event/trigger/member specs.
  - Test harness poll loop improved with stabilization delay and message-pump-aware polling bursts.
  - Script defaults updated for external server poll tuning.
- Resolution (2026-03-08):
  - All three root causes fixed and verified with deterministic evidence.
  - Evidence artifacts:
    - Zero-arg (OnSimpleEvent): `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestEventServer_20260308T223239Z.md`
    - Single-arg (OnValueChanged): `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestEventServer_20260308T223250Z.md`
    - Pair-arg (OnPairChanged): `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestEventServer_20260308T223358Z.md`

## Structured summary

- Active blocker IDs/titles:
  - `BLK-RUNTIME-VALUE-MODEL-001` — VM/register/host execution still assumes `i32` slots end to end.
- Impact by milestone/phase:
  - blocks further honest progress on `WORKSET_2026-03-11_RUNTIME_VALUE_MODEL_MIGRATION.md` beyond the already-landed wrapper, observation-surface, `WithEvents`, and COM-entry slices
  - blocks full closure of `WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md`
  - blocks parity-complete completion of late-bound COM/client work that depends on richer runtime-side object/string/array transport
- Exact unblocking steps:
  - replace or strictly extend the HAL `ValueToken = i32` contract with the canonical runtime value model or explicit indirection model
  - migrate the remaining HAL token-only call seams
  - migrate remaining VM/JIT/public caller and parity-harness expectations off the integer observation lane
- Suggestions/questions for the user:
  - no new product decision is required
  - the next work should be treated as a dedicated core-contract migration program, not another adapter-local cleanup slice
- Previously resolved blockers:
  - `BLK-EVT-001` — resolved (runtime subscription graph)
  - `BLK-COM-001` — resolved (COM event callback parity with external registered server evidence)








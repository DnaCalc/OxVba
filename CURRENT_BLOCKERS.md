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

### BLK-COM-BOUNDARY-001: Final oxvba-com extraction from HAL
- Status: resolved in current run.
- Resolution summary:
  - oxvba-com now exposes WindowsComBridge as the live Windows COM client facade.
  - standard.rs now delegates create-object activation, invoke execution, object description/release, event subscription/callback access, and typelib resolve/load/invalidate through that bridge.
  - native subscription transport teardown for object release now also executes inside oxvba-com, removing the last substantive COM lifecycle seam from HAL.
  - the remaining HAL COM code is limited to capability/policy gating, apartment/bootstrap hooks, selector-based fallback, and error mapping.
  - the IP-04 closure verification matrix is green:
    - cargo fmt --all,
    - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings,
    - cargo test -p oxvba-com -p oxvba-hal -p oxvba-host --quiet,
    - ./scripts/check-governance.ps1,
    - ./scripts/meta-check.ps1 -Fast -NoArtifacts.

## Active blocker entries

### BLK-COM-IDISPATCH-001: Late-bound COM parity remains below VBA/Excel `IDispatch` behavior
- Impact:
  - Blocks `IP-03` Windows late-bound COM client parity.
  - Blocks full closure of `HAL-DYN-008` and parts of `IP-09` declare/marshaling parity.
- Current state:
  - `oxvba-com` invoke transport now carries per-argument name and omission metadata,
  - bytecode `IntrinsicDispatchInvokeHost` now preserves per-argument slot/name metadata,
  - VM host invoke construction now forwards that metadata into `ComInvokeRequest`,
  - Windows native adapter now supports general named-argument `DISPPARAMS` packing for member-known method/property-get lanes,
  - member-known property-put/property-putref lanes now canonicalize fully named/indexed arguments so the property value does not depend on caller argument order,
  - expression-form `DispatchInvoke(...)` assignments now preserve named trailing COM arguments instead of rejecting the statement outright,
  - omitted-argument metadata now survives the invoke request and yields deterministic required-argument faults,
  - controlled `IDispatch` variant roundtrips now cover `VT_NULL` and `VT_ERROR` in addition to the existing scalar subset,
  - controlled/native result conversion now also accepts `VT_I1`, `VT_I2`, `VT_I4`, `VT_I8`, `VT_INT`, `VT_R4`, `VT_R8`, `VT_CY`, `VT_DATE`, `VT_DECIMAL`, `VT_UI1`, `VT_UI2`, `VT_UI4`, `VT_UI8`, and `VT_UINT`, with `VT_R4` / `VT_R8` / `VT_DATE` preserved on a first-class semantic `f64` carrier, `VT_CY` preserved on a new exact scaled-`i64` currency carrier, `VT_DECIMAL` preserved on a new exact `Decimal96` carrier, and the integer forms still narrowing into the current `i32` carrier when the payload fits,
  - controlled/native result conversion now also accepts one-dimensional typed SAFEARRAY results with `VT_I1`, `VT_I2`, `VT_I4`, `VT_I8`, `VT_INT`, `VT_R4`, `VT_R8`, `VT_CY`, `VT_DATE`, `VT_DECIMAL`, `VT_UI1`, `VT_UI2`, `VT_UI4`, `VT_UI8`, `VT_UINT`, `VT_BOOL`, and `VT_BSTR` element payloads into `RuntimeValue::ArrayIntent`, preserving `VT_R4` / `VT_R8` / `VT_DATE` elements on the same semantic `f64` carrier, preserving `VT_CY` elements on the exact scaled-`i64` currency carrier, preserving `VT_DECIMAL` elements on the exact `Decimal96` carrier, and still narrowing the integer payloads into the current `i32` carrier when they fit,
  - controlled/native host coverage now also proves deterministic bounded diagnostics when scalar VT_I8 / VT_UI4 / VT_UI8 / VT_UINT values or one-dimensional typed VT_ARRAY | VT_I8 / VT_UI4 / VT_UI8 / VT_UINT elements exceed the current i32 carrier lane instead of silently wrapping,
  - controlled/native host coverage now also proves a stable unsupported-path diagnostic for scalar and typed-array VT_BYREF result payloads instead of letting undocumented byref shapes drift through the adapter,
  - controlled/native object result conversion now has end-to-end host evidence for both `VT_DISPATCH` and `VT_UNKNOWN` values that expose `IDispatch`,
  - controlled/native result conversion now also has end-to-end host evidence for one-dimensional typed `VT_ARRAY | VT_DISPATCH` and `VT_ARRAY | VT_UNKNOWN` results when the element interfaces expose `IDispatch`,
  - outbound object-valued COM arguments now have end-to-end host evidence via a controlled raw-variant classifier method,
  - outbound scalar `True`, `BSTR`, `Empty`, `Null`, and `CVErr(...)` COM arguments now also have end-to-end host evidence via the same controlled raw-variant classifier lane,
  - outbound float/date/currency/decimal host evidence now proves the tagged semantic `f64` lane preserves exact outward `VT_R4`, `VT_R8`, and `VT_DATE` tags while `Currency` and `Decimal` preserve exact `VT_CY` / `VT_DECIMAL` tags,
  - outbound `Array(...)` expressions now have end-to-end host evidence as semantic `VT_ARRAY | VT_VARIANT` payloads via the controlled raw-variant classifier lane,
  - one-dimensional `VT_ARRAY | VT_VARIANT` payloads with nested `VT_DISPATCH` elements now have end-to-end host evidence on both argument and result paths via controlled classifier and return-array fixture members,
  - controlled host coverage now also proves a stable unsupported-path diagnostic for both rank-2 typed SAFEARRAY results and rank-2 `VT_ARRAY | VT_VARIANT` results instead of an unbounded crash or shape drift,
  - controlled host coverage now also proves a stable unsupported-path diagnostic when one-dimensional `VT_ARRAY | VT_VARIANT` results contain nested `VT_UNKNOWN` elements that do not expose `IDispatch`,
  - invoke failure translation now distinguishes real `ArgErr` presence from the previous synthetic `arg_err=0` fallback,
  - controlled `DISP_E_EXCEPTION` lanes now preserve bounded `EXCEPINFO` source/description/scode details in the adapter-fault surface,
  - controlled direct-error coverage now also proves VM/JIT host parity for real `arg_err` indexing on `DISP_E_TYPEMISMATCH`, bounded `EXCEPINFO` source/description/scode detail on `DISP_E_EXCEPTION`, stable `DISP_E_BADPARAMCOUNT` classification, stable `DISP_E_PARAMNOTFOUND` classification, stable `DISP_E_MEMBERNOTFOUND` classification, stable `DISP_E_UNKNOWNNAME` classification for runtime string member selectors, stable bounded `E_NOINTERFACE` classification for non-`IDispatch` `IUnknown::QueryInterface(IDispatch)` rejection, stable bounded internal carrier-overflow classification for out-of-lane integer automation results, stable bounded unsupported-`VT_BYREF` return classification, bounded runtime-string success for zero-arg method plus zero-arg/indexed property-get selectors via `DISP_E_BADPARAMCOUNT` flag fallback, bounded runtime-string named-argument success for method plus indexed property-get selectors, and bounded metadata-backed runtime-string default-member-name, property-put/property-putref, and indexed property-put/property-putref execution on the existing bound member path, while the adapter boundary also preserves the same stable `DISP_E_UNKNOWNNAME` classification for raw `GetIDsOfNames` failures,
  - explicit `DispatchInvoke(obj, 0, name := value)` now routes through authoritative default-member metadata when the binding exposes one,
  - natural late-bound default-member calls with named arguments now lower and execute when the bound COM object exposes authoritative default-member metadata,
  - controlled host coverage now also proves stable bounded `E_NOINTERFACE` classification when both single-value `VT_UNKNOWN` results and typed `VT_ARRAY | VT_UNKNOWN` elements do not expose `IDispatch`,
  - broad non-IDispatch interface-pointer handling, multi-dimensional SAFEARRAYs, non-IDispatch interface arrays, and fuller external VARIANT parity remain below target,
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
  - Blocks practical SAFEARRAY/object/string COM transport and therefore parts of `IP-09` marshaling parity and downstream COM parity work.
- Current state:
  - `oxvba-com` now exposes an executable generic dynamic-object protocol API (`DynamicCallRequest`, `DynamicMemberSelector`, `DynamicCallKind`, `DynamicEventPayload`) with conversions from the current COM request/payload structs,
  - `oxvba-com` now owns a first semantic carrier slice via `ComValue`,
  - `oxvba-com` now also owns the extracted Windows `VARIANT`/one-dimensional `SAFEARRAY` translation bridge for the currently supported scalar/string/array subset, including typed `VT_I2`, `VT_I4`, `VT_I8`, `VT_R4`, `VT_R8`, `VT_CY`, `VT_DATE`, `VT_DECIMAL`, `VT_UI2`, `VT_UI4`, `VT_UI8`, `VT_BOOL`, `VT_BSTR`, `VT_DISPATCH`, and `VT_UNKNOWN` SAFEARRAY result elements on the controlled `IDispatch`-exposing lane, a semantic `f64` result carrier for float/date payloads, an exact scaled-`i64` carrier for `VT_CY`, an exact `Decimal96` carrier for `VT_DECIMAL`, and checked scalar narrowing for the current `RuntimeValue::I32` carrier,
  - `oxvba-com` now classifies Invoke-owned Windows result `VARIANT`s into either semantic `ComValue` results or dispatch-capable object pointers before HAL-owned binding state is applied,
  - `oxvba-com` now also owns shared Windows COM invoke failure and `EXCEPINFO` capture types/helpers, reducing the remaining wire/error mechanics left in HAL,
  - the canonical runtime-value `IDispatch::Invoke` helper for the semantic COM request path now also lives in `oxvba-com`, while HAL retains only object-handle resolve/bind state around that call,
  - `ComInvokeArg.value` and `ComCallbackPayload.args` no longer use raw `i32` tokens at the shared COM boundary,
  - VM `DispatchInvoke` construction now preserves `Empty`/`Null`/`CVErr(...)`/array-intent shape and runtime strings into that carrier instead of flattening them before the COM boundary,
  - compiler lowering now materializes both VBA `Array(...)` literals and `ParamArray` packs as semantic `IntrinsicArrayLiteral` payloads rather than legacy count tags,
  - `SafeArray` carrier values can now preserve owned semantic element payloads instead of only length/dimension shape,
  - Windows COM invoke/result translation now maps that carrier to and from `VARIANT` for the supported subset, including BSTR string arguments/results, and callback payload polling returns the same carrier family,
  - Windows COM invoke/result translation now also supports owned one-dimensional `VT_ARRAY | VT_VARIANT` payloads end to end on the helper and controlled `EchoVariant` invoke lanes,
  - native late-bound COM argument marshalling now clears temporary `VARIANT` invoke arguments after dispatch so BSTR-backed calls do not leak adapter-owned allocations,
  - `ComValue` now preserves `ObjectHandle(...)` semantically instead of degrading it back into plain integers before the COM boundary,
  - Windows native COM argument marshalling now resolves `ObjectHandle(...)` through adapter-owned binding state and emits `VT_DISPATCH` with balanced `AddRef`/`VariantClear` ownership for native COM-backed objects,
  - Windows native COM invoke result conversion now binds `VT_DISPATCH` results back into adapter-owned object handles on the runtime-value path instead of discarding them into the legacy scalar lane,
  - Windows native COM invoke result conversion now also binds `VT_UNKNOWN` results back into adapter-owned object handles when the returned interface exposes `IDispatch`,
  - Windows native COM invoke/result translation now also preserves one-dimensional `VT_ARRAY | VT_VARIANT` payloads with nested `VT_DISPATCH` elements into runtime-owned `ArrayIntent` values on the controlled fixture lane,
  - the runtime value model itself is now semantic/value-first, but COM wire translation still only covers the currently supported subset,
  - length-only array intent still falls back to the old placeholder integer projection because only owned semantic array payloads can be marshalled honestly today,
  - broader interface-pointer result forms that do not expose `IDispatch` still do not traverse the shared runtime-facing carrier,
  - callback ingress now preserves the shared carrier at the COM boundary, and rank-2 SAFEARRAY results now fail with a deterministic unsupported-shape diagnostic on the controlled lane, but broader multi-dimensional SAFEARRAY support, non-`IDispatch` interface arrays/payloads, and richer external automation payload fidelity remain partial.
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
  - Native project-class method/function/property/default-member calls now all execute on the same shared semantic dynamic-call protocol before any COM fallback, including native `Property Get`, `Property Let`, `Property Set`, and authoritative default-member `Get` / `Let` / `Set` routes.
  - Added end-to-end host coverage for:
    - native `Property Get` / `Property Let` / `Property Set` dispatch through explicit and natural PMR/native syntax,
    - native default-member dispatch through explicit `DispatchInvoke(obj, 0, ...)`,
    - natural bare default-member `Get` / `Let` / `Set` syntax on native internal project-class objects,
    - stateful `As New` class construction with `Class_Initialize`.

### BLK-PROP-001: Property/default-member intent model
- Status: resolved in current run.
- Resolution summary:
  - The `IP-02` checklist audit is now complete in [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md).
  - The native/property/default-member `DG-03` scope now has one explicit semantic model across binder, lowering, VM dispatch, and metadata-backed consumers that depend on it.
  - `Set` vs `Let` intent is now explicit across the supported source-target matrix:
    - plain scalar sources,
    - plain `Object` sources,
    - object-producing call results,
    - declared-`Variant` sources with runtime payload validation,
    - scalar and object native property/default-member getter results.
  - Non-authoritative native default-member fallback is now closed for the supported scope:
    - single-visible-candidate fallback executes deterministically,
    - ambiguous and missing cases fail deterministically with `PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS` and `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING`,
    - unsupported no-parentheses RHS read-assignment forms fail deterministically on the existing `unsupported statement` surface.
  - Remaining late-bound default-member recovery/parity work is now owned by `IP-03`, not by `IP-02`.

### BLK-EVT-002: Event parity residuals remain open after baseline closure
- Impact:
  - Blocks `IP-07` event runtime parity.
  - Blocks part of `IP-08` host project / Office-style hosting parity.
- Current state:
  - baseline event runtime work is stronger, but open residuals remain:
    - explicit host-event ingress now executes bound handlers directly into live runtime sessions through source-instance-aware guard wrappers with deterministic ordering, bounded zero/one-argument forwarding, and bounded missing-target/arity diagnostics,
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

### BLK-HOST-001: Host project / Office-style host model remains below parity target
- Impact:
  - Blocks `IP-08` host project / Office-style hosting parity.
- Current state:
  - host bridge and tooling contracts are defined,
  - explicit host-event ingress now dispatches bound handlers into live runtime sessions for the current zero/one-argument subset,
  - host-injected referenced class modules marked `VB_PredeclaredId` or `VB_GlobalNamespace` now participate in bounded implicit receiver lowering for property/default-member read lanes,
  - the same host-injected root path now also has bounded executable `Property Let` coverage for named and authoritative default-member write lanes across both `VB_PredeclaredId` and `VB_GlobalNamespace`,
  - plain project references still remain on the ordinary unresolved-name path and do not gain implicit host-root behavior,
  - broader host project/root/global exposure rules, host object identity, callback routing, and wider statement/write behavior are still missing from the executable host model.
- Exact unblock steps:
  - extend the host-injected root/global rules across the supported write and statement-call lanes where intended,
  - make host project object identity and runtime session ownership explicit in executable behavior,
  - connect host-backed object identity to the live callback/event ingress path where the host foundation requires it,
  - continue through the checklist in [WORKSET_2026-03-19_IP-08A_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-19_IP-08A_EXECUTION_CHECKLIST.md).
- Recommendation:
  - keep `IP-08` focused on executable host substrate first, then widen parity breadth only after the foundation matrix is covered.

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















# In-Progress Feature Worklist

Date: 2026-03-10  
Status: active  
Purpose: canonical repo-level register of feature areas that remain `in-progress` under the workset completion doctrine in `OPERATIONS.md`.

This file is the authoritative consolidation point for part-implemented feature work.

Latest execution pass:
1. `docs/IN_PROGRESS_FEATURE_EXECUTION_2026-03-10.md`

Latest note (2026-03-18): the active `IP-03A` late-bound COM transport subset now includes controlled `VT_R4` / `Single`, `VT_R8` / `Double`, and `VT_DATE` scalar and one-dimensional typed SAFEARRAY result lanes on a tagged semantic `f64` carrier that now also preserves outward `Single` and `Date` vartype fidelity plus controlled `VT_CY` / `Currency` scalar and one-dimensional typed SAFEARRAY result lanes on an exact scaled-`i64` currency carrier and controlled `VT_DECIMAL` scalar and one-dimensional typed SAFEARRAY result lanes on an exact `Decimal96` carrier; named-result host evidence for scalar `VT_BOOL`, `VT_BSTR`, `VT_EMPTY`, `VT_NULL`, and `VT_ERROR` plus outbound classifier evidence for scalar `VT_BOOL`, `VT_BSTR`, `VT_EMPTY`, `VT_NULL`, and `VT_ERROR` arguments are also in place; scalar `VT_I8` / `VT_UI4` / `VT_UI8` / `VT_UINT` values, one-dimensional typed `VT_ARRAY | VT_I8` / `VT_UI4` / `VT_UI8` / `VT_UINT` overflow, rank-2 `VT_ARRAY | VT_VARIANT` results, nested non-`IDispatch` `VT_UNKNOWN` elements inside one-dimensional `VT_ARRAY | VT_VARIANT` results, and scalar or typed-array `VT_BYREF` result payloads now fail with deterministic bounded diagnostics instead of silently wrapping or drifting through the current adapter surface; bounded invoke-failure evidence now also covers stable `DISP_E_MEMBERNOTFOUND` and `DISP_E_BADPARAMCOUNT` classification on the host fault surface plus stable `DISP_E_UNKNOWNNAME` classification both at the adapter boundary for raw `GetIDsOfNames` failures and on the host fault surface for runtime string member selectors, and the runtime-string subset now also has bounded success evidence for zero-arg method, named-argument method, zero-arg property-get, indexed property-get, named-argument indexed property-get, metadata-backed default-member-name dispatch, metadata-backed property put/property putref selectors, and metadata-backed indexed property put/property putref selectors; the native/property/default-member `IP-02` semantic model is now closed through [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md), while broader external `VARIANT`, non-`IDispatch` interface, and multi-dimensional SAFEARRAY parity remain `in-progress` under `IP-03`.

Update (2026-03-18, later pass): `IP-02A` now also has direct bounded evidence that ambiguous/missing source-resolution diagnostics apply to non-authoritative object-valued default-member reads under explicit `Let` and implicit assignment to typed `Object` targets across bare, zero-arg parenthesized, and indexed syntax.
Update (2026-03-18, current pass): `IP-02A` now also has direct bounded evidence that the same ambiguous/missing source-resolution diagnostics apply to typed `Variant` targets under explicit `Let` and implicit assignment across bare, zero-arg parenthesized, and indexed syntax.
Update (2026-03-18, later current pass): `IP-02A` now also has direct bounded evidence that scalar-typed native property/default-member getter results reject explicit `Set` across typed `Variant`, `Object`, and scalar targets for named, zero-arg parenthesized, indexed, authoritative default-member, and bounded single-candidate non-authoritative default-member syntax.
Update (2026-03-18, latest pass): `IP-02A` now also has direct bounded evidence that scalar-typed native property/default-member getter results support explicit `Let` and implicit assignment into typed `Variant` and scalar targets, while rejecting both forms on typed `Object` targets, across named, zero-arg parenthesized, indexed, authoritative default-member, and bounded single-candidate non-authoritative default-member syntax.
Update (2026-03-18, current pass): plain declared-`Variant` source variables now also have direct bounded runtime-validated assignment-intent evidence across the current `Variant` / `Object` / scalar target lanes for both scalar-payload and object-payload shapes, and the optimizer no longer collapses those post-typecheck lanes into semantically different constant-source assignments.
Update (2026-03-18, current pass): no-parentheses getter calls on the native PMR/default-member path now also have direct bounded compile-time rejection evidence in RHS read-assignment contexts across named, authoritative default-member, and single-visible-candidate non-authoritative default-member receivers under both explicit `Let` and implicit assignment.
Update (2026-03-18, current pass): plain `Object`-typed source variables now also have direct bounded assignment-intent evidence across the current `Object` / `Variant` / scalar target lanes for explicit `Set`, explicit `Let`, and implicit assignment.
Update (2026-03-18, current pass): plain scalar sources now also have direct bounded assignment-intent evidence across the current typed scalar / `Variant` / `Object` target lanes for explicit `Set`, explicit `Let`, and implicit assignment.
Update (2026-03-18, current pass): object-returning native property/default-member getter results now also have direct bounded scalar-target rejection evidence for explicit `Set` across named, zero-arg parenthesized, indexed, authoritative default-member, and landed single-visible-candidate non-authoritative default-member syntax.
Update (2026-03-18, current pass): ambiguous/missing non-authoritative object-valued default-member source-resolution diagnostics now also have direct bounded explicit-`Set` evidence on both typed `Object` and typed `Variant` targets across bare, zero-arg parenthesized, and indexed syntax.
Update (2026-03-18, current pass): the same ambiguous/missing explicit-`Set` source-resolution diagnostics now also have direct bounded scalar-target precedence evidence across bare, zero-arg parenthesized, and indexed syntax.
Update (2026-03-18, current pass): ambiguous/missing non-authoritative object-valued default-member source-resolution diagnostics now also have direct bounded scalar-target evidence for explicit `Let` and implicit assignment across bare, zero-arg parenthesized, and indexed syntax.
Update (2026-03-18, current pass): no-parentheses getter RHS read-assignment rejection now also has direct bounded compile-time evidence across typed `Variant`, `Object`, and scalar targets under explicit `Set`, explicit `Let`, and implicit assignment for named, authoritative default-member, and single-visible-candidate non-authoritative default-member receivers.
Update (2026-03-18, closure pass): the `IP-02` checklist audit found no remaining unclassified lane in the supported native/property/default-member `DG-03` scope, so `IP-02` is now closed. Remaining late-bound default-member parity continues under `IP-03`, and wider oracle/formal program obligations continue under `IP-10` / `IP-11`.
Update (2026-03-18, current pass): bounded invoke-failure evidence now also covers stable `DISP_E_PARAMNOTFOUND` classification on the host fault surface.
Update (2026-03-18, current pass): bounded non-`IDispatch` rejection evidence now also covers stable `E_NOINTERFACE` classification for `IUnknown::QueryInterface(IDispatch)` failures on the host fault surface.
Update (2026-03-18, current pass): bounded internal invoke-conversion failures now also classify stable carrier-overflow and unsupported-`VT_BYREF` return faults on the host surface instead of leaving those deterministic lanes in the generic unspecified bucket.
Update (2026-03-18, current pass): `IP-05A` now runs from [WORKSET_2026-03-18_IP-05A_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-05A_EXECUTION_CHECKLIST.md), and the supported external early-bound member-call rewrite path now resolves member tokens from `oxvba-com` synthetic typelib metadata instead of the compiler-local hardcoded external member-token table.
Update (2026-03-18, current pass): the supported external `As New` rewrite path now also resolves deterministic `CreateObject` selectors from `oxvba-com` synthetic typelib metadata instead of the compiler-local hardcoded external selector table.
Update (2026-03-18, current pass): the supported external early-bound call rewrite path now also enforces exact argument arity from synthetic typelib metadata, so wrong-arity imported-member calls fail deterministically at compile time instead of drifting into runtime dispatch faults.
Update (2026-03-18, current pass): the supported external early-bound rewrite path now also consults imported invoke-kind metadata, so required-argument `PropertyGet` members like `Lookup` have direct compiler+host evidence while imported `PropertyPut` / `PropertyPutRef` shapes fail deterministically at compile time on `BIND-E-TYPELIB-MEMBER-SHAPE-UNSUPPORTED`.
Update (2026-03-18, current pass): the supported external early-bound rewrite path now also consumes authoritative imported default-member identity for parenthesized call syntax, so `obj(42)` lowers through the metadata-backed `EchoVariant` lane while wrong default-member arity still fails deterministically at compile time.
Update (2026-03-18, current pass): the only remaining compiler-local member-token switch is now explicitly isolated to native/internal PMR dynamic-object routing, with direct compiler evidence that imported external early-bound lowering no longer depends on that local table.
Update (2026-03-18, current pass): imported member/default-member metadata lookup now distinguishes deterministic `not found` versus `ambiguous` compile-time failures, and imported default-member call syntax no longer falls through silently when authoritative metadata does not resolve a unique target.
Update (2026-03-18, current pass): supported imported early-bound bindings now carry their authoritative typelib metadata blob inside the compiler binding/lowering path, so imported member/default-member rewrite no longer re-resolves supported types from a side-channel string lookup at each call site.
Update (2026-03-18, current pass): `IP-05A` metadata authority is now the completed floor for the supported imported subset; the remaining `IP-05` gap is the broader `IP-05B` parity matrix, richer typelib coverage, and wider Office/Excel object-model behavior rather than lingering authority ownership ambiguity.
Update (2026-03-18, current pass): the controlled imported early-bound subset now also lowers named and indexed `PropertyPut` assignment syntax through authoritative metadata, so `obj.SetValue = 9` and `obj.SetIndexedValue(7) = 11` execute end to end via deterministic `DispatchInvoke` setter lanes while imported `PropertyPutRef` assignment syntax and setter arity drift still fail deterministically at compile time in the current subset.
Update (2026-03-18, current pass): the controlled imported early-bound subset now also lowers direct zero-arg `PropertyGet` read-assignment syntax through authoritative metadata, so `x = obj.Value` and `Let x = obj.Value` execute end to end instead of remaining outside the imported parenthesized-call subset.

Use it to answer:
1. what major behavior areas are still unfinished,
2. why they are still `in-progress`,
3. which workset/spec/register owns the remaining work,
4. what must be true before the area can be described as implemented/closed.

Do not use this file for:
1. immutable historical gate records,
2. line-by-line execution logging,
3. deferred formal lane row management,
4. detailed oracle capture inventories.

Those remain in:
1. `docs/IMPLEMENTATION_LOG.md`,
2. `docs/profile-status/`,
3. `docs/evidence/formal/DEFERRED_GATES.md`,
4. `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`.

## Status vocabulary

- `in-progress`: partial implementation exists but parity for the scoped area is not complete.
- `blocked`: in-progress and currently constrained by an active blocker in `CURRENT_BLOCKERS.md`.
- `planned`: explicitly accepted area with no shipped parity slice yet.
- `closed`: scoped work area is complete for its defined target and the closure evidence is recorded.

## Feature register

| ID | Feature area | Status | Current floor | Remaining gap to close | Canonical owners |
|---|---|---|---|---|---|
| `IP-01` | Full VBA 7.1 language/runtime parity | in-progress | large executable language/runtime subset completed through historical ladders | full VBA 7.1 parity claim is still open at program level, including residual semantic, oracle, and matrix closure work | `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md` |
| `IP-02` | VBA property model and default-member semantics | closed | the native/property/default-member `DG-03` semantic model is now explicit and end to end executable across binder, compiler lowering, VM dynamic dispatch, authoritative native default-member identity, deterministic single-visible-candidate native fallback, deterministic ambiguous/missing fallback diagnostics, statement-context / `Call` / zero-arg parenthesized / indexed / no-parentheses-argument getter syntax in the supported native scope, and the complete `Set` / `Let` / implicit-assignment source-target matrix for plain scalar sources, plain `Object` sources, object-producing call results, declared-`Variant` sources with runtime payload validation, and scalar/object native property/default-member getter results; metadata-backed consumers that depend on this semantic model follow the same authoritative default-member identity contract | closed on 2026-03-18 for the scoped native/property/default-member target; remaining late-bound COM default-member recovery/parity continues under `IP-03`, and broader oracle/formal program closure remains under `IP-10` / `IP-11` | `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md`, `docs/worksets/WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md`, `docs/worksets/WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md`, `docs/worksets/WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md` |
| `IP-03` | Windows late-bound COM client (`IDispatch`) parity | in-progress | invoke transport now carries named/omitted metadata; `oxvba-com` now owns a first semantic request/callback value carrier (`ComValue`) for the recoverable subset; array/null/error intent, runtime strings, native COM-backed object arguments, native `VT_DISPATCH` results, `VT_UNKNOWN` results that expose `IDispatch`, and owned one-dimensional `VT_ARRAY | VT_VARIANT` payloads now survive into and back out of the runtime-facing COM boundary; compiler lowering now materializes both VBA `Array(...)` literals and `ParamArray` packs as semantic array payloads before the COM boundary; named method/property-get and member-known property-put/property-putref lanes execute; controlled dispatch now roundtrips `VT_NULL`, `VT_ERROR`, `VT_I2`, `VT_UI2`, `VT_R8`, BSTR strings, end-to-end `VT_DISPATCH` and `VT_UNKNOWN` object results, end-to-end object-valued COM arguments via `VT_DISPATCH`, one-dimensional `VT_ARRAY | VT_VARIANT` payloads with nested `VT_DISPATCH` elements on both argument and result paths, controlled one-dimensional typed `VT_ARRAY | VT_DISPATCH` and `VT_ARRAY | VT_UNKNOWN` payloads on the result path, and one-dimensional typed `VT_ARRAY | VT_I2`, `VT_ARRAY | VT_R8`, `VT_ARRAY | VT_BOOL`, and `VT_ARRAY | VT_BSTR` payloads; bounded `ArgErr`/`EXCEPINFO` fidelity is now preserved in the controlled native lane together with stable host classification for `DISP_E_MEMBERNOTFOUND`, `DISP_E_BADPARAMCOUNT`, `DISP_E_PARAMNOTFOUND`, runtime-string-selector `DISP_E_UNKNOWNNAME`, bounded non-`IDispatch` `E_NOINTERFACE` rejection, bounded internal carrier-overflow rejection, and bounded unsupported-`VT_BYREF` return rejection, bounded runtime-string success for zero-arg and named-argument method plus zero-arg/indexed property-get selectors via the current fallback/packing subset, metadata-backed runtime-string default-member-name dispatch, property-put/property-putref, and indexed property-put/property-putref selector execution now reuses the same bound member path as compile-time-known members, plus stable adapter-boundary classification for raw `DISP_E_UNKNOWNNAME` lookup failures; explicit metadata-backed default-member dispatch via `DispatchInvoke(obj, 0, ...)` now resolves when authoritative identity exists; natural named default-member syntax now lowers and executes on the same metadata-backed COM path when authoritative identity exists; controlled/registered lanes and deterministic member resolution subset remain in place | natural/default-member syntax for non-metadata-backed bindings, broader non-`IDispatch` interface-pointer handling, non-`IDispatch` or multi-dimensional SAFEARRAY marshalling, fuller external `ArgErr`/`ExcepInfo`/`VarResult`, and practical Office automation parity remain open | `docs/worksets/WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md`, `docs/worksets/WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md`, `docs/spec/COM_CLIENT_SERVER_SCOPE_V1.md` |
| `IP-04` | `oxvba-com` architectural repurpose and HAL COM extraction | closed | `oxvba-com` now owns the live Windows COM client bridge through `WindowsComBridge`, including activation/binding, invoke planning/execution, invoke-result rebinding/lifecycle, event subscription/callback transport, and typelib/runtime metadata services; `oxvba-hal` now retains only capability/policy gating, apartment/bootstrap hooks, selector fallback, delegation, and boundary error mapping over that bridge | closed on 2026-03-14 for the architectural ownership target; downstream COM parity work remains in `IP-03`, `IP-05`, `IP-06`, and `IP-08` | `docs/worksets/WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`, `docs/worksets/WORKSET_2026-03-14_IP04_OXVBA_COM_HAL_EXTRACTION_CLOSURE.md`, `docs/ARCHITECTURE.md` |
| `IP-05` | Windows early-bound COM and type-library parity | in-progress | metadata-authority-backed early-binding subset is now in place for the supported imported scope: `oxvba-com` owns authoritative imported member/default-member and activation metadata; compiler lowering binds supported imported declarations through that metadata path; supported imported bindings now carry their authoritative metadata blob through lowering; executable conformance lanes exist for the current imported method/property-get/default-member call subset plus controlled zero-arg `PropertyGet` read-assignment syntax and controlled named/indexed `PropertyPut` assignment syntax; and deterministic compile-time diagnostics cover qualifier, member identity, shape, and arity failures in that metadata-backed subset, including deterministic rejection of still-unsupported imported `PropertyPutRef` assignment syntax | remaining `IP-05B` work is the broader early-bound parity matrix: richer typelib/member coverage, broader imported member/property/event/default-member lowering, wider Office/Excel object-model behavior, object-valued imported setter parity, and final parity closure beyond the current supported imported subset | `docs/spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md`, `docs/worksets/WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md`, `docs/worksets/WORKSET_2026-03-18_IP-05A_EXECUTION_CHECKLIST.md`, `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md` |
| `IP-06` | Windows COM server/export parity | planned | COM client/event groundwork exists; scope tiering is defined (`S0..S3`) | native COM server behavior is still below parity target; class exposure, typelib publication, outward `IDispatch` parity, and host policy model remain to be implemented | `docs/spec/COM_CLIENT_SERVER_SCOPE_V1.md`, `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md` |
| `IP-07` | Event runtime parity (non-COM + COM adapter lanes) | in-progress | baseline event runtime pass completed; runtime owner iteration and major non-COM/event infrastructure are executable | full `WithEvents` instance graph semantics, unified host ingress parity, and complete COM event parity lanes remain open | `docs/worksets/WORKSET_2026-03-08_EVENTS_PARITY_CLOSURE.md`, `CURRENT_BLOCKERS.md` |
| `IP-08` | Host project / Office-style hosting parity | planned | host bridge and object/event ingress contracts are now locked at design level | full Host Project semantics, Office-style root/global exposure, callback-path hosting parity, and integration with final COM/property model remain open | `docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md`, `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md` |
| `IP-09` | Declare/native marshaling parity | in-progress | deterministic declaration subset, descriptor routing, and bounded host-backed lanes are implemented | full Automation legality matrix, pointer-string lanes, byref writeback, richer native ABI shapes, and `IDispatch::Invoke` output obligations remain open | `docs/spec/HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md`, `docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md` |
| `IP-10` | Oracle/differential parity closure for required behavior areas | in-progress | deferred-oracle structure and topic tracking are in place; some targeted probes have been captured | required Office/host differential captures are not yet exhausted for open parity areas, so claim closure cannot rely only on local subset evidence | `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`, `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md` |
| `IP-11` | Formal foldback for active parity claims | in-progress | formal infrastructure and many obligations exist; policy for non-blocking deferred lanes is defined | open deferred gates and failed/deferred formal lanes still require foldback or bounded resolution before full parity claims can close | `docs/evidence/formal/DEFERRED_GATES.md`, `docs/FORMAL.md` |

## Area notes

### `IP-01` Full VBA 7.1 language/runtime parity

Why still open:
1. the repo has many completed historical ladders, but the current governing claim is the full compliance program,
2. that program explicitly requires zero unresolved in-scope divergences, no open in-scope deferred gates, and a green Office differential matrix,
3. those terminal conditions are not met yet.

### `IP-02` VBA property model and default-member semantics

Closure summary:
1. the DG-03 native/property/default-member semantic model is now fully classified in [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md),
2. no live IP-02 semantic blocker remains in [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md),
3. remaining late-bound default-member recovery and non-metadata-backed COM behavior are owned by IP-03, not by IP-02,
4. remaining oracle and formal program gates stay under IP-10 / IP-11 and do not keep the scoped IP-02 native closure target open.

### `IP-03` Windows late-bound COM client parity

Why still open:
1. the current lane is stronger than before but remains a subset,
2. the scope doc still limits current maturity to C2 runway/subset behavior,
3. the dedicated `IDispatch` completion workset exists because the parity gap is real and specific.

### `IP-04` `oxvba-com` repurpose and HAL COM extraction

Closure summary:
1. `oxvba-com` now owns the live Windows COM client facade through `WindowsComBridge`.
2. `standard.rs` now delegates activation, invoke, object description/release, event subscription/callback interrogation, and typelib services through that facade.
3. The remaining HAL COM code is limited to capability/policy gating, apartment/bootstrap hooks, selector fallback, and error mapping.
4. `CURRENT_BLOCKERS.md` no longer carries `BLK-COM-BOUNDARY-001` as an active blocker.
5. Remaining COM behavior/parity work continues under `IP-03`, `IP-05`, `IP-06`, and `IP-08`; `IP-04` itself is closed.

### `IP-05` Windows early-bound COM and type-library parity

Why still open:
1. the current early-binding implementation is explicitly a constrained subset,
2. broader Office parity was intentionally left out of the earlier tranche,
3. the doctrine now requires this to remain `in-progress` until the real parity target is closed.

### `IP-06` Windows COM server/export parity

Why still open:
1. the scope doc still identifies server behavior as below parity target,
2. current COM progress has primarily been on client/event and shared transport foundations,
3. the server side remains a major unfinished domain.

### `IP-07` Event runtime parity

Why still open:
1. the event workset itself says the baseline pass is complete but residual parity work remains,
2. host-event ingress, sink-instance graph parity, and COM event tiers are still open,
3. these behaviors must converge before an events parity claim is valid.

### `IP-08` Host project / Office-style hosting parity

Why still open:
1. the bridge contracts are better defined now,
2. but the actual Office-style hosting model is not yet parity-complete,
3. this area depends on property semantics, object/value identity, and COM boundary completion.

### `IP-09` Declare/native marshaling parity

Why still open:
1. many HAL dynamic-link clauses remain `implemented-partial`,
2. the current supported subset is deliberately narrow,
3. the docs explicitly call out remaining Automation/native ABI work.

### `IP-10` Oracle/differential parity closure

Why still open:
1. this is not a feature by itself, but it is required for parity closure of multiple features,
2. several implementation-defined or deferred-oracle topics remain open,
3. without oracle foldback the repo cannot honestly claim full VBA/Excel parity in those areas.

### `IP-11` Formal foldback for active parity claims

Why still open:
1. many formal lanes are historical and folded,
2. but open deferred/failing lanes still exist in the live register,
3. the full-compliance claim model requires these to be folded or explicitly bounded for in-scope parity claims.

## Operating rules

When any feature area above changes:
1. update this file,
2. update the owning workset/spec/register,
3. keep the status as `in-progress` until the scoped parity target is actually complete,
4. only remove an entry when its scope is truly parity-complete or when the scope is explicitly retired/replaced.

## Update checklist

1. Is the area still part of the active parity target?
2. Is there still any open blocker, deferred gate, oracle gap, or unimplemented parity behavior in scope?
3. If yes, keep the entry `in-progress`.
4. If no, update the owning docs first, then remove or mark the entry complete through an explicit documented decision.











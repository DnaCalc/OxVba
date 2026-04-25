# Workset: COM Parity, Property Semantics, Server Export, and Hosting Sequence

Date: 2026-03-14  
Status: planned  
Scope: define the dependency-ordered execution program for the remaining COM- and hosting-related work after `IP-04` closure, covering `IP-02`, `IP-03`, `IP-05`, `IP-06`, and `IP-08`, with cross-cutting notes for `IP-07` where event/runtime ingress is a prerequisite.

## 1. Purpose

This workset is the large-program execution map for the next COM/hosting phase of the repo.

`IP-04` closed the architectural ownership question:
1. `oxvba-com` is now the live Windows COM bridge.
2. `oxvba-hal` is no longer the substantive home of COM execution logic.

What remains is behavioral and product-parity work:
1. complete late-bound COM parity,
2. complete the missing property/default-member semantics that cut across COM and hosting,
3. complete the synthetic reference facade and early-bound COM story,
4. complete COM server/export work,
5. complete Office-style host/project behavior.

This workset exists to answer:
1. what order those areas should be executed in,
2. what each area depends on,
3. what can be done in parallel and what should not,
4. what completion means for each area,
5. how to avoid re-opening the already-closed COM/HAL boundary work.

## 2. Covered areas

Primary in-scope feature areas:
1. `IP-02` VBA property model and default-member semantics.
2. `IP-03` Windows late-bound COM client (`IDispatch`) parity.
3. `IP-05` Windows early-bound COM and type-library parity.
4. `IP-06` Windows COM server/export parity.
5. `IP-08` Host project / Office-style hosting parity.

Cross-cutting dependency area:
1. `IP-07` event runtime parity.

`IP-07` is not the main subject of this workset, but parts of it must advance where host ingress, callback behavior, or `WithEvents`/event surface parity block `IP-06` or `IP-08`.

## 3. High-level dependency order

The dependency spine is:
1. `IP-03` foundation work.
2. `IP-02` semantic closure slices that depend on the late-bound transport and dynamic-object substrate.
3. `IP-05` reference facade / early-bound completion on the same authoritative metadata source.
4. `IP-03` broad parity expansion after the property/reference substrate is stronger.
5. `IP-06` COM server/export substrate and outward automation surface.
6. `IP-07` event/ingress closure where it blocks host/server behavior.
7. `IP-08` Office-style host/project closure on top of the finished object/property/COM/event model.

Short version:
1. fix the runtime semantics and transport first,
2. then fix the compiler-visible COM model,
3. then finish late-bound and early-bound parity,
4. then expose OxVba outward as COM,
5. then finish the Office-style hosting model.

Current execution note (2026-03-18):
1. step 2 is now complete for the scoped native/property/default-member `DG-03` target via [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md),
2. the remaining program still runs in the same order for `IP-03`, `IP-05`, `IP-06`, `IP-07`, and `IP-08`.

Current value-model migration note (2026-04-25):
1. HAL trait surfaces now classify `RuntimeValue` methods as compatibility projection contracts and `_variant` companions as retained value-model entry points for VM/JIT callers.
2. Legacy `SafeArray` `RuntimeValue` constructors/accessors now classify themselves as compatibility projections and point new value-model call sites at retained `Variant` APIs.
3. Host project-runtime, immediate-session, and embedded invocation `RuntimeValue` snapshots/requests/results now classify themselves as compatibility projections and point retained-value callers at `Variant` APIs.
4. COM model `RuntimeValue`/legacy-token helpers now classify themselves as compatibility projections around retained `Variant` invoke/callback payloads.
5. Windows COM bridge/invoke `RuntimeValue` result and callback-argument APIs now classify themselves as compatibility projections beside retained `Variant`/`ComValue` transport.
6. Dynamic COM value and portable dispatch surfaces now classify `RuntimeValue` entry points as compatibility projections around retained `Variant`/`ComValue` carriers.
7. VM legacy scalar helper writes now materialize compatibility-tagged `Variant` slots directly instead of routing through a temporary `RuntimeValue`.
8. Runtime `Variant`/`RuntimeValue` bridge helpers now classify the retained `Variant` carrier as primary and the `RuntimeValue`/i32 slot-token routes as compatibility projections.
9. JIT/Cranelift `RuntimeValue` execution and slot helper APIs now classify themselves as compatibility projections over retained `Variant` execution APIs.
10. VM `RuntimeSlot` and JIT `RtSlot` `RuntimeValue`/i32 conversion helpers now classify themselves as compatibility ingress/egress projections around retained `Variant` slot carriers.
11. Runtime pointer-helper `RuntimeValue` registration/readback APIs now classify themselves as compatibility projections beside retained `Variant` pointer-helper APIs.
12. HAL standard process legacy `RuntimeValue` methods now classify themselves as compatibility wrappers beside retained `Variant` process APIs.
13. VM shared semantic helpers and JIT runtime helper bridges now classify `RuntimeValue` helper contracts as compatibility layers over retained `Variant` slot storage.
14. HAL dynamic-link legacy `RuntimeValue` trait methods/default adapters and standard adapter hooks now classify themselves as compatibility layers beside retained `Variant` invoke paths.
15. HAL diagnostics, UI, event-pump, and time legacy `RuntimeValue` methods now classify themselves as compatibility wrappers beside retained `Variant` companion methods.
16. HAL filesystem legacy `RuntimeValue` methods now classify themselves as compatibility wrappers beside retained `Variant` filesystem companion methods.
17. HAL console legacy `RuntimeValue` methods now classify themselves as compatibility wrappers beside retained `Variant` console companion methods.
18. HAL COM activation/dispatch/event legacy `RuntimeValue` methods now classify themselves as compatibility result projections beside retained `Variant` COM companion methods.
19. Host debugger `RuntimeValue` frame/evaluation APIs now classify themselves as compatibility projections from retained `Variant` frame reads.
20. Non-standard HAL null/WASM/replay adapter `RuntimeValue` methods now classify themselves as compatibility wrappers beside retained `Variant` companion methods.
21. VM/JIT string slice intrinsics `Len`, `Left`, `Right`, and `Mid` now read retained `Variant` slots directly through Variant-native text/count coercion helpers instead of projecting through `RuntimeValue` first.
22. VM/JIT text transform/search intrinsics `InStr`, `InStrRev`, `LCase`, `UCase`, `Replace`, `Trim`, `LTrim`, `RTrim`, `StrComp`, and `StrReverse` now read retained `Variant` slots directly through Variant-native text coercion helpers.
23. VM/JIT char/format-adjacent intrinsics `Chr`, `Asc`, `Space`, `String$`, `Hex`, `Oct`, and `MonthName` now read retained `Variant` slots directly through Variant-native coercion helpers and write retained `Variant` results directly.
24. VM/JIT `Like` and `StrConv` now read retained `Variant` slots directly through Variant-native text/conversion coercion helpers and write retained `Variant` results directly.
25. VM/JIT `Format` now reads retained `Variant` value/format slots directly through Variant-native numeric/text coercion helpers and writes retained `Variant` string results directly.
26. VM/JIT date/time intrinsics `DateSerial`, `TimeSerial`, `DateValue`, `TimeValue`, `DateAdd`, `DateDiff`, `Year`, `Month`, `Day`, `Weekday`, and the JIT `CDate` helper now read retained `Variant` slots directly through Variant-native date/time coercion helpers and write retained `Variant` results directly.
27. VM/JIT math intrinsics `Abs`, `Sgn`, `Round`, `Sqr`, `Sin`, `Cos`, `Log`, `Exp`, `Atn`, and `Tan` now read retained `Variant` slots directly through Variant-native numeric coercion helpers and write retained `Variant` results directly.
28. VM-only conversion intrinsics `CStr`, `Str$`, `Val`, and `CDateValue` now read retained `Variant` slots directly through Variant-native conversion helpers and write retained `Variant` results directly.
29. VM/JIT aggregate string intrinsics `Mid` statement, `Split`, and `Join` now read retained `Variant` slots directly through Variant-native string/array helpers and write retained `Variant` results directly.
30. VM/JIT core arithmetic operators `Add`, `Sub`, `Mul`, `Div`, `IntDiv`, `Mod`, `Pow`, `Concat`, `Neg`, `AddConst`, `SubConst`, and `Inc` now read retained `Variant` slots directly through Variant-native arithmetic helpers and write retained `Variant` results directly.
31. VM/JIT comparison and Boolean operators now read retained `Variant` slots directly through Variant-native comparison/truthiness helpers and write retained `Variant` Boolean results directly.
32. VM/JIT `Rnd` and `Randomize` seed operands now read retained `Variant` slots directly through Variant-native numeric seed coercion while preserving retained `Variant` result writes.
33. VM/JIT dynamic array `ReDim`, `ReDim Preserve`, array get, and array set bound/index operands now read retained `Variant` slots directly through Variant-native numeric coercion while preserving retained SAFEARRAY-backed `Variant` array carriers.
34. VM/JIT scalar/control/binding helper operands now read retained `Variant` slots directly for `Int`/`Fix`, conditional jumps, runtime assignment validation, finance/collection numeric lanes, and WithEvents binding-token lanes.
35. Trait-level HAL default projection companions and legacy `SafeArray`/host/COM/JIT compatibility APIs remain migration/classification work and do not close the `IP-03` `VARIANT`/SAFEARRAY foundation area.

## 4. Why this order is correct

### 4.1 Why `IP-03` foundation comes first

`IP-03` still blocks the most basic COM semantic fidelity:
1. broad `VARIANT` payload support,
2. object/interface results and arguments,
3. SAFEARRAY breadth,
4. realistic `IDispatch::Invoke` error/result behavior.

Without this:
1. `IP-02` property semantics will stabilize on an incomplete COM call model,
2. `IP-05` early-bound lowering will still target an incomplete runtime boundary,
3. `IP-06` outward automation implementation would risk using the wrong semantic shapes,
4. `IP-08` host behavior would rest on incomplete object/property behavior.

### 4.2 Why `IP-02` must be advanced before declaring broad COM or hosting closure

The property/default-member model is one of the highest-leverage shared semantics:
1. `Property Get/Let/Set`,
2. `Set` vs `Let`,
3. default-member source of truth,
4. call-vs-value context,
5. indexed/default-property behavior.

These affect:
1. late-bound COM calls,
2. early-bound COM lowering,
3. COM server/export outward behavior,
4. Office-style hosting.

If this stays partial, every other area remains partially semantic.

### 4.3 Why `IP-05` should complete before `IP-06`

COM server/export is not just wire plumbing.
It needs:
1. a coherent object/member model,
2. outward property/default-member behavior,
3. metadata/type publication discipline,
4. confidence that the compiler/runtime understand the same member kinds the server side will expose.

That means the reference facade and early-bound model should be stronger first.

### 4.4 Why `IP-08` is last

Office-style hosting is the most integrative area.
It depends on:
1. object identity,
2. property/default-member semantics,
3. callback/event ingress behavior,
4. host bridge semantics,
5. COM consumption and possibly COM exposure.

Trying to close `IP-08` before the others would force design churn into the host model.

## 5. Execution model

The recommended execution model is staged, not perfectly serial.

Use this rule:
1. foundation-first,
2. semantics second,
3. outward exposure third,
4. host/product behavior last.

Allowed overlap:
1. `IP-05` metadata/reference-facade work can start once the `IP-03` value/transport foundation is stable enough.
2. `IP-07` targeted ingress/event work can proceed whenever it is needed to unblock `IP-06` or `IP-08`.
3. documentation/spec work should be folded back continuously, not left to the end.

Do not overlap aggressively:
1. `IP-06` should not harden before `IP-02` semantics are sufficiently closed.
2. `IP-08` should not become the place where unresolved COM/property semantics are “worked around.”

## 6. Program phases

### Phase A. Late-bound COM value and invoke substrate (`IP-03A`)

Primary target:
1. complete the remaining transport and `Invoke` substrate required for real late-bound COM work.

Why first:
1. this is the shared runtime boundary for the rest of the COM program.

Deliverables:
1. broaden the canonical external-call value carrier for COM-facing work:
   - more scalar categories,
   - BSTR/string coverage where still incomplete,
   - object/interface categories,
   - broader SAFEARRAY ranks/element types where supported,
   - explicit unsupported-path diagnostics where not yet supported.
2. complete `IDispatch::Invoke` request/result fidelity for the next honest parity tier:
   - named args,
   - omitted args,
   - `ArgErr`,
   - bounded `ExcepInfo`,
   - clearer `VarResult` handling.
3. remove remaining lossy fallback assumptions in late-bound COM execution where they still distort supported cases.
4. expand controlled fixture coverage so every newly supported payload/invoke shape is proven.

Key files/worksets:
1. [WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md)
2. [WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md)

Acceptance:
1. the supported late-bound COM matrix no longer depends on narrow legacy token projections for supported cases,
2. current supported COM value shapes move through `oxvba-com` semantically and deterministically,
3. fixture coverage exists for every newly supported call/value class.

Planning notes:
1. this phase is still not “full `IP-03` closure.”
2. it is the last broad transport/substrate phase before higher-level semantic closure.

### Phase B. Property/default-member semantic closure foundation (`IP-02A`)

Status:
1. completed on 2026-03-18 for the scoped native/property/default-member `DG-03` target tracked in [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md).
2. remaining late-bound default-member recovery/parity is downstream `IP-03` work, not residual `IP-02` work.

Primary target:
1. lock the end-to-end semantics for `Property Get/Let/Set`, default members, and call-vs-value context.

Why here:
1. after the transport substrate is credible, the next highest-risk ambiguity is semantic intent.

Deliverables:
1. define one authoritative property/default-member semantic model across:
   - binder,
   - compiler lowering,
   - VM dynamic dispatch,
   - COM late-bound bridge,
   - early-bound metadata-backed calls,
   - future COM server/export behavior.
2. make `Set` vs `Let` intent explicit in the runtime-facing call model.
3. make default-member identity authoritative from one source of truth for:
   - native project objects,
   - COM metadata-backed objects,
   - future outward COM exposure.
4. complete indexed/default-property call-vs-value behavior in the supported scope.
5. add end-to-end tests that show the same property semantics across native object and COM-backed object paths where applicable.

Key dependencies:
1. stable dynamic-object protocol,
2. sufficient late-bound invoke transport fidelity from Phase A.

Acceptance:
1. property/default-member behavior is no longer described as partial/shared debt in `CURRENT_BLOCKERS.md`,
2. `IP-02` has a coherent end-to-end model even if some downstream parity areas still consume it incrementally.
3. this acceptance was satisfied on 2026-03-18 for the scoped native/property/default-member target.

Planning notes:
1. this phase likely resolves the biggest remaining semantic ambiguity shared by `IP-03`, `IP-05`, `IP-06`, and `IP-08`.
2. do not let Office-host-specific behavior become the first implementation site for unresolved property semantics.

### Phase C. COM reference facade and metadata authority (`IP-05A`)

Status:
1. completed on 2026-03-18 for the metadata-authority target tracked in [WORKSET_2026-03-18_IP-05A_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-05A_EXECUTION_CHECKLIST.md).

Primary target:
1. make COM type-library imports behave like real referenced-library metadata to the compiler.

Why after Phase B:
1. the compiler-visible model should attach to the same authoritative member/property/default-member semantics.

Deliverables:
1. complete the synthetic COM reference facade in `oxvba-com`.
2. make binder/typecheck/lowering consume that facade as the authoritative imported-library model.
3. reduce hardcoded member-token assumptions further in favor of metadata-driven lowering.
4. carry invoke kind, default-member markers, optional/named parameter data, and event metadata through the facade.
5. align early-bound lowering and metadata-backed late-bound improvements to the same source.

Key files/worksets:
1. [WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md)
2. [COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md)
3. [COM_REFERENCE_FACADE_AND_DYNAMIC_OBJECT_PROTOCOL_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_REFERENCE_FACADE_AND_DYNAMIC_OBJECT_PROTOCOL_V1.md)

Acceptance:
1. COM imports no longer behave like an ad hoc side-domain,
2. early-bound and metadata-backed late-bound behavior share the same member metadata authority.

Planning notes:
1. this is the compiler-facing complement to the now-closed `IP-04` runtime ownership work.
2. expect this phase to flush out more exact VBA/Excel precedence and ambiguity rules.
3. broader early-bound member/property/event/default-member parity remains downstream `IP-05B` work; it is not residual `IP-05A` authority ambiguity anymore.

### Phase D. Broad late-bound COM parity closure (`IP-03B`)

Primary target:
1. close `IP-03` on the correct substrate.

Why after Phases A-C:
1. by this point the transport, property semantics, and metadata authority should all be strong enough to support a real parity push.

Deliverables:
1. close remaining late-bound gaps for the scoped Windows client surface:
   - natural default-member syntax where identity can be recovered or bounded honestly,
   - broader object/interface result handling,
   - broader SAFEARRAY and automation payload support,
   - broader external server behavior coverage,
   - stronger `Err`/failure fidelity.
2. extend evidence beyond controlled fixtures where practical:
   - deterministic registered servers,
   - Office-relevant lanes where defensible and reproducible.
3. update scope docs so they state the supported parity matrix precisely.

Acceptance:
1. `IP-03` can be closed honestly for the scoped Windows late-bound client target,
2. blockers for late-bound COM parity move out of `CURRENT_BLOCKERS.md` or are narrowed to clearly out-of-scope future items.

Planning notes:
1. this is where the repo should start using the word `implemented` for late-bound COM only if the scoped parity target is truly complete.
2. if Excel/VBA and spec behavior diverge, capture the divergence explicitly rather than softening the claim language.

### Phase E. Early-bound COM parity closure (`IP-05B`)

Primary target:
1. close the scoped early-bound/type-library parity area.

Why after Phase D:
1. broad late-bound and property behavior provides runtime confidence for the early-bound lowering target.

Deliverables:
1. complete the early-bound member/property/event/default-member lowering matrix for the scoped target.
2. expand typelib coverage where the current early-bound subset is intentionally narrow.
3. ensure compile-time diagnostics and runtime behavior align on the same imported metadata model.
4. close the scoped early-bound conformance/evidence surfaces.

Acceptance:
1. `IP-05` can be closed honestly for its defined scope,
2. the repo no longer describes early-bound COM as merely a rewrite-oriented tranche.

Planning notes:
1. this phase should be allowed to share implementation slices with late-bound work when the authoritative metadata model overlaps.
2. do not duplicate metadata rules between early and late paths.

### Phase F. COM server/export substrate (`IP-06A`)

Primary target:
1. build the outward COM automation substrate on top of the now-stable object/property/metadata model.

Why here:
1. outward COM publication depends on the same member/property/default-member semantics and metadata discipline already being stable.

Deliverables:
1. define the outward server/export policy model:
   - what OxVba objects/classes can be exposed,
   - how registration/activation works,
   - what typelibs are published,
   - what hosting policy gates apply.
2. implement the first real outward automation bridge in `oxvba-com`.
3. expose OxVba objects as COM automation objects with correct outward member/property behavior for the scoped surface.
4. add deterministic server/export fixtures and roundtrip tests.

Acceptance:
1. there is a real COM server/export substrate, not just a planned scope document.
2. the outward automation surface reflects the same semantics as the inward client model where intended.

Planning notes:
1. start with the smallest honest server tier that exercises object exposure, method/property dispatch, and typelib publication.
2. do not overreach into full Office add-in/application integration in the first server slice.

### Phase G. Event/runtime ingress closure needed for host/server behavior (`IP-07 dependency slice`)

Primary target:
1. finish the event/runtime ingress pieces required by `IP-06` and `IP-08`.

Why here:
1. host parity and outward COM exposure both depend on event/callback behavior being coherent.

Deliverables:
1. finish unified host event ingress behavior where still inconsistent.
2. close remaining COM callback/event transport residuals that affect host or outward object behavior.
3. tighten `WithEvents`/instance graph/event ownership behavior where it blocks Office-style hosting.

Acceptance:
1. `IP-07` may or may not close fully here,
2. but the pieces that block `IP-06` and `IP-08` must be complete.

Planning notes:
1. treat this as a dependency-clearing phase, not a separate detour.
2. if full `IP-07` closure becomes reachable naturally, close it; otherwise document the exact remaining non-blocking event residuals.

### Phase H. Host project / Office-style hosting foundation (`IP-08A`)

Status:
1. completed on 2026-03-19 for the bounded host-foundation target tracked in [WORKSET_2026-03-19_IP-08A_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-19_IP-08A_EXECUTION_CHECKLIST.md).
2. remaining host work is now `IP-08B` parity breadth, not missing foundation semantics.

Primary target:
1. turn the host/project model from design contract into executable host behavior.

Why after the COM/property/server phases:
1. the host should consume a stable runtime object/property/event model, not define it by accident.

Deliverables:
1. complete the host project model:
   - application/root objects,
   - global exposure rules,
   - project object behavior,
   - runtime session lifecycle,
   - object identity and callback routing.
2. align host object behavior with the now-stable property/default-member semantics.
3. make COM-backed host interactions and native host objects coexist on the same object/value/event model where intended.
4. advance the host bridge/tooling proposal into implementation-backed behavior.

Acceptance:
1. the repo has a real Office-style hosting substrate rather than only a proposal.
2. host behavior is not carrying unresolved semantics that belong in lower layers.

Planning notes:
1. this is the point where future DnaVbCalc and related host shells benefit directly.
2. keep repo boundaries clear: host model work here is OxVba-side hosting semantics, not the separate future DnaVbCalc repo itself.

### Phase I. Office-style hosting parity closure (`IP-08B`)

Status:
1. in progress from 2026-03-19 via [WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md).

Primary target:
1. close the scoped host/project parity area.

Deliverables:
1. complete the scoped host/root/global behavior matrix.
2. close callback-path hosting parity for the scoped model.
3. align documentation, evidence, and host behavior claims.
4. close any final semantic gaps shared with `IP-02`, `IP-06`, or `IP-07`.

Acceptance:
1. `IP-08` can be closed honestly for its scoped Office-style hosting target.

Planning notes:
1. this is where UI/shell/pathfinder hosts can rely on stable behavior rather than exploratory contracts.
2. do not close this area until the host behavior is evidenced, not just designed.

## 7. Detailed dependency matrix

### `IP-02` depends on
1. enough of `IP-03A` to carry property/default-member intent through realistic COM calls,
2. the shared dynamic-object protocol already in place,
3. stable object/value identity rules.

### `IP-05` depends on
1. `IP-02A` property/default-member source-of-truth work,
2. stable `oxvba-com` metadata/reference facade ownership,
3. enough of `IP-03A` that runtime execution of lowered early-bound forms is not targeting a known-broken invoke substrate.

### `IP-03B` depends on
1. `IP-03A` transport completion,
2. `IP-02A` for semantic call/property/default-member intent,
3. `IP-05A` where metadata-backed default-member/member-kind behavior shares the same source.

### `IP-06` depends on
1. `IP-02` property/default-member semantics,
2. `IP-05` metadata/type publication discipline,
3. enough of `IP-07` that callback/event behavior is coherent,
4. stable object/value identity and late-bound dispatch semantics.

### `IP-08` depends on
1. `IP-02` semantic closure,
2. enough of `IP-03` and `IP-05` that host object interactions are stable,
3. enough of `IP-07` that event ingress/`WithEvents` behavior is dependable,
4. enough of `IP-06` if the chosen host parity surface requires outward automation behavior.

## 8. Recommended execution sequence

Recommended concrete order:
1. `IP-03A` late-bound COM value and invoke substrate.
2. `IP-02A` property/default-member semantic closure foundation.
3. `IP-05A` reference facade and metadata authority.
4. `IP-03B` late-bound COM parity closure.
5. `IP-05B` early-bound COM parity closure.
6. `IP-06A` COM server/export substrate.
7. `IP-07` dependency-clearing event ingress slices.
8. `IP-08A` host project / Office-style hosting foundation.
9. `IP-06B` COM server/export parity closure if not already reached in `A`.
10. `IP-08B` Office-style hosting parity closure.

Why this specific order:
1. it clears runtime semantics before compiler-facing closure,
2. clears compiler-facing closure before outward server/export,
3. clears both before the host/project layer tries to depend on them.

## 9. Suggested work package split

To keep the program executable, split the remaining work into these packages:
1. `PKG-COM-LATEBOUND-SUBSTRATE`
2. `PKG-PROPERTY-DEFAULTMEMBER-SEMANTICS`
3. `PKG-COM-REFERENCE-FACADE`
4. `PKG-COM-LATEBOUND-PARITY`
5. `PKG-COM-EARLYBOUND-PARITY`
6. `PKG-COM-SERVER-EXPORT-SUBSTRATE`
7. `PKG-EVENT-INGRESS-HOST-BLOCKERS`
8. `PKG-HOST-PROJECT-FOUNDATION`
9. `PKG-COM-SERVER-EXPORT-PARITY`
10. `PKG-HOSTING-PARITY-CLOSEOUT`

Each package should have:
1. exact scope,
2. explicit non-goals,
3. green verification matrix,
4. closure evidence,
5. doctrine-compliant status language.

## 10. Planning notes by area

### `IP-03` planning notes
1. keep all new COM wire behavior inside `oxvba-com`.
2. continue using OxVba semantic values as the runtime-facing value model.
3. do not let fixture convenience define the final Office/VBA parity claim.

### `IP-02` planning notes
1. default-member behavior must have one authority.
2. avoid separate native-object and COM-object property semantics.
3. this area should likely get its own dedicated closure workset if it grows beyond the current shared worksets.

### `IP-05` planning notes
1. the compiler-visible COM facade is not optional polish; it is the correct compile-time destination.
2. avoid re-encoding metadata logic in the compiler if `oxvba-com` can project it canonically.

### `IP-06` planning notes
1. keep server/export tiers explicit.
2. ship the smallest honest outward automation slice first.
3. do not claim “COM server support” until the outward automation surface is actually evidenced.

### `IP-08` planning notes
1. the host should consume stable semantics, not define them.
2. future pathfinder/UI hosts benefit from this area, but this repo’s scope is still OxVba hosting semantics first.
3. the separate DnaVbCalc repo should consume this work, not be mixed into this workset.

## 11. Verification expectations

For each package/phase, run the normal focused matrix plus the relevant targeted suites.

At program level, keep these green where touched:
1. `cargo test -p oxvba-com -p oxvba-hal -p oxvba-host -p oxvba-vm -p oxvba-compiler --quiet`
2. focused end-to-end COM client/server/early-bound/host tests as applicable,
3. `./scripts/check-governance.ps1`
4. `./scripts/meta-check.ps1 -Fast -NoArtifacts`

For closure claims in these areas, additionally require:
1. scope docs updated,
2. worklist/blocker surfaces updated,
3. evidence that the scoped parity target is complete,
4. no use of `implemented`/`closed` until that evidence exists.

## 12. Exit condition for this workset

This orchestration workset is successful when:
1. the repo has one accepted dependency-ordered execution sequence for the remaining COM/property/hosting program,
2. that sequence is specific enough to drive future workset closure decisions,
3. the order makes clear which areas are foundational and which are consumers,
4. downstream work can proceed without reopening the already-closed `IP-04` architecture work.

## 13. Related documents

1. [WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md)
2. [WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md)
3. [COM_REFERENCE_FACADE_AND_DYNAMIC_OBJECT_PROTOCOL_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_REFERENCE_FACADE_AND_DYNAMIC_OBJECT_PROTOCOL_V1.md)
4. [COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md)
5. [COM_CLIENT_SERVER_SCOPE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_CLIENT_SERVER_SCOPE_V1.md)
6. [HOSTING_PROJECT_TOOLING_PROPOSAL.md](C:\Work\DnaCalc\OxVba\docs\spec\HOSTING_PROJECT_TOOLING_PROPOSAL.md)
7. [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
8. [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)

# Workset: VBA 7.1 + Windows Office COM Full Compliance Program

Date: 2026-03-08  
Status: planned  
Scope: achieve and evidence a strict "100% VBA 7.1 behavior + 100% COM behavior" parity claim for Windows-hosted OxVBA under Office-compatible hosting assumptions.

## 1. Program intent

This workset defines the endgame program that closes:
1. Full VBA 7.1 language/runtime semantic parity (as specified + observed in Office),
2. Full COM interoperability parity on Windows (client + server + events + typelib semantics),
3. Full host-integration parity expected in Office-style hosting (including Host Project layering, default members, Set/Let behavior, and COM property semantics).

## 2. Claim model ("100%" compliance standard)

For this program, a "100%" claim is allowed only when all of the following are true:
1. Clause coverage is complete for in-scope normative sources (no unclassified clauses).
2. No open high/medium-severity behavioral divergences remain in in-scope lanes.
3. Deferred gates are empty for in-scope lanes, or explicitly re-scoped by approved claim text.
4. Office differential matrix for required scenarios is green across required versions/bitness.
5. Formal obligation set is complete for designated safety/correctness invariants and is passing (or explicitly bounded where proof tooling cannot model external platform behavior).

## 3. Normative scope baseline

Primary normative anchors:
1. MS-VBAL (VBA language semantics),
2. MS-OAUT (Automation/Variant/IDispatch/typelib semantics),
3. MS-COM and related Windows COM contracts,
4. VBA 7.1 behavior under Office on Windows (black-box oracle captures only; clean-room compliant).

Target environment for parity claim:
1. Windows host profile,
2. Office-compatible hosting model with Host Project injection,
3. 32-bit and 64-bit process lanes.

## 4. Required closure domains

### D1. Core VBA language/runtime completeness
1. Full statement/expression semantics in declared scope,
2. Full coercion/variant/null/empty/error semantics and edge behavior,
3. Full `On Error` and error state lifecycle parity,
4. Full procedure and parameter model parity (optional/named/byref/byval/paramarray),
5. Full object/reference semantics (default instances, lifetime, termination behavior).

### D2. Property model completeness (VBA + COM)
1. `Property Get/Let/Set` behavior parity in compiler/runtime,
2. `Set` vs `Let` semantic distinction in assignments and calls,
3. Default member resolution parity (`VB_UserMemId = 0`, `DISPID_VALUE` paths),
4. Indexed/default property semantics and call-vs-value context parity.

### D3. COM client completeness (Windows)
1. `CreateObject`/binding parity (progid/clsid/versioned behavior),
2. IDispatch invoke parity:
   - `DISPATCH_METHOD`,
   - `DISPATCH_PROPERTYGET`,
   - `DISPATCH_PROPERTYPUT`,
   - `DISPATCH_PROPERTYPUTREF`,
   - named-arg behavior (`DISPID_PROPERTYPUT`) and LCID handling.
3. Dual-interface/vtable behavior parity and explicit strategy policy,
4. Variant/SAFEARRAY/BSTR marshaling parity including byref/out/inout behavior.

### D4. COM server completeness (Windows)
1. Office-compatible class exposure semantics,
2. Type info/typelib consistency and registration model,
3. `IDispatch::GetIDsOfNames` and `Invoke` behavior parity,
4. Error/HRESULT/EXCEPINFO mapping parity,
5. Reference counting/lifecycle and deterministic termination semantics.

### D5. Event system completeness
1. Non-COM `WithEvents`/`RaiseEvent` parity,
2. COM event parity:
   - COM-EVT-A (connection point + dispatch sink),
   - COM-EVT-B (source interface/vtable sink).
3. Event callback signature/argument marshaling parity,
4. Subscribe/unsubscribe/reassignment/teardown parity.

### D6. Host project + Office-style hosting completeness
1. Host Project injected semantics parity,
2. Root object model hookup and default/global exposure behavior,
3. Host event ingress unified with runtime event graph,
4. HAL policy/governance compatibility with host services in callback paths.

## 5. Required architecture/program decisions

Decision set to lock before implementation closure:
1. `COM-HAL-V2` contract shape for full invoke semantics (method/property put/putref/named args/lcid/excepinfo surface).
2. Value transport model upgrade for COM boundaries (beyond single `i32` token semantics where required).
3. Default-member resolution source of truth:
   - typelib metadata,
   - Host Project metadata,
   - runtime fallback policy.
4. Assignment semantics model for `Set`/`Let` in parser/binder/emitter/runtime.
5. COM strategy policy:
   - dispatch-first/vtable-first selection,
   - no hidden fallback across compliance lanes.
6. Compliance claim granularity and allowed residual-scope language.

## 6. Detailed execution tracks

### Track A - Spec and obligation closure
1. Build and publish clause-complete spec set for property/default-member/COM invoke/event semantics.
2. Extend obligation catalog with deep invariants:
   - assignment semantic invariants (`Set`/`Let`),
   - invoke flag and named-arg invariants,
   - default-member resolution invariants,
   - RC/lifecycle invariants,
   - event lifecycle invariants.
3. Freeze "claim contract" document that defines exactly what "100%" means.

### Track B - Compiler/binder semantics closure
1. Distinguish assignment intent (`Set`/`Let`) through bound model and emitted instructions.
2. Implement call-context default member rules and ambiguity diagnostics.
3. Move early-bound member/property mapping from hardcoded subset to typelib-driven complete model.
4. Close late-bound classification/arity/named-arg/default-member edges.

### Track C - Runtime/object semantics closure
1. Implement object-reference value model required for true `Set` semantics.
2. Implement property dispatch intent model at runtime (get/let/set).
3. Align error routing and resume semantics for COM/host failures.
4. Validate deterministic teardown and object graph lifecycle parity.

### Track D - COM HAL/runtime closure (client)
1. Extend COM HAL to carry full invoke descriptors and argument bundles.
2. Implement `PROPERTYPUT` and `PROPERTYPUTREF` with named-arg payload.
3. Implement robust VARIANT marshalling coverage (scalar/object/array/byref/out).
4. Expand typelib ingestion for member kinds, flags, default markers, optional params, and dispatch ids.
5. Ensure adapter diagnostics mirror deterministic VBA-compatible failure classification.

### Track E - COM server closure
1. Complete COM server behavior for Office-compatible automation expectations.
2. Ensure typelib/project metadata coherency with callable behavior.
3. Validate client/server roundtrip behavior in controlled fixture and external Office lanes.

### Track F - Events closure
1. Complete non-COM parity path to zero open divergences.
2. Complete COM-EVT-A and COM-EVT-B parity implementation and evidence.
3. Verify event behavior across reassignment, teardown, and reentrancy stress.

### Track G - Office oracle and matrix closure
1. Expand oracle corpus to cover all high-risk semantics:
   - property get/let/set with default members,
   - set/let edge behavior,
   - dispatch flags/named args/optional parameters,
   - COM event callback signatures.
2. Run required matrix:
   - Office version lanes in support scope,
   - x86/x64,
   - deterministic fixture + external oracle.
3. Fold all oracle outcomes back into clause catalog, divergences, and diagnostics registry.

### Track H - Terminal integrated gate
1. Run integrated gate commands for compiler/runtime/host/com/formal/oracle governance.
2. Ensure zero unresolved in-scope divergences and zero pending deferred gates.
3. Publish final compliance dossier and claim text.

## 7. Validation matrix requirements

Required test families:
1. Unit/property tests for parser/binder/runtime/HAL invariants,
2. Differential tests against Office behavior captures,
3. Conformance suites per domain lane (language/runtime/property/com/events/host),
4. Stress suites (reentrancy, teardown, RC pressure, error transitions),
5. Formal harnesses (Kani + proof artifacts where modeled).

Required matrix dimensions:
1. Office version in declared support set,
2. Process bitness (`x86`, `x64`),
3. COM invocation strategy (`dispatch`, `vtable`),
4. Backend execution mode (`VM`, `JIT`) where applicable.

## 8. Evidence package outputs

Required final artifacts:
1. `docs/evidence/conformance/VBA71_WINDOWS_FULL_COMPLIANCE_MATRIX.csv`,
2. `docs/evidence/conformance/VBA71_WINDOWS_FULL_COMPLIANCE_REPORT.md`,
3. `docs/evidence/formal/VBA71_WINDOWS_FULL_COMPLIANCE_FORMAL_REPORT.md`,
4. `docs/evidence/divergences/` entries closed or explicitly re-scoped with approved claim language,
5. diagnostics registry and generated snippets fully synchronized.

## 9. Exit criteria (strict)

This workset is complete only when:
1. all in-scope spec clauses are implemented and mapped to executable tests,
2. no unresolved in-scope divergence remains,
3. no in-scope deferred gate remains open,
4. required Office differential matrix is green,
5. formal obligation set is complete and passing/bounded with explicit approved rationale,
6. compliance claim text is published and consistent with evidence.

## 10. Immediate companion ladder

Execution ladder for this program:
- `docs/worksets/PROFILE_LADDER_2026-03-08_MACH1000_V467_V620_VBA71_WINDOWS_OFFICE_COMPLIANCE.md`

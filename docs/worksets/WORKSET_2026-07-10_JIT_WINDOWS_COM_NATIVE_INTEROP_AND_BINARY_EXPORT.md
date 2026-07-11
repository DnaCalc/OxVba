# Workset: Ideal Windows Interop and Native Tooling Realization

Date: 2026-07-10
Owner: unassigned
Status: accepted; active under AutoRun `bd-59co`
Type: architecture, Windows capability and conformance delivery
Profiles: `PROFILE-WIN-001`, Windows portion of `PROFILE-TOOL-001`
Source review: [`../OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md`](../OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md)

## 1. Outcome

Realize one exact x64 Windows interop architecture shared by VM3 and JIT: authoritative typelib/reference metadata, exact carriers, one verified interop call plan, late and early COM clients, connection-point events with synchronous ByRef writeback, late and early/dual COM serving, outgoing events, VBA7 Declare/pointers/callbacks, JIT-backed wrappers and genuine x64 native DLL/EXE outputs.

The result is full Windows VBA compatibility for the declared target plus a distinct standalone native-output extension. It is not complete when the JIT merely stops declining a fixture, when VM3 alone works, or when a wrapper is relabelled native.

Authority:

- system clauses `WIN-*`, `COM-*`, `NATIVE-IMPORT-001`, `BUILD-*`, `SEC-BOUNDARY-001`, `CONF-*`;
- [`../spec/OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md`](../spec/OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md);
- shared package/JIT contracts from the core profile.

## 2. Claim gates and target

### VBA7 Windows compatibility gate

- supported 64-bit Windows builds;
- x64 OxVba processes, fixtures and artifacts;
- actual 64-bit Excel for Excel-specific rows;
- late/early COM client, events, serving, Declare, pointers/callbacks under both VM3 and JIT;
- in-proc and out-of-proc/apartment rows required by the canonical ledger.

x86/32-bit Office, WOW64, ARM64 and other Windows architectures are outside the accepted profile. They have no active successor workset and carry no implied support.

### Standalone native-output gate

- x64 wrapper artifacts;
- JIT-backed WrappedComServer;
- genuine program-specific native DLL and EXE;
- external clients, versioned ABI, loader/initialization and clean deployment evidence.

Both gates must be green for this overall workset. A mandatory unsupported/unavailable row remains in progress unless a user-approved scope split creates an open successor owner.

## 3. Consumed and produced contracts

Consumes from the core workset:

- CORE-3 typed calls, source Declare legality and class/export facts;
- CORE-4 sealed verified image/descriptors/target metadata;
- CORE-5 exact carriers, helper ABI, interop-safe ownership and sessions;
- CORE-7 ideal JIT lowering/calling/helper architecture;
- CORE-8 persistent JIT sessions/cache and object/blob handoff;
- CORE-9 full differential observable.

Produces for language services:

- stable raw typelib library/type/member/event identities;
- authoritative signatures, attributes, reference order and resolver revision/digest;
- registered/file provenance and broken-reference state.

Language services own virtual content and query projection. Core owns source Declare identity/signature/call-site diagnostics; this workset owns native ABI/import resolution and runtime availability.

## 4. Architectural transformation

| current state | required state | clauses |
|---|---|---|
| COM/native behavior split across compiler/HAL/COM/VM paths | exact compiler descriptors and one COM/native ownership boundary | `SYS-OWN-001`, `WIN-META-001` |
| fixture/bounded metadata routes | authoritative registry/file resolver for all consumers | `WIN-META-001` |
| backend-specific marshalling | one verifier-checked backend-neutral interop plan carrying transport, signatures, ownership, provenance, writeback, cleanup, errors, reentry and apartment policy | `WIN-PLAN-001` |
| VM3 substrate, JIT whole-image decline | shared VM3/JIT late/early/native execution | `COM-CLIENT-001`, `NATIVE-IMPORT-001` |
| queued event snapshots/no writeback | typed synchronous connection-point delivery | `COM-EVENT-001` |
| bounded VM-backed serving | VM3/JIT late and generated early/dual serving | `COM-SERVE-001` |
| bounded Declare/Ptr support | complete x64 calls, helpers and callbacks | `NATIVE-IMPORT-001` |
| VM-backed wrappers, no native output | JIT wrappers plus genuine native artifacts | `BUILD-*` |

## 5. Binding invariants

1. Runtime carriers remain exact canonical values; boundary structs remain boundary-only.
2. `oxvba-com` owns COM metadata, activation, invocation, events, serving and wire conversion.
3. HAL owns policy/capability/delegation, not COM or native ABI semantics.
4. Every interop call is driven by verified compiler/package descriptors.
5. VM3 and JIT consume the same verifier-checked interop plan unchanged and differ only in execution adapter mechanics.
6. Early binding does not silently become IDispatch.
7. ByRef writeback occurs before the native caller observes return.
8. Every reference, pin, temporary, callback and registration has explicit cleanup.
9. JIT-backed hosts do not fall back to VM3.
10. Wrapper and native artifact labels remain honest.
11. Mandatory rows cannot close through ignored tests, feasibility-only work or target narrowing.

## 6. Canonical artifacts and fixtures

Matrices:

1. `WINDOWS_JIT_COM_CLIENT_MATRIX_V1.csv`
2. `WINDOWS_JIT_COM_EVENTS_MATRIX_V1.csv`
3. `WINDOWS_JIT_COM_SERVER_MATRIX_V1.csv`
4. `WINDOWS_JIT_NATIVE_IMPORT_MATRIX_V1.csv`
5. `WINDOWS_NATIVE_EXPORT_AND_PACKAGING_MATRIX_V1.csv`
6. `WINDOWS_ABI_CARRIER_MATRIX_V1.csv`

Each row records target architecture (fixed to x64), process/apartment shape, exact signature, compiler/package/VM3/JIT/build state, metadata source/revision, expected VBA compile/runtime result, transport proof, controlled fixture, artifact/environment hashes, lifecycle/error expectation and evidence owner.

Controlled fixtures include x64 native DLLs; Automation/dual/custom COM servers; in/out-of-proc activation; event sources with ByRef cancellation; typed interface-array and VT_RECORD shapes; error/IErrorInfo/EXCEPINFO; callback/reentry; native and VBA consumers.

## 7. Execution epics

### WIN-0 — Target ledger, authority and rollout

Type: support
Clauses: `PROFILE-WIN-001`, `CONF-MATRIX-001`

Deliver workset/epic/bead graph, six matrices, environment/fixture manifest, old IP-08/COM/native residual classification, registry/process/apartment cleanup policy and explicit compatibility/native-output gates.

First beads: rollout; matrix seed; fixture manifest; historical residual migration; environment provisioning ledger.

Close: every mandatory row has an owner, authority, fixture and delivery-ready path.

### WIN-1 — Authoritative metadata and exact Windows carriers

Type: delivery
Clauses: `WIN-META-001`, `RUNTIME-VALUE-001`, `IMAGE-ABI-001`

Deliver:

- registered/file typelib discovery and GUID/version/LCID/platform/reference selection;
- stable library/type/member/event identities and resolver digest;
- inheritance/default/source interface, coclass activation and broken-reference facts;
- package/provenance and language-service raw metadata handoff;
- x64 BSTR/VARIANT/SAFEARRAY/IUnknown/numeric layout proof;
- nominal interface/object arrays and VT_DISPATCH/VT_UNKNOWN mapping;
- nominal VT_RECORD descriptors and scalar/array behavior;
- copy/drop/ByRef/writeback/identity lifecycle;
- target/capability admission.

First beads: resolver; identity/digest; metadata handoff; layout probes; interface arrays; VT_RECORD; carrier lifecycle; target admission.

Close: every later wire/metadata shape is authoritative, representable, verified and lifecycle-safe.

### WIN-2 — Shared interop plan and Windows session attachment

Type: delivery
Clauses: `WIN-PLAN-001`, `HOST-SESSION-001`, `RUNTIME-ABI-001`

Deliver:

- elaborate descriptors into one verifier-checked plan containing exact transport and signatures, marshalling temporaries and ownership, ByRef writeback order, cleanup, error mapping, metadata/source provenance, reentry and apartment policy;
- migrate existing VM3 paths to the plan;
- VM3/JIT execution adapters that consume the same verified plan unchanged and expose exact observables;
- Windows target admission and interop services attached to the common verified-image project session for x64;
- Windows symbol/calling/unwind/executable-memory policy;
- COM apartment and host policy in session ownership;
- callback/reentry to the correct live session;
- target/ABI/profile-aware admission and comhost backend selection using the CORE-8 cache rather than a private Windows cache.

WIN-2 defines no second project-session type, mutable VBA-state owner, helper registry or cache. It attaches apartment/reentry services and unchanged-plan VM3/JIT adapters to the CORE-5/CORE-8 session contract. Every Windows adapter/helper symbol is generated from the CORE-5 versioned catalog.

The exact shared-plan anchors are late and early COM calls, incoming and
outgoing events, late and dual COM serving, `Declare`, and callbacks. Their rows
must prove one plan identity across VM3 and JIT; the dual-serving anchor also
proves that its vtable route does not fall back to dispatch. Verified OxImage
descriptors are inputs to planning, while the completed plan consumes the common
Core runtime ABI, evaluation, ownership, session, JIT and cache boundaries.
WIN-2 owns that boundary; later Windows epics only consume the plan.

First beads: complete plan types/provenance/verifier; VM3 unchanged-plan adapter; JIT unchanged-plan adapter after the bounded CORE-7 typed-entry/lowering handoff; x64 session attachment after the CORE-8 session/cache handoff; apartment/reentry; comhost selection; plan-identity differential.

Close: all later interop executes through the shared plan inside persistent correctly owned sessions.

### WIN-3 — Late-bound COM client and activation

Type: delivery
Clauses: `COM-CLIENT-001`

Deliver:

- ComCallLate and dynamic object/property/default-member lowering;
- ProgID/CLSID, CLSCTX, registry-view, in/out-of-proc activation;
- ROT/moniker/file GetObject;
- host-returned objects and stable IUnknown identity;
- DISPID lookup/cache and exact DISPPARAMS ordering;
- named/omitted/Optional/ParamArray, put/putref and ByRef;
- scalar/string/object/interface/array/record conversion;
- chaining, IEnumVARIANT and For Each;
- HRESULT/EXCEPINFO/IErrorInfo/LCID/Err;
- cleanup, host policy and reentry.

First beads: activation/GetObject; scalar method/property; named/default/put; ByRef/writeback; arrays/records; identity/chaining/enumeration; error/locale; cleanup stress.

Close: every mandatory late-bound row is green under VM3/JIT and real COM.

### WIN-4 — Early-bound native-vtable COM client

Type: delivery
Clauses: `COM-CLIENT-001`

Deliver typed ComCallEarly planning/lowering, exact vtable slots/system ABI, Automation/custom signatures, HRESULT/out-retval, ByRef/in/out/inout, interface inheritance/QI identity, arrays/records/interfaces and in/out-of-proc proxy behavior.

First beads: scalar vtable transport; properties/defaults/optional; interface returns/QI; arrays/records/ByRef; custom interface; out-of-proc proxy; transport proof.

Close: every mandatory early row uses the general descriptor route and proves no hidden IDispatch fallback.

### WIN-5 — Incoming COM connection-point events

Type: delivery
Clauses: `COM-EVENT-001`

Deliver typed WithEvents/source-interface/handler compile rules; connection-point discovery; IDispatch/custom-vtable sinks; ByVal/object/array args; synchronous VT_BYREF invocation and copyback; cancellation patterns; same-thread and cross-apartment/out-of-proc reentry; ordering/fan-out/replacement/unsubscribe/termination; handler error mapping; shared VM3/JIT event state.

First beads: compiler event binding; ByVal sink; scalar ByRef cancellation; object/array args; custom sink; apartment/reentry; lifecycle/fan-out.

Close: every mandatory event/process/apartment row is parity-complete with synchronous writeback.

### WIN-6 — Late-bound COM serving foundation

Type: delivery
Clauses: `COM-SERVE-001`

Deliver:

- class exposure/instancing and stable CLSID/ProgID/version identity;
- InprocServer32/LocalServer32, threading and the x64 registry view;
- VM3/JIT-backed class factory/session activation;
- stable IUnknown/IDispatch/type-info behavior;
- methods, property groups, defaults, enumeration, Optional/named/ParamArray/ByRef;
- object/array/record/interface values;
- HRESULT/EXCEPINFO/IErrorInfo and class lifecycle;
- Automation versus proxy/stub strategy;
- LocalServer message-loop/class-object/lock/shutdown behavior;
- clean registration/deployment/unload.

First beads: identity/registration policy; VM3/JIT scalar class; IDispatch breadth; complex values/ByRef; errors/lifecycle; LocalServer; clean deployment.

Close: every mandatory exported-class/activation row is late-bound callable without project-specific Rust code.

### WIN-7 — Early/dual serving and Implements

Type: delivery
Clauses: `COM-SERVE-001`

Deliver generated signature-driven vtable entry thunks; dual/custom QI; wire-to-OxVba calls/returns; imported Implements; one identity/state across early/late; matching typelibs; arrays/records/interfaces/ByRef; errors; required proxy/stub artifacts; Excel/VBA early reference.

First beads: scalar dual vtable; imported interface; complex shapes; mixed-client identity; typelib parity; proxy/stub packaging.

Close: every mandatory early/late/marshalling row uses one served object and signature-driven path.

### WIN-8 — Outgoing COM events

Type: delivery
Clauses: `COM-EVENT-001`, `COM-SERVE-001`

Deliver source interfaces/connection-point containers, sink cookies/lifetime, RaiseEvent-to-native invocation, all mandatory argument/ByRef shapes, ordering/fan-out/errors/teardown and VBA WithEvents consumption for VM3/JIT-backed servers.

First beads: scalar event; multi-sink lifecycle; object/array/ByRef; VM3/JIT parity; Excel consumer.

Close: served classes are complete event sources for every mandatory row.

### WIN-9 — VBA7 Declare import

Type: delivery
Clauses: `NATIVE-IMPORT-001`

Deliver compile/oracle legality for PtrSafe/LongPtr/LongLong/VBA7/Win64/x64 convention/aliases/As Any; verified external-call plan; secure DLL/entry/ordinal resolution; x64 calls; scalar returns; ByVal/ByRef/writeback; ANSI/Wide/BSTR buffers; arrays/UDTs/As Any; immediate LastDllError capture; cleanup; exact missing/policy diagnostics.

First beads: compile matrix; secure loader; x64 scalar; returns/ByRef/error; strings/buffers; arrays/UDTs/As Any; policy/missing symbols.

Close: full mandatory import matrix is green under VM3/JIT/VBA/native fixtures.

### WIN-10 — Pointer helpers and AddressOf callbacks

Type: delivery
Clauses: `NATIVE-IMPORT-001`

Deliver compile-time AddressOf eligibility; exact VarPtr/StrPtr/ObjPtr/addressability; typed callback thunks; owning-session/thread/apartment reentry; arguments/results/ByRef/errors; synchronous and retained registration/release lifetimes; nested native/COM/VBA cycles; x64; UAF/stale-thunk safety.

First beads: pointer storage; compile matrix; synchronous scalar callback; signature breadth; nested reentry/errors; retained lifetime; stale/disposed safety.

Close: every mandatory pointer/callback/lifetime row is exact and lifecycle-safe.

### WIN-11 — JIT-backed wrapped outputs

Type: delivery
Clauses: `BUILD-PACKAGE-001`, `BUILD-CLASS-001`

Deliver verified embedded/deployed `.oxi`; JIT WrapperExe/WrapperLibrary; JIT WrappedComServer; deterministic entries; ABI/target/source-map manifests; reset/unload/DllCanUnloadNow; compiler-free deployment where promised; loader-lock-safe explicit/lazy initialization; clean-machine evidence.

First beads: wrapper EXE; wrapper library; WrappedComServer; loader-lock instrumentation; deployment/version rejection.

Close: all wrapper classes run standalone through the selected backend without fallback and remain honestly labelled.

### WIN-12 — Genuine native DLL/EXE outputs

Type: delivery
Clauses: `BUILD-NATIVE-001`, `JIT-AOT-001`

Deliver project export manifest/signature eligibility; versioned external ownership/error/concurrency ABI; Cranelift object/blob and PE/COFF relocation/import format; x64 entries/names/ordinals; runtime/helper initialization outside loader lock; DLL/EXE global/session/unload rules; source/debug maps; ASLR/clean-machine/reproducible builds; native C/Rust clients.

First beads: ABI/export contract; object/blob prototype; scalar x64 DLL; signature breadth; native EXE; reloc/import/ASLR; reproducibility/debug maps.

Close: mandatory x64 native targets are real program-specific outputs, not wrappers.

### WIN-13 — Native-boundary safety and lifecycle

Type: delivery/conformance
Clauses: `SEC-BOUNDARY-001`, `CONF-QUALITY-001`

Deliver descriptor mutation/fuzz, ASAN/fault injection, QI/refcount/connection/pin/callback balance, hostile/missing COM/native dependencies, apartment enforcement, reentrancy, registration/temp cleanup and explicit host capability profiles.

Close: malformed metadata and hostile failures cannot corrupt state, leak resources or damage unrelated processes/registry.

### WIN-14 — Windows, native-client and Excel/VBA certification

Type: delivery/conformance
Clauses: `CONF-ORACLE-001`, `CONF-DIFF-001`

Deliver pinned Windows/Office builds; actual 64-bit Excel; x64 native runners; controlled COM fixtures; non-default locale; compiler diagnostics; VM3/JIT outputs; Excel clients of external and served objects; bidirectional events/ByRef cancellation; native clients of exports; registration/load/unload/restart; artifact/environment hashes; promoted named runners; archived UIA/VBE/cleanup evidence.

Close: every mandatory row has its specified controlled and real external/Excel authority; unavailable environments remain blockers.

### WIN-15 — Terminal architecture and profile release

Type: support/conformance
Clauses: `CONF-DONE-001`, `DOC-AUTH-001`, `DOC-TRACE-001`

Reconcile system/Windows/JIT/package contracts, current architecture, code, matrices, old COM ladders/blockers, output labels and derived compatibility/native-output reports. Run final security/deployment/user-path fresh-eyes review.

Close: both internal claim gates and every required delivery epic are green.

## 8. Dependency graph

| epic | hard prerequisites |
|---|---|
| WIN-0 | accepted workset and CORE-0 truth shape |
| WIN-1 | starts after CORE-1 and WIN-0; compiler/package/carrier consumers wait for the exact CORE-3/4/5 producer slices |
| WIN-2 | schema/rollout work starts after the WIN-1 metadata handoff; plan verification waits for exact CORE-4/5 descriptor/catalog slices, the JIT adapter waits for a bounded CORE-7 slice, and session attachment waits for a bounded CORE-8 slice |
| WIN-3 | WIN-1, WIN-2 |
| WIN-4 | WIN-1, WIN-2 |
| WIN-5 | WIN-1 event metadata, WIN-2, WIN-3 and applicable WIN-4 transport |
| WIN-6 | CORE-3/4 class/export facts, WIN-1, WIN-2 |
| WIN-7 | WIN-1, WIN-6 |
| WIN-8 | WIN-5 and applicable WIN-6/7 |
| WIN-9 | CORE-3 Declare legality, WIN-1, WIN-2 |
| WIN-10 | WIN-2, WIN-9 |
| WIN-11 | CORE-4/8, WIN-2; WrappedComServer also WIN-6/7/8 |
| WIN-12 | CORE-4/5/7/8, WIN-1/2 and relevant WIN-11 loader work |
| WIN-13 | safety scaffolding starts after CORE-1 and WIN-0; closes after WIN-3 through WIN-12 |
| WIN-14 | clean certification-VM provisioning starts after CORE-0; final certification waits for WIN-0 through WIN-13 |
| WIN-15 | WIN-14 and both claim gates |

Late COM client and scalar Declare can proceed in parallel after their WIN-1/2 slices. Serving and events share metadata/session/transport prerequisites. Safety, fixture construction and clean-VM provisioning run continuously instead of waiting for capability completion. Coarse Core-3/4/5/7/8 epic dependencies must not be restored where an exact producer leaf can express the handoff.

## 9. Checks and terminal condition

Per bead: targeted VM3/JIT/compiler/build tests, exact transport/ABI proof, balance and fault neighbor, matrix/contract update, fresh-eyes review.

Merge gate: strict workspace gates on Windows, x64 controlled fixtures, default/single-thread stress, governance/truth checks and no stale ignored touched row.

Release gate: actual 64-bit Excel, in/out-of-proc COM, x64 native artifacts/clients, ASAN/fault injection, clean deployment/registration/unload, locale, performance/size and current docs/matrices.

This workset is complete only when every mandatory x64 compatibility row works under both VM3 and JIT, 64-bit Excel evidence is green, wrappers and genuine native outputs pass their distinct gates, all unsafe/lifecycle checks are clean and documentation/artifacts/matrices tell the same truth.

## 10. Bead-preparation handoff

Create WIN-0 through WIN-15 epics and rollout beads, then materialize the first candidates above. Every bead names contract clauses, `target=x64`, process/apartment shape, matrix rows, fixture/environment prerequisites, dependencies, exact evidence and blocker/residual behavior. Registry/fixture mutation is serialized. Capability epics cannot close on docs, matrices or fixture rollout alone.

## 11. Exact routed contract responsibility

The clause lists in the epic sections state each outcome's primary contract. The complete producer, consumer and matrix-boundary responsibility exercised by its executable leaves is:

- WIN-0: `CONF-MATRIX-001|CONF-ORACLE-001|DOC-AUTH-001|DOC-TRACE-001|PROFILE-WIN-001`
- WIN-1: `COMP-BIND-001|IMAGE-ABI-001|PROJ-REF-001|RUNTIME-VALUE-001|WIN-META-001`
- WIN-2: `HOST-SESSION-001|IMAGE-VERIFY-001|JIT-CACHE-001|JIT-CORE-001|RUNTIME-ABI-001|RUNTIME-EVAL-001|SYS-OWN-001|WIN-PLAN-001`
- WIN-3: `COM-CLIENT-001|WIN-PLAN-001`
- WIN-4: `COM-CLIENT-001|COMP-BIND-001|PROJ-REF-001|WIN-META-001|WIN-PLAN-001`
- WIN-5: `COM-EVENT-001|CONF-ORACLE-001|CONF-QUALITY-001|JIT-PARITY-001|SEC-BOUNDARY-001|WIN-PLAN-001`
- WIN-6: `COM-SERVE-001|WIN-PLAN-001`
- WIN-7: `COM-SERVE-001|WIN-PLAN-001`
- WIN-8: `COM-EVENT-001|COM-SERVE-001|WIN-PLAN-001`
- WIN-9: `COMP-BIND-001|COMP-DIAG-001|NATIVE-IMPORT-001|WIN-PLAN-001`
- WIN-10: `CONF-ORACLE-001|CONF-QUALITY-001|NATIVE-IMPORT-001|RUNTIME-ABI-001|SEC-BOUNDARY-001|WIN-PLAN-001`
- WIN-11: `BUILD-CLASS-001|BUILD-PACKAGE-001|HOST-SESSION-001|PROFILE-TOOL-001|SYS-ART-001`
- WIN-12: `BUILD-CLASS-001|BUILD-NATIVE-001|DEBUG-MAP-001|HOST-SESSION-001|IMAGE-VERIFY-001|JIT-AOT-001|JIT-CACHE-001|PROFILE-TOOL-001|SYS-ART-001`
- WIN-13: `CONF-QUALITY-001|HOST-HAL-001|SEC-BOUNDARY-001|VM3-SAFE-001`
- WIN-14: `AUTH-CLEAN-001|AUTH-SPEC-001|AUTH-VBA-001|COM-CLIENT-001|COM-EVENT-001|COM-SERVE-001|CONF-DIFF-001|CONF-ORACLE-001|CONF-QUALITY-001|JIT-PARITY-001|NATIVE-IMPORT-001|PROFILE-TOOL-001|PROFILE-WIN-001|SYS-DUAL-001|VM3-REF-001`
- WIN-15: `CONF-DONE-001|DOC-AUTH-001|DOC-TRACE-001|PROFILE-TOOL-001|PROFILE-WIN-001`

The canonical disposition and trace ledgers remain machine authority for these routes; any change updates this appendix, the epic contract and those ledgers together.

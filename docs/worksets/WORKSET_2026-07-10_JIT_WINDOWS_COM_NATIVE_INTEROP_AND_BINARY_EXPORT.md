# Workset: Windows JIT COM, Native Interop, and Binary Export

Date: 2026-07-10
Owner: unassigned
Status: proposed; bead rollout not yet performed
Type: Windows capability delivery and conformance
Source review: [`OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md`](../OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md)

## 1. Outcome

Complete the Windows-only execution and deployment capabilities that the Linux JIT implementation could not deliver:

- late-bound COM client import through `IDispatch`;
- early-bound COM client import through native vtables;
- COM events and connection points, including synchronous ByRef event writeback;
- OxVba/VBA classes served as late-bound and early-bound COM objects;
- outgoing events from served classes;
- Windows native DLL import through VBA `Declare`;
- `AddressOf` callbacks and pointer-helper/native-memory behavior;
- wrapped and native DLL/EXE output backed by the JIT and the shared `OxImage` package.

The terminal state is one coherent Windows ABI and object model shared by VM3 and JIT, with real Excel/VBA and external native/COM evidence. Portable Linux object projections and fixture-only metadata are useful unit scaffolds but do not satisfy a Windows capability row.

## 2. Relationship to the other worksets

This workset consumes from [`WORKSET_2026-07-10_POST_JIT_CORE_CONFORMANCE_AND_READINESS.md`](WORKSET_2026-07-10_POST_JIT_CORE_CONFORMANCE_AND_READINESS.md):

- sealed verified program/image loading from CORE-4;
- helper/carrier ABI versions and sound runtime ownership from CORE-4/CORE-5;
- stable JIT sessions/cache from CORE-7;
- complete core error, call, lifecycle and recursion semantics from CORE-6;
- full structural differential observables from CORE-8.

It provides to [`WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md`](WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md):

- authoritative COM typelib/reference metadata;
- served class/interface/event descriptions;
- native ABI/import resolution, runtime availability and stable invocation diagnostics; Core owns source `Declare` identity/signature/call-site diagnostics;
- stable raw metadata identities, signatures, attributes, revision/digest and registered/file-backed provenance. The language-service workset owns virtual URIs/content and query projection.

Early fixture and matrix work can begin in parallel. General native codegen, serving and release closure must not build on an unverified package or unstable helper ABI.

## 3. Scope

### 3.1 In scope

#### COM client

- `CreateObject`, `GetObject`, host-returned COM objects and retained object identity;
- `IUnknown` identity, `QueryInterface`, `AddRef` and `Release`;
- late-bound `IDispatch::GetIDsOfNames`/`Invoke`;
- early-bound dual/custom interface vtable calls;
- properties, default members, methods and statement/function forms;
- named/positional/omitted/Optional/ParamArray arguments;
- ByVal/ByRef, put/putref and writeback;
- object/interface returns and chaining;
- `IEnumVARIANT`/`For Each`;
- `VARIANT`, `BSTR`, `SAFEARRAY`, typed interface arrays and `VT_RECORD`;
- COM errors, `EXCEPINFO`, `IErrorInfo`, HRESULT and Err mapping;
- in-proc/out-of-proc, STA/reentrancy and lifecycle behavior;
- locale/LCID behavior.

#### COM events

- source-interface discovery;
- connection-point enumeration, `Advise`/`Unadvise`;
- sink `IDispatch` and typed event args;
- event ordering, fan-out, replacement and teardown;
- synchronous ByRef writeback before source `Invoke` returns;
- nested/reentrant calls and handler errors;
- cross-apartment/out-of-process policy;
- VM3 and JIT event delivery through shared runtime state.

#### COM serving

- class factory, registration and activation;
- `IUnknown`/`IDispatch` and type-info behavior;
- generated dual/custom vtables for every mandatory typelib signature row;
- properties, methods, default members, enums and classes;
- `Implements` interfaces and QueryInterface identity;
- early- and late-bound clients of the same object;
- object/interface/array/record values and ByRef;
- outgoing source interfaces and connection points;
- error/HRESULT/IErrorInfo projection;
- threading/apartment, reentrancy, lifetime and unload rules;
- wrapped in-proc DLL and out-of-proc EXE hosting for the mandatory target ledger.

#### Native import

- VBA 7 `Declare PtrSafe Sub/Function`;
- x86 and x64 calling conventions;
- library/name/alias resolution;
- all VBA7 scalar, pointer, string, array, UDT and `As Any` shapes in the mandatory target ledger;
- ByVal/ByRef, return values and writeback;
- ANSI/Wide string and buffer rules;
- `VarPtr`, `StrPtr`, `ObjPtr` and addressability;
- `AddressOf` callback thunks and callback reentry;
- `Err.LastDllError` and error mapping;
- host policy/capability gating and deterministic unsupported diagnostics.

#### Native/wrapped output

- JIT-backed `WrapperExe` and `WrapperLibrary`;
- JIT-backed `WrappedComServer`;
- general native DLL exports from VBA procedures selected by the project export manifest defined in WIN-12;
- native EXE entry/shim;
- Cranelift object/blob generation, relocation and PE packaging;
- export names, ordinals and calling convention;
- embedded verified `OxImage`, helper/carrier ABI and target metadata;
- cold/warm loading, registration and deployment;
- debug/source-map sidecars.

### 3.2 Explicitly out of scope

- Office object-model reimplementation;
- a replacement Windows registry or COM subsystem;
- arbitrary unsafe FFI without a declared signature/policy;
- silent VM fallback inside a JIT-backed output;
- a forms designer or IDE;
- language-service transport;
- platform-neutral compiler/runtime gaps owned by the core workset.

### 3.3 Architecture target

The target is:

`verified OxImage
  -> shared Windows-aware runtime/carriers
       |-- VM3
       \-- JIT
            |-- helper-mediated native/COM calls where required
            |-- direct typed vtable/Declare calls where proven safe
            \-- generated native entry thunks for serving/export

oxvba-com owns COM semantics and wire conversion
oxvba-hal owns capability/policy/bootstrap/delegation
oxvba-rt-abi owns versioned engine-neutral runtime ABI
oxvba-build owns deterministic output planning/packaging
oxvba-comhost or successor owns reusable host entry points`

Boundary structs such as `DISPPARAMS` and `EXCEPINFO` remain COM-boundary data, not canonical VM/JIT values.

Compiler/package descriptors must elaborate into one verified backend-neutral interop call plan: exact signature, transport, marshalling temporaries, ownership/cleanup, ByRef writeback order, HRESULT/native-error-to-Err mapping and reentry policy. VM3 and JIT use separate execution adapters for that plan; neither reconstructs semantics or maintains an independent signature allowlist. Existing VM routes must migrate to the shared plan and remain the differential reference.

### 3.4 Target and claim gates

The mandatory VBA7 Windows compatibility target uses supported 64-bit Windows builds with both x64 and WOW64/x86 OxVba processes, COM/native fixtures and artifacts. Actual 64-bit Excel and actual 32-bit Excel run in separate owned Windows x64 environments. A generic x86 VBA host may substitute only for non-Excel compile/runtime rows, never for Excel object/event rows. The standalone native-output extension likewise requires x64 and x86 artifacts/clients. A 32-bit Windows OS is not required. ARM64 has an explicit status row but is not silently implied.

The workset exposes two internal gates:

1. **VBA7 Windows compatibility:** COM import/serving/events and native import/pointer/callback parity.
2. **Standalone native-output extension:** wrapped and genuine native DLL/EXE output.

The overall terminal gate requires both. An unavailable or unsupported mandatory row remains `in-progress` with a documented blocker. It may leave the target only through a user-approved workset scope split with an open successor delivery owner; an ignored test, “accepted subset” label or feasibility note cannot close it.

## 4. Current entry state

Verified in the 2026-07-10 review:

- VM3 and `oxvba-com` already contain substantial live Windows COM and Declare support.
- Current live COM/Office tests are mostly ignored/operator-run.
- The JIT declines an entire image if `external_calls` or `com_interfaces` is non-empty.
- `ComCallEarly`, imported COM activation, Declare/native calls and `OxInst::Ptr` are not JIT-lowered.
- portable `OxVba.TestDispatch` and project-object tests are not native COM evidence.
- project `WithEvents`/`RaiseEvent` does not prove COM connection points.
- current COM ByRef event transport queues value snapshots and cannot write back synchronously.
- package array types cannot express nominal COM object/interface element arrays.
- record layouts are not sufficient for broad nominal `VT_RECORD` interop.
- no persistent JIT comhost/package session, generated JIT COM vtable, native DLL export or AOT PE loader exists.
- Windows VM native-import tests execute successfully but stale JIT diagnostic assertions keep the ordinary host lane red.

## 5. Canonical matrices and fixtures

Rollout must create or designate:

1. `docs/validation/WINDOWS_JIT_COM_CLIENT_MATRIX_V1.csv`
2. `docs/validation/WINDOWS_JIT_COM_EVENTS_MATRIX_V1.csv`
3. `docs/validation/WINDOWS_JIT_COM_SERVER_MATRIX_V1.csv`
4. `docs/validation/WINDOWS_JIT_NATIVE_IMPORT_MATRIX_V1.csv`
5. `docs/validation/WINDOWS_NATIVE_EXPORT_AND_PACKAGING_MATRIX_V1.csv`
6. `docs/validation/WINDOWS_ABI_CARRIER_MATRIX_V1.csv`

Each row must name:

- architecture/bitness;
- in-proc/out-of-proc/apartment shape;
- compiler/package/VM3/JIT/build status;
- exact value/wire signature;
- error/lifecycle expectations;
- controlled fixture;
- authoritative metadata/reference source and resolver revision;
- expected Excel compile result where source-visible;
- transport counter for early-bound rows proving the vtable route;
- backend, fixture/artifact hash and 32/64-bit registration view;
- real external and/or Excel/VBA oracle evidence;
- evidence artifact ID;
- residual owner.

Required controlled fixtures:

- x86 and x64 native DLLs with deterministic exports;
- Automation/dual/custom-interface COM servers;
- in-proc and out-of-proc COM fixtures;
- event source with by-value, object, array and synchronous ByRef scalar args;
- COM server consumer fixtures in native code and VBA;
- typed object/interface SAFEARRAY and `VT_RECORD` fixtures;
- error/IErrorInfo/EXCEPINFO fixtures;
- callback APIs for `AddressOf`, nested callbacks and failure paths.

Fixtures must exercise the general production mechanism. Dependency-specific hardcoding is not closure evidence.

## 6. Binding invariants

1. Exact runtime carrier storage remains the canonical value substrate.
2. COM policy and wire translation live in `oxvba-com`.
3. HAL does not become the owner of COM semantics.
4. Typed descriptors originate in the compiler/package and are verified before codegen.
5. Early-bound calls do not silently fall back to late binding unless VBA does so for that exact case.
6. Late-bound calls do not use fixture-specific name/token tables as the general path.
7. ByRef writes occur before the native caller observes return.
8. All owned COM references and native pins have explicit cleanup on success, fault, panic, handler reentry and cancellation.
9. JIT-backed hosts do not fall back to VM3.
10. Both VM3 and JIT map the same HRESULT/native error to the same VBA Err result.
11. Cross-apartment behavior is explicit; unsupported reentry is rejected before unsafe execution.
12. A wrapped output is labelled wrapped. A native/AOT output is not claimed until the generated code and loader/export path are real.
13. Mandatory-target rows cannot become green through target narrowing, ignored tests or a feasibility-only bead.

## 7. Execution epics

### WIN-0 — Initiation, target matrix and fixture rollout

Type: support

Required outcomes:

- create the workset root, epics and rollout beads;
- seed all six canonical matrices;
- inventory existing COM/native fixtures, ignored tests, evidence and blockers;
- classify every current VM3 row as reusable reference, incomplete, fixture-only or stale;
- define Windows target tiers:
  - x64 Windows/Office required;
  - x86 Windows/Office required for VBA7 pointer/calling-convention parity;
  - ARM64 compile/runtime status explicitly decided and never implied;
- define owned process, registry, apartment and cleanup rules;
- publish a target/claim ledger that distinguishes compatibility and native-output gates and records every mandatory row, blocker and approved scope split;
- leave ready delivery beads for target ABI, late COM and Declare.

First bead candidates:

| candidate | type | outcome | close evidence |
|---|---|---|---|
| WIN-0.1 | support | roll out all WIN epics | executable graph and next ready delivery beads |
| WIN-0.2 | support | seed Windows matrices from existing tests/evidence | no ignored or historical row is unclassified |
| WIN-0.3 | support | publish controlled fixture/environment manifest | reproducible build/register/run/cleanup instructions |
| WIN-0.4 | support | classify old IP-08B/COM/native blockers into this graph | no residual exists only in narrative docs |

### WIN-1 — Windows ABI, authoritative type metadata and exact carrier completion

Type: delivery

Required outcomes:

- version and verify Windows ABI/layout facts;
- implement authoritative registered/file-backed typelib discovery and reference resolution: GUID/version/LCID/platform selection, registry view, file provenance, reference precedence, aliases and broken-reference diagnostics;
- publish stable library/type/member/event identities, signatures, attributes, inherited/default/source-interface relationships, coclass activation metadata and resolver revision/digest;
- serialize the selected type/reference provenance needed by package verification and downstream language services;
- assert `BSTR`, `VARIANT`, `SAFEARRAY`, `IUnknown` and numeric layouts on x86/x64;
- add nominal object/interface array element types with exact `VT_DISPATCH`/`VT_UNKNOWN` mapping;
- add nominal record identity, GUID/typeinfo/size/alignment/field metadata for `VT_RECORD`;
- define object/interface/record array default, clone, drop, erase, preserve and ByRef behavior;
- validate Decimal, Currency, Date, Boolean and pointer-width carriers;
- define COM pointer/interface identity and QueryInterface ownership;
- ensure package target/capability requirements reject incompatible images.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| WIN-1.1 | x86/x64 ABI layout contract | compile-time/runtime layout probes |
| WIN-1.2 | nominal COM object/interface arrays | SAFEARRAY round-trip through VM3 and fixture |
| WIN-1.3 | nominal record descriptors | `VT_RECORD` scalar/array round-trip |
| WIN-1.4 | carrier copy/drop/writeback matrix | zero-leak success/fault/reentry tests |
| WIN-1.5 | package target admission | incompatible bitness/ABI rejects before execution |
| WIN-1.6 | authoritative typelib/reference resolver | registry/file, version/LCID/platform, precedence and broken-reference matrix |
| WIN-1.7 | stable raw COM metadata handoff | deterministic identities/signatures/attributes/digest consumed by package and language services |

Close condition: every Windows wire shape required by later epics is representable and lifecycle-safe in the shared package/runtime.

### WIN-2 — JIT Windows session and native-boundary substrate

Type: delivery

Required outcomes:

- enable verified-image JIT sessions on Windows;
- register all required helper/native symbols without relying on a Unix global symbol namespace;
- validate Cranelift Windows calling convention and unwind/panic boundaries;
- implement executable memory policy and code lifetime;
- thread COM apartment and host policy into session creation;
- make reentry from COM/native callback reach the correct live session;
- define cache keys including target triple, bitness, helper/carrier ABI and COM/native capability profile;
- provide comhost/wrapper backend selection with no fallback.
- elaborate verified descriptors into the shared backend-neutral interop call plan and migrate VM3 to consume it;
- provide VM3/JIT adapters with exact marshalling, cleanup, writeback and Err/HRESULT differential evidence.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| WIN-2.1 | Windows x64 JIT package session | load/invoke/reset/drop integration |
| WIN-2.2 | Windows x86 JIT package session and ABI path | executable x86 load/invoke/reset/drop evidence; a feasibility blocker does not close this delivery bead |
| WIN-2.3 | apartment-aware session ownership | STA/reentry/lifetime tests |
| WIN-2.4 | JIT comhost backend selection | selected backend observable and no fallback |
| WIN-2.5 | verified shared interop call plan | VM3/JIT adapters consume identical plan and observables |

Close condition: later COM/native calls execute inside a persistent, correctly owned Windows JIT session.

### WIN-3 — Late-bound COM client (`IDispatch`)

Type: delivery

Required outcomes:

- lower `ComCallLate` and dynamic object/default-member/property operations;
- support `CreateObject`, `GetObject` and host-returned COM objects;
- implement ProgID/CLSID resolution, `CLSCTX`, x86/x64 registry-view selection and in-proc/out-of-proc activation under explicit host capability policy;
- implement ROT, moniker and file-based `GetObject` forms with exact VBA Err mapping;
- perform correct name/DISPID lookup and caching;
- construct `DISPPARAMS` with VBA argument reversal, named DISPIDs and property-put marker;
- handle Optional/Missing/ParamArray, ByRef and put/putref;
- convert all mandatory scalar/string/object/array/record value rows;
- preserve `IUnknown` identity across returns and arguments;
- support object-result chaining and `IEnumVARIANT`;
- map HRESULT, `EXCEPINFO` and `IErrorInfo` to Err;
- respect LCID and statement/function context;
- clean up all VARIANT/BSTR/SAFEARRAY/interface temporaries on every edge.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| WIN-3.1 | minimal real `IDispatch` method/property call | JIT vs VM3 controlled COM differential |
| WIN-3.2 | named/omitted/default-member/put matrix | TestDispatch plus real Scripting evidence |
| WIN-3.3 | ByRef/writeback and arrays | scalar/Variant/SAFEARRAY mutation evidence |
| WIN-3.4 | object identity/chaining/enumeration | Excel/Scripting identity and For Each rows |
| WIN-3.5 | COM error/locale mapping | exact Err/EXCEPINFO/IErrorInfo/LCID matrix |
| WIN-3.6 | cleanup/reentry stress | handle/QI balance under loops and faults |
| WIN-3.7 | COM activation and `GetObject` | ProgID/CLSID/CLSCTX/registry/ROT/moniker/file/policy matrix under VM3, JIT and VBA |

Close condition: every mandatory late-bound target row is green under the JIT against VM3 and real COM.

### WIN-4 — Early-bound COM client (native vtables)

Type: delivery

Required outcomes:

- lower `ComCallEarly` from verified typed member descriptors;
- generate or select correct caller thunks for Automation and custom dual interfaces;
- call exact vtable slots with the Windows system ABI;
- support scalar, BSTR, VARIANT, SAFEARRAY, typed interface pointer and record shapes;
- support HRESULT/out-retval, ByRef/in/out/inout and Optional forms;
- preserve interface inheritance, QueryInterface and vtable identity;
- keep early-bound dispatch early; no hidden IDispatch route;
- provide a documented fallback decision only where real VBA/COM does;
- test in-proc and proxy/out-of-proc interfaces.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| WIN-4.1 | scalar dual-interface vtable call | transport counter proves no IDispatch |
| WIN-4.2 | property/default/named/optional signatures | descriptor-driven matrix |
| WIN-4.3 | object/interface return and QI | identity and lifetime evidence |
| WIN-4.4 | SAFEARRAY/record/ByRef breadth | controlled and third-party fixture rows |
| WIN-4.5 | out-of-proc proxy behavior | Excel/DAO or controlled local server evidence |

Close condition: the general typed descriptor path covers every mandatory early-bound target row without interface-name allowlists.

### WIN-5 — COM connection points and event delivery

Type: delivery

Required outcomes:

- implement compiler/package binding for typed `WithEvents`, source-interface selection, handler-name/signature matching and compile-time diagnostics;
- discover source interfaces and connection points from authoritative metadata;
- build both Automation `IDispatch` and custom-vtable sink objects where the selected source interface requires them, with correct lifetime and QueryInterface;
- deliver ByVal and object/array event args;
- add synchronous handler invocation for supported ByRef args before `IDispatch::Invoke` returns;
- copy handler mutations back to source-owned `VT_BYREF` storage;
- handle event cancellation patterns such as `Workbook.BeforeClose(Cancel)`;
- define and test same-thread reentry;
- implement cross-apartment/out-of-process synchronous reentry for mandatory rows; a temporary safety rejection remains an open blocker until a user-approved scope split;
- preserve ordering, fan-out, replacement, unsubscribe and termination timing;
- map handler faults and source HRESULTs consistently;
- share event runtime state between VM3 and JIT.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| WIN-5.1 | by-value connection-point event under JIT | controlled source fires ordered events |
| WIN-5.2 | synchronous scalar ByRef event writeback | source observes handler mutation before return |
| WIN-5.3 | object/array event args | identity, mutation and cleanup matrix |
| WIN-5.4 | reentry/apartment policy | nested call and rejection tests |
| WIN-5.5 | lifecycle/fan-out/unadvise | leak-free replacement and termination stress |
| WIN-5.6 | typed event binding and custom source interfaces | compiler/oracle matrix plus custom-vtable sink transport proof |

Close condition: every mandatory apartment/process event row is parity-complete, including ByRef cancellation.

### WIN-6 — Late-bound COM serving from OxVba classes

Type: delivery

Required outcomes:

- define class exposure/instancing policy and stable CLSID/ProgID/version identity;
- define `InprocServer32`/`LocalServer32`, threading-model and 32/64-bit registry-view registration rules;
- select Automation marshaler versus generated proxy/stub strategy per interface and deliver every mandatory custom out-of-process shape; a temporary rejection remains an open blocker;
- implement out-of-process message-loop, class-object registration/revocation, lock/lifetime and shutdown behavior where `LocalServer32` is in scope;
- run served classes inside a verified JIT package session;
- implement class factory, activation, locking and unload behavior;
- expose stable `IUnknown` and `IDispatch` identity;
- provide `GetTypeInfoCount`, `GetTypeInfo`, `GetIDsOfNames` and `Invoke`;
- project methods, Property Get/Let/Set, default members, enumerators and events;
- support named/Optional/ParamArray and ByRef/writeback;
- project return values and errors through HRESULT/EXCEPINFO/IErrorInfo;
- generate deterministic typelib/registration descriptors from the package;
- support multiple objects/sessions and correct class lifecycle.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| WIN-6.1 | JIT-backed late-bound scalar class | native and VBA CreateObject smoke |
| WIN-6.2 | property/default/member breadth | IDispatch matrix from VBA and native client |
| WIN-6.3 | object/array/record and ByRef serving | wire-shape matrix |
| WIN-6.4 | error and lifecycle semantics | IErrorInfo, Terminate and unload evidence |
| WIN-6.5 | registration/deployment isolation | per-user fixture install/uninstall with clean registry |
| WIN-6.6 | server identity, instancing and local-server policy | stable IDs, registry views, message-loop/lifetime and clean restart evidence |

Close condition: every mandatory exported-class/activation row is callable late-bound without project-specific generated Rust code.

### WIN-7 — Early-bound/dual COM serving and Implements

Type: delivery

Required outcomes:

- generate native vtable entry thunks per served interface/member signature;
- expose dual/custom interfaces through QueryInterface;
- map COM wire arguments to typed OxVba calls and returns;
- support imported interface `Implements` without hardcoded Office profiles;
- keep late- and early-bound views on one object identity/state;
- generate/load matching type libraries;
- generate or package required proxy/stub artifacts for every mandatory non-Automation custom interface; any undelivered row remains open rather than assuming Automation marshaling;
- support scalar, interface, SAFEARRAY, record and ByRef shapes;
- map faults to HRESULT/IErrorInfo;
- validate Excel/VBA early-bound references to the generated typelib.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| WIN-7.1 | generated scalar dual vtable | native and VBA early-bound client passes |
| WIN-7.2 | imported interface `Implements` | descriptor-driven custom interface test |
| WIN-7.3 | complex wire-shape vtables | arrays/records/interfaces/ByRef matrix |
| WIN-7.4 | dual identity and mixed clients | early/late calls share state and IUnknown |
| WIN-7.5 | typelib parity | MIDL/OleAut-generated typeinfo comparison |

Close condition: the serving path is signature-driven and supports every mandatory early/late client and marshaling row over the same object.

### WIN-8 — Outgoing COM events from served classes

Type: delivery

Required outcomes:

- publish source interfaces and connection-point containers;
- manage sink enumeration, cookies and lifetime;
- lower OxVba `RaiseEvent` to native sink invocation;
- marshal every mandatory event-argument and ByRef row;
- preserve ordering, fan-out, errors and teardown;
- support VBA `WithEvents` consumers of OxVba-served classes;
- validate both JIT- and VM3-backed serving for every shared serving/event matrix row; only JIT-specific native artifact generation may be backend-specific.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| WIN-8.1 | scalar outgoing event | native and VBA sink evidence |
| WIN-8.2 | multi-sink/lifecycle matrix | enumerate/advise/unadvise/fan-out |
| WIN-8.3 | object/array/ByRef event shapes | exact wire/writeback evidence |
| WIN-8.4 | Excel host scenario | owned workbook/add-in event smoke |

Close condition: served event sources behave as Automation/dual COM sources for every mandatory event target row.

### WIN-9 — JIT Windows `Declare` import

Type: delivery

Required outcomes:

- match Excel/VBA compile-time legality for `PtrSafe`, `LongPtr`, Win64-only `LongLong`, VBA7/Win64 conditional declarations, x86 calling conventions, ordinal aliases and invalid `As Any`/ByRef shapes;
- lower verified external-call descriptors;
- resolve libraries, names, ordinals and aliases using an explicit secure DLL-search/loading policy that does not inherit unsafe current-directory behavior;
- implement x64 and x86 ABI/calling-convention selection;
- cover Sub/Function returns and all VBA7 scalar widths in the mandatory target ledger;
- support ByVal/ByRef and writeback;
- support String/BSTR/ANSI/Wide buffers;
- support fixed/dynamic arrays, UDTs and bounded `As Any`;
- capture and preserve `Err.LastDllError` at the exact post-call point before cleanup/helpers can overwrite it;
- clean pins/buffers on success and fault;
- emit stable diagnostics for invalid descriptors, missing DLL/entry and unsupported signatures;
- compare helper/libffi and direct-typed call paths where both exist.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| WIN-9.1 | scalar x64 Declare call | JIT/VM3/native fixture parity |
| WIN-9.2 | scalar x86 calling conventions | 32-bit runner evidence |
| WIN-9.3 | ByRef/return/error matrix | exact writeback and LastDllError |
| WIN-9.4 | string/buffer matrix | ANSI/Wide/BSTR pointer behavior |
| WIN-9.5 | array/UDT/As Any matrix | layout and cleanup evidence |
| WIN-9.6 | policy/missing-symbol diagnostics | stable compile/runtime outcomes |
| WIN-9.7 | Declare compile-time and loader-policy parity | x86/x64 Excel compile matrix plus secure resolution/ordinal cases |

Close condition: the full mandatory native-import matrix executes under both VM3 and JIT with VBA/native parity.

### WIN-10 — Pointer helpers and `AddressOf` callbacks

Type: delivery

Required outcomes:

- match compiler legality for `AddressOf` procedure eligibility, signature compatibility, visibility and invalid callback shapes;
- lower `VarPtr`, `StrPtr`, `ObjPtr` and `OxInst::Ptr` with exact addressability/lifetime;
- generate callback thunks for supported VBA procedure signatures;
- reenter the owning JIT session on the correct thread/apartment;
- support callback arguments, return values, ByRef and error propagation;
- enforce synchronous versus retained callback lifetime policy;
- implement a safe registration/release/session lifetime model for every mandatory retained-callback row; a rejection remains an open blocker until an approved scope split;
- support nested callbacks and callback-to-COM/native-to-VBA cycles;
- cover pointer-width and 32/64-bit behavior;
- add UAF, double-release and stale-thunk tests.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| WIN-10.1 | addressable scalar/string/array/object pointers | native read/write fixture |
| WIN-10.2 | synchronous scalar callback thunk | `CallWindowProc`-style VM3/JIT parity |
| WIN-10.3 | typed callback signature matrix | arguments/returns/ByRef |
| WIN-10.4 | nested reentry and fault behavior | stable Err and session reuse |
| WIN-10.5 | retained-callback policy | supported lifetime or deterministic rejection |
| WIN-10.6 | `AddressOf` compile-time parity | Excel diagnostic/token/line matrix for valid and invalid procedures |

Close condition: pointer/callback behavior is exact and lifecycle-safe for every mandatory signature/lifetime row.

### WIN-11 — Wrapped JIT DLL/EXE outputs

Type: delivery

Required outcomes:

- make `WrapperExe` and `WrapperLibrary` consume a verified embedded `OxImage`;
- compile/load the image through the JIT from a normal explicit/lazy initialization path; `DllGetClassObject` may trigger safe initialization for COM serving;
- keep `DllMain` minimal: no JIT compilation, COM initialization, loader recursion or blocking work under loader lock;
- expose deterministic wrapper entry points;
- make `WrappedComServer` select and retain the JIT backend;
- package helper/carrier ABI and target facts;
- preserve source/debug maps and diagnostics;
- implement reset/unload/`DllCanUnloadNow` behavior;
- eliminate source/compiler dependencies from deployed artifacts where intended;
- test clean-machine deployment.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| WIN-11.1 | JIT wrapper EXE | clean-process run with embedded image |
| WIN-11.2 | JIT wrapper library | native host invokes exported wrapper API |
| WIN-11.3 | JIT WrappedComServer | registration/activation/invoke/unload |
| WIN-11.4 | deployment manifest | clean VM/machine smoke and version rejection |
| WIN-11.5 | loader-lock-safe initialization | instrumentation proves `DllMain` is minimal and compile/COM work occurs after loader lock |

Close condition: wrapped outputs are usable standalone and are honestly labelled as runtime-backed wrappers.

### WIN-12 — Generic native DLL/EXE export and AOT packaging

Type: delivery

Required outcomes:

- define the project manifest/attribute policy that selects exported VBA procedures, eligible signatures, stable external names/ordinals and compatibility/version rules without inventing undocumented VBA source semantics;
- define a versioned external ABI: caller/callee ownership for BSTR/SAFEARRAY/interface/record values, ByRef lifetime, error return/HRESULT/IErrorInfo projection, concurrency, TLS and global/session state;
- lock the Cranelift object/blob and relocation format;
- generate native entry thunks from exported VBA signatures;
- emit PE/COFF code/data/relocations or a verified runtime-relocatable blob;
- construct exports by name/ordinal with x86 decoration rules where applicable;
- resolve helper imports through a fixed loader/import table;
- keep `DllMain` minimal: no JIT compilation, COM initialization, loader recursion or blocking work under loader lock; initialize runtime/package/session state through explicit or safe lazy entry points;
- map native calls into VBA values/errors without unwinding across FFI;
- define global state, concurrency, reset and unload semantics;
- emit debug/source-map metadata;
- support deterministic/reproducible builds;
- distinguish fixed-host wrapped export from genuine per-program native export.

First bead candidates:

| candidate | outcome | close evidence |
|---|---|---|
| WIN-12.1 | AOT object/blob format decision | executable prototype and reviewed spec |
| WIN-12.2 | scalar native DLL export | C/native client invokes exported VBA function |
| WIN-12.3 | typed signature export matrix | every scalar/String/array/UDT/ByRef row in the mandatory native-output ABI ledger |
| WIN-12.4 | native EXE entry | process exit/output/error semantics |
| WIN-12.5 | relocation/import/ASLR hardening | randomized-base and clean-machine tests |
| WIN-12.6 | reproducibility and debug maps | byte/hash/source lookup evidence |
| WIN-12.7 | native export selection and ABI contract | manifest/signature/ownership/error/version matrix plus C and Rust consumer probes |

Close condition: mandatory x64 and x86 native DLL/EXE targets are real generated native outputs, not renamed wrappers.

### WIN-13 — Security, safety and lifecycle hardening

Type: delivery/conformance

Required outcomes:

- fuzz/mutate COM/native descriptors before codegen;
- ASAN and fault-injection across marshalling, callbacks, events and unloading;
- QI/AddRef/Release and connection-point balance under loops;
- invalid pointer, invalid vtable, missing typelib/DLL and hostile server behavior;
- apartment/thread-affinity enforcement;
- reentrancy guard validation;
- registration and temporary-file cleanup;
- explicit host capability/policy profiles for activation, native load and callbacks;
- no process-wide dialog dismissal, registry damage or unrelated process termination.

Close condition: unsafe/native boundaries are robust against malformed metadata, hostile failures and repeated lifecycle stress.

### WIN-14 — Windows, native-client and Excel/VBA certification

Type: delivery/conformance

Required environments:

- an environment manifest pinning exact Windows edition/build and Office/Excel product/channel/build;
- Windows x64 with actual 64-bit Office/Excel;
- Windows x64 with actual 32-bit Office/Excel for Excel object/event rows; an equivalent owned x86 VBA oracle may substitute only for non-Excel compile/runtime rows;
- at least one non-default locale/code-page profile for LCID/string/date behavior;
- x64 and x86 native fixture runners;
- controlled in-proc and out-of-proc COM fixtures;
- installed Scripting/Office/Excel typelibs;
- optional DAO/Access lanes where dependencies are available;
- an explicit ARM64 status record.

Required evidence:

- compiler diagnostics for Declare/COM declarations and call legality;
- VM3/JIT side-by-side runtime outputs;
- Excel/VBA early and late client calls into external fixtures;
- Excel/VBA early and late calls into OxVba-served classes;
- Excel `WithEvents` against OxVba events and OxVba against Excel events;
- synchronous cancellable ByRef event;
- native C/Rust clients of native exports;
- registration/load/unload/restart cycles;
- in-proc/out-of-proc identity and error/lifecycle behavior;
- handle, interface, thread and process cleanup.
- backend, target, fixture binaries, registration view and artifact hashes for every run;
- promoted named integration runners rather than ignored-test-only evidence;
- archived UIA/VBE compile diagnostics and owned-process/registry cleanup manifests.

Excel oracle execution follows the repository modal-handling protocol: visible VBE compile, UI Automation scoped to owned PIDs, captured dialog/token/line evidence and PID-scoped cleanup.

An unavailable mandatory environment is a documented blocker with exact provisioning steps; it is not a skipped green row.

Close condition: every required matrix row has controlled fixture evidence and the exact real external and/or Excel/VBA authority specified by that row.

### WIN-15 — Terminal truth and release handoff

Type: support/conformance

Required outcomes:

- update architecture, COM/native/package/build docs and canonical matrices;
- reconcile old COM ladders, IP-08B state and blockers;
- generate derived capability reports;
- document wrapped versus native output labels;
- publish supported target/bitness/apartment/process matrix;
- run fresh-eyes code, security, registration, deployment and user-path review;
- leave no required follow-up only in chat or narrative evidence.

Close condition: all required delivery epics are closed and Windows capability claims match runnable artifacts.

## 8. Dependency graph

| epic | hard prerequisites | closure dependencies/notes |
|---|---|---|
| WIN-0 | workset acceptance, CORE-0 truth shape | establishes target ledger, matrices and fixtures |
| WIN-1 | CORE-3 typed facts, CORE-4 package metadata, CORE-5 carrier ABI | authoritative type resolver and exact Windows carriers gate every typed boundary |
| WIN-2 | CORE-4/5/6/7, WIN-1 | verified persistent Windows JIT session and shared VM/JIT interop plan |
| WIN-3 | WIN-1, WIN-2 | late transport may proceed in parallel with WIN-9 |
| WIN-4 | WIN-1 authoritative typelib resolver, WIN-2 | early transport must prove vtable rather than IDispatch |
| WIN-5 | WIN-1 event metadata, WIN-2 reentrant session, WIN-3 and applicable WIN-4 transports | includes typed event binding, custom sinks and synchronous writeback |
| WIN-6 | CORE-3/4 class-export metadata, WIN-1, WIN-2 | late serving plus identity/registration/local-server policy |
| WIN-7 | WIN-1, WIN-6 | early/dual serving also requires typelib and proxy/marshaling strategy |
| WIN-8 | WIN-5 and applicable WIN-6/7 serving surfaces | outgoing source interfaces/connection points |
| WIN-9 | CORE-3 Declare legality, WIN-1, WIN-2 | native import can proceed in parallel with COM client work |
| WIN-10 | WIN-2, WIN-9 | pointers/callbacks require native ABI and reentrant session ownership |
| WIN-11 | CORE-4/7, WIN-2 | wrapper EXE/library use session substrate; `WrappedComServer` additionally requires WIN-6/7/8 |
| WIN-12 | CORE-4/5/7, WIN-1/2, relevant WIN-11 loader work | native export has its own versioned external ABI gate |
| WIN-13 | WIN-1 initially | safety harness is continuous and closes only after WIN-3 through WIN-12 |
| WIN-14 | every applicable WIN-1 through WIN-13 delivery row | certification cannot skip an unavailable mandatory environment |
| WIN-15 | WIN-14 and both internal claim gates | terminal truth after all delivery residuals close |

Late COM client and scalar Declare are the first useful parallel lanes after WIN-1/2. Fixture expansion and safety work run continuously. No downstream epic may substitute a narrative dependency for the exact consumed core/WIN matrix rows.

## 9. Required checks

### Per delivery bead

- targeted VM3 and JIT tests;
- exact transport/calling-convention proof;
- handle/interface/pin balance;
- fault and cleanup neighbor;
- relevant matrix row update;
- fresh-eyes review.

### Windows merge gate

- format and strict workspace Clippy;
- workspace tests on Windows;
- x64 VM3/JIT COM/native controlled fixtures;
- default-parallel and single-thread stress;
- governance/meta checks;
- no stale ignored row in the touched matrix.

### Milestone/release gate

- x86 and x64 matrices;
- live Excel/VBA oracle;
- in-proc/out-of-proc COM;
- ASAN/fault injection;
- registration/load/unload clean-machine smoke;
- wrapped and native artifact inspection;
- performance and size;
- current docs/matrices.

## 10. Terminal condition

This workset is complete only when:

1. every required `WIN-*` delivery epic is closed;
2. the JIT no longer declines any COM/native image required by the mandatory VBA7 Windows target ledger;
3. late-bound and early-bound COM client matrices are parity-complete under both VM3 and JIT;
4. COM events under both VM3 and JIT include synchronous ByRef writeback and declared apartment/process behavior;
5. both VM3- and JIT-backed OxVba classes are served to late- and early-bound clients, including outgoing events;
6. Windows Declare, pointer helpers and callbacks are complete under both VM3 and JIT for the mandatory VBA7 signature matrix;
7. wrapped DLL/EXE/COM-server outputs run through the JIT without fallback;
8. mandatory x64/x86 native DLL/EXE export targets are real and pass native-client tests;
9. x86/x64, real COM and Excel/VBA evidence is green;
10. carrier/interface/pin/session/registration lifecycle gates are clean;
11. the VBA7 Windows compatibility gate and standalone native-output extension gate are each independently green;
12. docs, matrices, build outputs and tests agree.

A fixture-only route, portable projection, VM3-only path, ignored test or documentation audit is not closure evidence.

## 11. Bead-preparation handoff

After acceptance:

1. create the workset root and epics `WIN-0` through `WIN-15`;
2. create a rollout bead for each epic;
3. materialize the listed first delivery beads;
4. serialize all registry/fixture mutation beads;
5. attach matrix rows, target bitness and fixture prerequisites to every validation bead;
6. mark delivery versus support explicitly;
7. record unavailable environments as blockers with exact provisioning steps, while continuing Linux-safe/Windows-safe fixture work;
8. never close a capability epic on matrix/doc/fixture rollout alone.

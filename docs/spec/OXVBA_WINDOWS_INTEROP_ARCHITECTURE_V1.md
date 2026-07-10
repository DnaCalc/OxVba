# OxVba Windows Interop Architecture V1

Date: 2026-07-10
Status: current destination architecture
System clauses: `PROFILE-WIN-001`, `WIN-*`, `COM-*`, `NATIVE-IMPORT-001`, `BUILD-NATIVE-001`
Supersedes: older COM/HAL/native planning documents where they conflict with this architecture

## 1. Target state

Windows COM and native interop use compiler/package descriptors, exact runtime carriers and one verified backend-neutral interop plan. VM3 and JIT execute adapters for the same plan, producing the same VBA values, Err state, writeback, side effects and lifecycle behavior.

`oxvba-com` owns COM metadata, activation, invocation, events, serving and wire conversion. Native Declare/callback execution uses a dedicated Windows ABI service under host policy. `oxvba-hal` owns capability/policy and delegation, not COM or ABI semantics.

## 2. Supported environment

The required compatibility environment is supported 64-bit Windows with x64 OxVba processes, fixtures and output artifacts. Excel-specific evidence uses actual 64-bit Excel in an owned environment. Native output targets x64 Windows.

x86/32-bit Office, WOW64, ARM64 and other Windows architectures are outside the accepted profile. They have no active delivery workset and no implied support. Non-Windows targets may parse and bind Windows declarations and consume synthetic metadata, but portable projections are not native COM evidence.

## 3. Authoritative metadata and references

One Windows reference resolver handles registered and file-backed typelibs with explicit:

- library GUID, version, LCID and platform selection;
- registry view and file provenance;
- reference order, aliases and ambiguity;
- coclasses, interfaces, inheritance and QueryInterface relations;
- methods, properties, DISPIDs and vtable slots;
- default/source interfaces and events;
- enums, records, aliases, arrays and wire shapes;
- broken/unavailable reference diagnostics;
- stable identities and resolver revision/digest.

Compiler, OxImage, runtime, build and language-service consumers use this authoritative metadata. Fixture catalogs may describe repo-owned fixtures but do not replace general discovery.

## 4. Exact carriers and layouts

COM and native boundaries operate on canonical BSTR, VARIANT, SAFEARRAY, object/interface and numeric carriers. Nominal interface/object arrays preserve VT_DISPATCH/VT_UNKNOWN and interface identity. Records carry GUID/typeinfo, size, alignment, field layout and VT_RECORD ownership information.

x64 layout probes and controlled native fixtures prove every boundary shape. Copy, clear, clone, preserve, ByRef, writeback and error cleanup rules are shared with the runtime rather than reimplemented per transport.

## 5. Verified interop call plan

Compiler and package descriptors elaborate into a plan containing:

- call kind and exact callable/wire signature;
- target interface/member/entry identity;
- argument order, named/omitted/Optional/ParamArray rules;
- input/output/inout and ByRef storage;
- marshalling temporaries, pins and ownership;
- success, VBA error, HRESULT/native error and cleanup edges;
- synchronous writeback order;
- reentrancy, apartment and callback policy;
- source/diagnostic provenance.

VM3 interprets the plan; the JIT lowers or invokes a typed plan adapter. Transport counters and structural results prove that early calls remain vtable-bound and both backends use equivalent semantics.

## 6. Late-bound COM client

Late binding supports ProgID/CLSID activation, host-returned objects, ROT/moniker/file GetObject, IDispatch name/DISPID resolution, methods, properties, default members, put/putref, statement/function context, named/omitted/Optional arguments, ByRef, chaining and enumeration.

DISPPARAMS ordering, property-put DISPIDs, LCID, VARIANT conversion, object identity and cleanup match Automation rules. HRESULT, EXCEPINFO and IErrorInfo map to exact VBA Err behavior.

Activation policy covers CLSCTX, registry views, in-proc/out-of-proc servers and host capability denial without hardcoded dependency routes.

## 7. Early-bound COM client

Early binding calls the verified native vtable slot with the Windows system ABI and typed descriptor. Automation dual and custom interfaces support scalar, BSTR, VARIANT, SAFEARRAY, interface pointer, record, HRESULT/out-retval and ByRef shapes.

Interface inheritance, QueryInterface identity and ownership are explicit. An early-bound call does not fall back to IDispatch unless real VBA/COM behavior for that exact row requires it.

In-process and proxy/out-of-process evidence demonstrates both local and marshaled behavior.

## 8. Incoming COM events

Typed `WithEvents` binding selects the authoritative source interface and validates handler names/signatures at compile time. Runtime subscription discovers connection points, builds the required IDispatch or custom-vtable sink and manages Advise/Unadvise lifetime.

Event delivery preserves ordering, fan-out, replacement, object identity, handler errors and class termination. VT_BYREF arguments are invoked synchronously and copied back before the source Invoke returns. Mandatory apartment/process rows implement safe reentry rather than silently queuing snapshots or dropping writeback.

## 9. COM serving

Exported OxVba classes expose stable CLSID/ProgID/version identity, class factories, IUnknown/IDispatch/type information, methods, property groups, default members, enumeration, Implements and generated early/dual vtables.

One object identity/state backs early and late interfaces. Errors map to HRESULT/IErrorInfo; arrays, records, interfaces and ByRef use the exact carrier plan. Class Initialize/Terminate and unload behavior match the owning project session.

Registration records InprocServer32/LocalServer32, threading model and the x64 registry view. Out-of-process/custom interfaces use Automation marshaling or generated proxy/stub artifacts according to their signatures. Local servers define message-loop, lock, class-object and shutdown behavior.

## 10. Outgoing COM events

Served classes expose source interfaces and connection-point containers. OxVba RaiseEvent invokes subscribed native sinks with typed marshalling, ordering, fan-out, errors, ByRef behavior and deterministic teardown.

Both VM3- and JIT-backed served sessions implement the same source-interface plan and are validated from native and Excel/VBA clients.

## 11. Declare and pointer helpers

The compiler validates VBA7/Win64 conditional declarations, PtrSafe, LongPtr, LongLong legality, the x64 convention, aliases/ordinals, ByVal/ByRef, As Any and AddressOf eligibility. Those compile facts live in the compiler contract.

Runtime import resolves DLLs and entries under an explicit secure search policy. Calls implement scalar, string/buffer, array, UDT, pointer and ByRef behavior for x64. LastDllError is captured immediately after the native call before cleanup can overwrite it.

VarPtr, StrPtr and ObjPtr expose exact addressable storage with bounded lifetime. AddressOf thunks reenter the owning project session, implement typed arguments/results/ByRef and have explicit synchronous or retained registration/release lifetimes. Stale thunks cannot call a disposed session.

## 12. Wrapped and native outputs

WrapperExe, WrapperLibrary and WrappedComServer consume a verified embedded/deployed OxImage and select VM3 or JIT honestly. DLL initialization performs no JIT, COM initialization or blocking work under loader lock; normal exports or DllGetClassObject trigger safe lazy initialization.

Native DLL/EXE outputs use program-specific generated code and an explicit project export manifest. Their versioned external ABI defines eligible signatures, names/ordinals, caller/callee ownership, errors, concurrency, TLS/global/session state and initialization.

Wrapped and native artifacts remain distinct in manifests, documentation and evidence.

## 13. Safety and evidence

All COM/native descriptors are verified before code generation or invocation. Unsafe boundaries contain panics/unwinds, balance QI/AddRef/Release, free pins/temporaries on every edge and enforce apartment/thread affinity.

Completion evidence includes:

- x64 controlled fixtures and artifacts;
- actual 64-bit Excel/VBA;
- in-proc and out-of-proc COM;
- transport counters and exact wire signatures;
- synchronous cancellable ByRef events;
- native C/Rust clients of exports;
- registration/load/unload/restart cycles;
- non-default locale/LCID rows;
- ASAN/fault/hostile-server and lifecycle balance;
- environment, fixture, artifact and evidence hashes.

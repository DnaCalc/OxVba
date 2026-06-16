# Workset: Wrapped COM Server Interface/Event/UDF Execution

Date: 2026-05-09
Owner: Codex
Status: in-progress
Bead root: `bd-wcs1`

## Purpose

Turn the current COM-shaped internal object floor into an executable, publishable
Windows desktop in-process COM server lane while preserving the long-term path to
`NativeComServer`.

This workset deliberately uses the term **WrappedComServer** for the near-term
artifact: a generated in-process COM DLL that wraps a canonical `.oxb` bundle and
executes OxVba code through the existing host/runtime/VM/JIT engine. A future
**NativeComServer** can reuse the same descriptors, typelibs, DISPIDs, IIDs,
connection-point metadata, and validation corpus while replacing wrapped engine
calls with direct native compiled entry points.

## Current truth

OxVba already has important substrate:

- 2026-06-17 clean reimplementation update: `oxvba-build` now emits a usable
  dispatch-backed `WrappedComServer` DLL for `OutputType=ComServer` projects,
  including per-user registration, generated TypeLib registration, late-bound
  `IDispatch` activation/member dispatch, and connection-point event
  publication. Current evidence includes a controlled raw COM sink and an
  Excel/VBA typed dispatch-interface `WithEvents` sink; this does not claim
  dual-interface vtable calls.
- Runtime values are OLE Automation shaped (`BStr`, 16-byte `Variant`, retained
  SAFEARRAY metadata, exact scalar COM VARTYPE carriers).
- `ObjectRef` is IUnknown-like and now carries descriptor-backed
  `RuntimeClassDescriptor`, `RuntimeInterfaceDescriptor`, and
  `RuntimeMemberDescriptor` metadata.
- Runtime descriptor-backed `QueryInterface` can advertise `IDispatch` and raw
  QI projection is covered by tests.
- Pure OxVba project objects now use descriptor-backed object identities and a
  descriptor-backed late-bound dispatch plan cache.
- Compiler bytecode carries typed project-member call metadata and imported COM
  early-bound member metadata.
- `Engine::create_class_instance` and
  `Engine::invoke_member_on_object_with_variants` exist as wrapper-facing hooks.
- `RuntimeCallFrame`/`RuntimeCallResult` are live, pure OxVba object member
  invocation routes through them, and Windows COM `DISPPARAMS`/`VARIANT`
  marshaling helpers now lower into and out of the same call-frame carrier.
- `OxBundle` v3 now persists a descriptor inventory for COM classes,
  interfaces, members, events, and host-callable procedures while preserving
  v1/v2 bundle read compatibility.
- `.basproj` parsing, canonical generation, and direct host project-settings
  DTOs now represent `BuildTarget=WrappedComServer`; the compatibility input
  spelling `WrapperComServer` normalizes to the canonical value.
- `oxvba-build`/CLI can compile the bounded Windows WrappedComServer DLL
  skeleton and verify the standard COM export names; `IDispatch` behavior is
  still intentionally not claimed.
- Generated `IClassFactory::CreateInstance` now creates wrapped OxVba class
  instances through `Engine::create_class_instance`; controlled Windows client
  coverage exercises `DllGetClassObject`, `CreateInstance`, `LockServer`, and
  `DllCanUnloadNow` without claiming `Invoke` success yet.
- Generated `IDispatch::GetIDsOfNames` and `Invoke` now route through the
  emitted member descriptor table and COM call-frame marshaling helpers for the
  first scalar method slice; object-return, array-return, and richer error
  evidence remain in the later late-bound validation bead.
- COM-0007 now has controlled Windows client evidence for emitted DLL exports,
  `LoadLibraryW`, `DllGetClassObject`, `IClassFactory::CreateInstance`,
  `LockServer`, `DllCanUnloadNow`, `GetIDsOfNames`, method `Invoke`, property
  get/let through default DISPID `0`, project object return as `VT_DISPATCH`,
  SAFEARRAY/array return, supported error/`EXCEPINFO` behavior,
  `DllRegisterServer`/`DllUnregisterServer` per-user registration,
  `CLSIDFromProgID`, and `CoCreateInstance` registered activation.
- COM-0008 now has generated TypeLib evidence for the publication half:
  `compile_wrapped_com_server_shim` emits a sibling `.tlb`, the `.tlb` loads
  through Windows TypeLib roundtrip checks, and `DllRegisterServer` writes
  per-user TypeLib metadata linked from each CLSID.
- COM-0008 now also has controlled TypeLib-aware client-call evidence:
  `wrapped_com_server_build_compiles_dll_with_standard_exports` loads the
  generated TypeLib, resolves wrapped member DISPIDs through `IWidget`
  `ITypeInfo`, and uses those TypeLib-derived DISPIDs to call scalar methods,
  default property get/let, object return, array return, error/`EXCEPINFO`, and
  registered `CoCreateInstance` activation slices. Office/VBA project-reference
  evidence remains outside the implemented subset.
- COM-0009 has historical pre-clean dual-interface projection evidence, but the
  clean 2026-06-17 `WrappedComServer` reimplementation does not carry that
  vtable tier. The active clean path emits dispatch-only default interfaces and
  returns `IDispatch` for those interface IIDs until a real dual-interface
  vtable implementation lands. Broader argument/property/byref/object/array/error
  vtable parity remains outside the implemented subset.
- COM-0010 now has source-dispinterface metadata evidence: wrapped server
  TypeLib generation consumes `descriptor_inventory.com_events`, emits
  deterministic `_<ClassName>Events` source dispinterfaces with stable event
  DISPIDs, and links them from coclasses as default source implemented types.
  It also has controlled runtime connection-point evidence for
  `IConnectionPointContainer::FindConnectionPoint`, `IConnectionPoint::Advise`,
  sink `IDispatch::Invoke` payload delivery from `Widget.FireChanged(123)`,
  `Unadvise`, and no callback after unsubscribe. The clean 2026-06-17
  `WrappedComServer` smoke additionally proves Excel/VBA `WithEvents`
  subscription against the generated TypeLib/source interface with typed
  dispatch-interface method invocation.
- PH-0011 now has the first descriptor metadata slice: host-call descriptors in
  `descriptor_inventory.host_calls` carry stable identities, entry/slot/type
  metadata, argument-name slots, and conservative UDF policy fields for
  selection, volatility, dependencies, side effects, thread safety, and allowed
  host contexts. It also has a first typed host API slice:
  `Engine::host_udf_catalog` enumerates public procedural functions only and
  `Engine::invoke_host_udf_with_variants` invokes a stable catalog entry through
  the prepared-session VM path with caller, locale, dependency-token, and
  volatile-request context shape. Broader array/error return and host harness
  evidence remains pending.
- OxIde/direct-host build DTOs now have a WrappedComServer planning/result
  slice: `EmbeddedBuildRequest` carries `EmbeddedBuildTarget::WrappedComServer`,
  `EmbeddedBuildRunHost::build_plan` returns planned `.oxb`/`.dll`/`.tlb` and
  registration artifacts, required tools, capability profile, and per-user
  registration plan, and `EmbeddedBuildResult` exposes direct `dll_path`,
  `tlb_path`, and `registration_plan` fields.
- 2026-05-10 direct-host correction closure: `EmbeddedBuildRunHost::build_workspace`
  now executes the WrappedComServer build lane for Windows `DiskOnly` requests,
  writes the planned `.oxb` and registration-plan artifacts, invokes the wrapper
  build command, and verifies required `.dll`/`.tlb`/plan artifacts before
  returning success. Non-Windows, non-`DiskOnly`, build-command failure, and
  missing-artifact paths now return typed failed build diagnostics.
- 2026-05-10 sequencing note: deeper host-program and UDF semantics are moved
  into `docs/worksets/WORKSET_2026-05-10_HOST_PROGRAM_DESIGN_AND_UDF_REWORK.md`
  (`bd-sg5h`) for execution after the next WrappedComServer workset.
- Final implemented-subset terminal audit is published at
  `docs/evidence/conformance/WRAPPED_COM_SERVER_TERMINAL_AUDIT_2026-05-09.md`.
  It confirms the terminal checks passed and keeps the remaining Office/VBA,
  broader dual-interface, richer event, and richer host-UDF gaps explicit in
  the validation rows.
- `OutputType=ComServer` and creatable class metadata exist in `.basproj` and
  project validation.
- `crates/oxvba-build/src/comserver.rs` emits a COM DLL skeleton with
  `DllGetClassObject`, `IClassFactory`, `IDispatch`, and registration exports.

The missing truth is also explicit:

- The clean `oxvba build --target WrappedComServer` lane now registers a usable
  in-process COM server DLL for the bounded dispatch-backed subset.
- Office/VBA early-bound project-reference evidence is not part of COM-0007 and
  remains deferred beyond the current COM-0008 controlled TypeLib-aware subset.
- Broader dual-interface argument/property/byref/object/array/error parity and
  Office/VBA early-bound/vtable event-client parity are not yet implemented.
- Host worksheet-UDF invocation for DnaOneCalc/OxIde-style hosts needs to share
  the same call descriptor/call-frame core but should not be conflated with
  Automation Add-Ins.
- Direct-host WrappedComServer execution currently requires
  `EmbeddedExecutionSourcePolicy::DiskOnly` and does not perform registration
  side effects itself.

## Governing vocabulary and build model

### Semantic shape versus packaging shape

Keep the current build-boundary doctrine:

- `OutputType` is semantic project shape.
- `BuildTarget` is physical artifact packaging.
- `.oxb` remains the canonical compiled semantic artifact.

Recommended near-term project shape:

```xml
<PropertyGroup>
  <OutputType>ComServer</OutputType>
  <BuildTarget>WrappedComServer</BuildTarget>
  <ProjectName>MyProject</ProjectName>
  <RuntimeFlavor>Jit</RuntimeFlavor>
</PropertyGroup>
```

Implementation note: because existing build targets use `WrapperExe` and
`WrapperLibrary`, the parser may accept `WrapperComServer` as a compatibility
alias, but the canonical UI-facing term for this workset is `WrappedComServer`.

### WrappedComServer

A generated Windows `cdylib`/DLL that contains or side-loads the `.oxb`, exports
standard COM server entry points, registers CLSIDs/ProgIDs/typelib metadata, and
runs OxVba objects through the existing engine. It is the first delivery target.

### NativeComServer

A future target that emits native code for object/member bodies and COM stubs.
It is not in scope here except as a design constraint: descriptors, typelibs,
IIDs, DISPIDs, event source metadata, and validation evidence must be reusable.

## External research notes: Excel native VBA UDF clues

Public Microsoft documentation does not expose Excel's private worksheet formula
engine to VBA-project ABI. The following published clues shape our host-UDF
model:

1. Excel's multithreaded recalculation documentation states that Excel uses the
   main thread for VBA user-defined functions and explicitly lists VBA UDFs as
   thread-unsafe. XLL functions can be registered as thread-safe; VBA UDFs
   cannot be treated that way by default.
   Source: <https://learn.microsoft.com/en-us/office/client-developer/excel/multithreaded-recalculation-in-excel>
2. Excel performance guidance also treats VBA and COM add-in UDFs as
   single-threaded functions. Source:
   <https://learn.microsoft.com/en-us/office/vba/excel/concepts/excel-performance/excel-improving-calculation-performance>
3. `Application.Volatile` marks a worksheet UDF as volatile and only has effect
   inside a UDF used to calculate a worksheet cell. Source:
   <https://learn.microsoft.com/en-us/office/vba/api/excel.application.volatile>
4. `Application.Caller` reports how Visual Basic was called; for a custom
   function in one cell it returns that cell as a `Range`, and for an array
   formula it returns the target range. Source:
   <https://learn.microsoft.com/en-us/office/vba/api/excel.application.caller>
5. `Application.ThisCell` returns the cell from which the UDF is being called
   and warns not to access Range properties/methods inside the UDF; cache it for
   later work after recalculation. Source:
   <https://learn.microsoft.com/en-us/office/vba/api/excel.application.thiscell>
6. `Application.MacroOptions` is the VBA-facing metadata route for descriptions,
   categories, and argument descriptions in the Insert Function dialog. Source:
   <https://learn.microsoft.com/en-us/office/vba/api/excel.application.macrooptions>
7. The Excel C API `xlUDF` can call VBA UDFs, XLM macro functions, and
   registered add-in functions, implying an internal callable UDF registry, but
   not exposing the worksheet formula engine's private invocation ABI. Source:
   <https://learn.microsoft.com/en-us/office/client-developer/excel/xludf>
8. Microsoft Support documents that UDFs should receive all calculation ranges
   as arguments; otherwise Excel may not account for dependencies correctly.
   Source:
   <https://support.microsoft.com/en-us/topic/description-of-limitations-of-custom-functions-in-excel-f2f0ce5d-8ea5-6ce7-fddc-79d36192b7a1>

Design conclusion: DnaOneCalc/OxIde host UDF support should be a first-class
**host callable function** adapter over OxVba descriptors and call frames, not a
COM Automation Add-In clone. The first host-UDF tier should be single-threaded,
context-aware, explicit about volatile/dependency behavior, and conservative
about side effects.

## Architecture review and best execution path

The best push is not to start by making `comserver.rs` smarter in isolation.
First add a shared **COM projection and host-call core** so the wrapper DLL,
early-bound clients, dual-interface stubs, events, and host UDF calls all use one
truth source.

### A. Runtime COM projection core

Add a runtime module such as `crates/oxvba-runtime/src/com_projection.rs` or
expand `object_ref.rs` with a stable projection model.

Required concepts:

```text
RuntimeGuid
  - data1/data2/data3/data4 representation compatible with COM GUIDs
  - deterministic generation helpers live outside runtime if needed

RuntimeInterfaceIdentity
  - well_known: IUnknown, IDispatch, IConnectionPointContainer, IConnectionPoint, none
  - guid: Option<RuntimeGuid>
  - name
  - kind: Unknown | Dispatch | Dual | DispInterface | EventSource | HostCallable
  - version / lcid fields where relevant to typelib projection

RuntimeObjectIdentity
  - stable object id
  - class descriptor pointer
  - lifetime/refcount policy
  - apartment/thread affinity metadata
  - runtime/session object handle

RuntimeInterfaceProjection
  - object identity
  - requested interface identity
  - interface descriptor
  - optional native vtable projection handle
```

`ObjectRef` can remain the public value carrier, but `QueryInterface` should be
able to return an explicit projection descriptor rather than only `Option<&'static
RuntimeInterfaceDescriptor>`. This makes `IUnknown`, `IDispatch`, `IMyInterface`,
and event connection-point projections all share one object identity.

### B. Canonical member and call descriptors

Promote current member metadata into a call model that is rich enough for every
call path:

```text
RuntimeMemberDescriptor
  - stable name
  - stable DISPID
  - optional vtable slot
  - invoke kind: method/property-get/property-let/property-set/event
  - visibility/export flags
  - default/indexed/NewEnum flags
  - params: name, Automation VARTYPE, internal value type, byref, optional,
    default value, ParamArray, LCID/retval flags
  - return type: Automation + internal value type
  - target: project procedure token / runtime thunk id / host callable id

RuntimeCallFrame
  - receiver object/projection
  - member descriptor or DISPID/vtable slot selector
  - positional args, named args, property put value
  - call kind
  - locale/caller/context metadata
  - byref writeback slots
```

The call frame is the hub. All of these should lower into it:

- internal typed pure OxVba project call,
- internal late-bound project object call,
- external `IDispatch::Invoke`,
- external dual-interface vtable thunk,
- event sink dispatch,
- host worksheet-UDF call.

### C. Descriptor persistence in `.oxb`

`OxBundle` should carry enough descriptor metadata to generate and consume COM
adapters without reparsing source:

- exported coclasses and class descriptors,
- default dispatch interface descriptors,
- dual interface descriptors,
- event source dispinterfaces,
- stable CLSIDs/IIDs/TypeLibID/progids,
- member DISPIDs/vtable slots,
- host-callable module-function descriptors.

The serializer should have compatibility tests so future NativeComServer work can
rely on stable metadata.

### D. COM marshaling policy layer

Place COM adapter marshaling in `oxvba-com`, not in generated source:

- `VARIANT`/`DISPPARAMS` -> `RuntimeCallFrame` arguments,
- `RuntimeCallResult` -> `VARIANT`/`EXCEPINFO`,
- COM error mapping through `Err`, `HRESULT`, `IErrorInfo`, `EXCEPINFO`,
- byref writeback,
- object identity/projection retention,
- SAFEARRAY and unsupported Automation type policy.

Generated COM server source should call stable helper APIs rather than carrying a
large hand-maintained marshaler copy.

### E. WrappedComServer generated DLL

The generated DLL should be thin:

- load embedded/sidecar `.oxb`,
- bootstrap a host engine/session with configured runtime flavor,
- maintain object identity table and reference counts,
- implement COM entry points and vtables,
- delegate lookup/invoke/marshaling to stable crates.

Required exports:

- `DllMain`
- `DllGetClassObject`
- `DllCanUnloadNow`
- `DllRegisterServer`
- `DllUnregisterServer`

Required COM interfaces for first usable tier:

- `IUnknown`
- `IClassFactory`
- `IDispatch`
- `ISupportErrorInfo` where supported

Registration should write CurrentUser or machine registry entries depending on
policy, but tests should prefer per-user/test-scope registration where possible.

### F. Typelib and early binding

Generate a `.tlb` from the same descriptors. First tier may execute through
`IDispatch::Invoke`; the point is that the client compiler can bind names,
interfaces, DISPIDs, and event source metadata early.

Type library content:

- library ID/version/name,
- coclass for each exported creatable class,
- default interface,
- default source dispinterface when events exist,
- public methods/properties with DISPIDs and Automation-compatible signatures,
- default member and NewEnum metadata,
- hidden/restricted flags where appropriate.

### G. Dual-interface vtable tier

After dispatch-backed early binding works, add dual vtable stubs. Scope the first
vtable tier tightly:

- Automation-safe scalars: Boolean, Byte, Integer, Long, LongLong, Single,
  Double, Currency, Date, Decimal, BSTR/String, Variant,
- COM object refs,
- SAFEARRAY(VARIANT) and explicitly supported typed SAFEARRAYs,
- `HRESULT` plus `[out, retval]` return convention,
- byref writeback for supported scalar/Variant refs.

Keep arbitrary UDT/record/native struct signatures out of the first vtable tier.
If surfaced through Automation as `VT_RECORD`, preserve the current Excel/VBA
observable error policy until an oracle-backed expansion exists.

### H. COM events and connection points

Promote events into descriptor-backed source interfaces:

```text
RuntimeEventSourceDescriptor
  - source IID
  - source dispinterface name, e.g. _WidgetEvents
  - event members with DISPIDs and parameter descriptors
  - sink dispatch policy
```

Runtime object identities that publish events own a connection table:

```text
ConnectionPointTable
  - source IID
  - cookie -> sink IDispatch/projection
```

Implement:

- `IConnectionPointContainer::EnumConnectionPoints`
- `IConnectionPointContainer::FindConnectionPoint`
- `IConnectionPoint::GetConnectionInterface`
- `IConnectionPoint::Advise`
- `IConnectionPoint::Unadvise`
- bounded `EnumConnections` behavior or explicit unsupported surface
- `RaiseEvent` -> sink `IDispatch::Invoke(event_dispid, ...)`

Microsoft's COM connection point docs define this as the standard connectable
object/event mechanism for outgoing interfaces and sink connect/disconnect.
Source:
<https://learn.microsoft.com/en-us/windows/win32/api/ocidl/nn-ocidl-iconnectionpointcontainer>

### I. Host UDF bridge for DnaOneCalc/OxIde-style hosts

Add host-callable descriptors for public module functions and any future
host-exposed class/static surfaces:

```text
RuntimeHostCallableDescriptor
  - project/module/function identity
  - procedure token
  - argument descriptors
  - return descriptor
  - volatility policy
  - dependency policy
  - allowed call contexts: worksheet formula, macro command, host command
  - side-effect policy
  - thread-safety policy
  - metadata: category, description, argument descriptions
```

Add `HostUdfCallContext`:

```text
HostUdfCallContext
  - caller cell/range identity
  - workbook/sheet identity
  - calculation pass id
  - volatile flag sink
  - dependency registration sink
  - host object projection for Application/Caller/ThisCell equivalents
```

Execution path:

```text
formula =MyFunc(A1:B2)
  -> host resolves RuntimeHostCallableDescriptor
  -> host builds RuntimeCallFrame with HostUdfCallContext
  -> engine runs VM/JIT member/procedure thunk
  -> result maps to scalar/array/error cell value
  -> volatile/dependency side effects are returned to host scheduler
```

Initial policy should match Excel's conservative VBA UDF behavior:

- single-threaded unless a future explicit safe tier is proven,
- all range dependencies should be explicit arguments or host-registered
  dependencies,
- volatile marking is explicit,
- side-effecting worksheet/environment changes are rejected or deferred.

## OxIde and DnaOneCalc integration review

### Project/build settings surface

Expose in host/project DTOs:

- semantic `OutputType=ComServer`,
- packaging `BuildTarget=WrappedComServer`,
- runtime flavor `Lite`/`Jit`,
- ProgID/CLSID/TypeLibID/version metadata,
- per-class creatability and instancing,
- registration scope: none/per-user/machine/manifest-only,
- bitness target and Windows-only availability,
- output artifacts: DLL, TLB, REG/manifest, PDB/logs,
- command availability with typed disabled reasons on non-Windows or missing toolchain.

### OxIde UI contract

OxIde should not shell out blindly or parse CLI text for core truth. OxVba should
provide typed planning DTOs and execution results:

- `BuildPlan { target: WrappedComServer, artifacts, required_tools, warnings }`
- `BuildResult { dll_path, tlb_path, registration_plan, diagnostics }`
- `RegistrationPlan { scope, clsids, progids, registry_keys, requires_admin }`
- `ComServerCapabilityProfile { windows, bitness, toolchain, registration }`
- `HostUdfCatalog { functions, signatures, categories, volatility }`

CLI can remain a consumer of the same services.

### Host UDF catalog surface

DnaOneCalc and OxIde need a catalog API independent of COM registration:

- list host-callable functions,
- resolve function names case-insensitively with project/module scoping,
- expose signatures and argument help,
- invoke with a host call context,
- return scalar/array/error values plus volatile/dependency side-effect records.

## Execution phases

### Phase WCS-0: workset initiation and design lock

Close when this workset, matrix rows, and bead tree exist and the naming/build
boundary is accepted.

### Phase WCS-1: internal COM projection core

Close when GUID-capable interface identity, explicit interface projection, and
object identity/projection tests land without changing observable internal object
behavior.

### Phase WCS-2: canonical call descriptors and call frames

Close when project member descriptors can produce a runtime call frame and the
existing pure OxVba typed/dynamic paths can be exercised through that core in a
bounded slice.

### Phase WCS-3: bundle descriptor persistence

Close when `.oxb` carries exported COM/host-call descriptors and round-trips them
through serialization tests.

### Phase WCS-4: wrapped late-bound COM server DLL

Close when `oxvba build` can emit a Windows in-process COM DLL for one creatable
class and an external client can `CreateObject` and late-bound invoke a method,
property get/let/set, default member, and object return.

### Phase WCS-5: type library and dispatch-backed early binding

Close when the generated typelib is registered/loaded by a real Office/VBA or
controlled COM client and early-bound member calls route through the wrapped
server by stable DISPIDs.

### Phase WCS-6: dual-interface vtable tier

Close when an Automation-safe interface can be called through `QueryInterface` +
vtable stubs and results match the dispatch path for the same members.

### Phase WCS-7: connection-point events

Close when a client can sink events from a wrapped OxVba class, `RaiseEvent`
reaches the sink, and VBA-style `WithEvents` coverage exists where practical.

### Phase WCS-8: host UDF bridge and OxIde/DnaOneCalc catalog

Close when public module functions are discoverable and callable through a typed
host UDF catalog with caller context, volatile marking, argument/result mapping,
and conservative threading/side-effect policy.

### Phase WCS-9: validation, docs, and conformance evidence

Close when matrix rows, specs, OxIde guidance, CLI docs, and evidence artifacts
are synchronized and no capability claim relies on skeleton/generated-source-only
proof.

## Test strategy

### Unit and crate tests

- `oxvba-runtime`:
  - GUID/interface identity equality and normalization,
  - object identity/projection lifecycle,
  - descriptor-backed `QueryInterface` for custom IIDs,
  - dispatch/default/member lookup by DISPID and vtable slot,
  - event source descriptor and connection table behavior.
- `oxvba-compiler`:
  - class/interface/event/host-call descriptors generated from source,
  - stable DISPID/IID generation,
  - default member/NewEnum/event metadata preservation,
  - call-frame descriptor coverage for method/property/default shapes.
- `oxvba-com`:
  - `DISPPARAMS` -> call frame mapping,
  - `RuntimeCallResult` -> `VARIANT`/`EXCEPINFO`,
  - byref writeback,
  - object projection retention,
  - event sink invocation payloads.
- `oxvba-build`:
  - `BuildTarget=WrappedComServer` parse/generate,
  - generated source compiles on Windows,
  - export list contains COM entry points,
  - generated IDL/TLB content matches descriptors.
- `oxvba-host`:
  - `Engine` creates class instances and invokes members through call frames,
  - host UDF catalog/invoke paths,
  - VM/JIT parity for wrapped call paths.

### Integration tests

- Controlled Rust/Win32 COM client loads the DLL with test registration.
- VBA/Excel oracle workbook references the generated TLB and calls early-bound
  methods/properties.
- VBA `WithEvents` or controlled sink receives wrapped class events.
- Host UDF harness simulates DnaOneCalc formula calls with caller context,
  volatile marking, explicit dependencies, array returns, and error returns.

### Evidence gates

A row is not complete until it has:

- source-level tests,
- generated-artifact compile/build evidence where applicable,
- Windows runtime execution evidence for COM rows,
- Office/VBA oracle evidence for early-bound/events when the claim names Excel/VBA,
- docs and matrix state aligned.

## Documentation/spec updates required

- `docs/spec/BASPROJ_SPEC_V1.md`
  - add/describe `BuildTarget=WrappedComServer`, parser alias policy, and sample.
- `docs/spec/BUILD_TARGET_AND_WRAPPER_BOUNDARY_V1.md`
  - add WrappedComServer as a packaging target over `.oxb`.
- COM server/conformance docs
  - distinguish skeleton/generated-source support from registered COM server
    execution support.
- OxIde guidance
  - add build-plan/build-result DTO expectations and disabled-state rules.
- README / scripts docs
  - document CLI build/register flow only after execution exists.

## Non-goals for this workset

- Out-of-process `ComExe` delivery, except preserving metadata design.
- Arbitrary native struct/record ABI signatures in dual vtables.
- Replacing all internal OxVba dynamic dispatch with Windows `IDispatch`.
- Thread-safe worksheet UDF claims.
- Automation Add-In behavior as the primary host-UDF model.
- `NativeComServer` codegen closure.

## Terminal condition

This workset is complete only when:

1. `BuildTarget=WrappedComServer` can emit a Windows in-process COM DLL from an
   OxVba project with creatable class modules.
2. External clients can late-bind through `IDispatch` into OxVba class methods,
   properties, default members, object returns, arrays, and supported errors.
3. Generated TLB metadata enables real early-bound client calls through stable
   DISPIDs.
4. At least one Automation-safe dual-interface vtable path is live and
   dispatch-equivalent.
5. COM events publish source dispinterfaces and fire through connection points.
6. Host UDF descriptors/catalog/invoke support is available for DnaOneCalc-style
   hosts with conservative Excel-informed semantics.
7. OxIde/direct-host build configuration surfaces can present, validate, and run
   the target without CLI text parsing.
8. Validation matrices, docs, and evidence artifacts agree with the actual
   implemented subset.

## Bead tree

Parent:

- `bd-wcs1` - WrappedComServer interface/event/UDF execution

Epics:

- `bd-wcs1.1` - workset initiation, naming, and descriptor contract lock
- `bd-wcs1.2` - internal COM projection core
- `bd-wcs1.3` - call descriptor/call-frame and marshaling core
- `bd-wcs1.4` - bundle export metadata and build-target integration
- `bd-wcs1.5` - wrapped late-bound COM server DLL
- `bd-wcs1.6` - typelib and dispatch-backed early binding
- `bd-wcs1.7` - dual-interface vtable projection
- `bd-wcs1.8` - connection-point events
- `bd-wcs1.9` - host UDF catalog and OxIde/DnaOneCalc build config surfaces
- `bd-wcs1.10` - validation, oracle evidence, docs, and terminal audit

First executable beads:

- `bd-wcs1.1.1` - publish this workset, matrix rows, and first bead rollout
- `bd-wcs1.1.2` - lock `WrappedComServer` naming and BuildTarget/OutputType
  contract in specs
- `bd-wcs1.2.1` - add GUID-capable interface identity and custom-IID descriptor
  tests
- `bd-wcs1.2.2` - split object identity from interface projection while keeping
  existing `ObjectRef` behavior green
- `bd-wcs1.2.3` - add descriptor-backed custom `QueryInterface` projection tests
- `bd-wcs1.3.1` - introduce `RuntimeCallFrame` and call result abstractions
- `bd-wcs1.3.2` - route one pure OxVba class method/property/default-member slice
  through the call-frame core
- `bd-wcs1.3.3` - centralize COM `DISPPARAMS`/`VARIANT` marshaling helpers over
  call frames
- `bd-wcs1.4.1` - persist COM class/interface/member/event descriptors in
  `OxBundle`
- `bd-wcs1.4.2` - add `BuildTarget=WrappedComServer` parser/generator/host DTO
  support
- `bd-wcs1.5.1` - compile generated wrapped COM DLL with standard COM exports
- `bd-wcs1.5.2` - implement `IClassFactory::CreateInstance` over
  `Engine::create_class_instance`
- `bd-wcs1.5.3` - implement `IDispatch::GetIDsOfNames` and `Invoke` over
  descriptors/call frames
- `bd-wcs1.5.4` - add controlled Windows client DLL-load late-bound invoke
  evidence
- `bd-wcs1.5.5` - expand controlled late-bound dispatch breadth beyond scalar
  method slice
- `bd-wcs1.5.6` - implement registered/per-user `CreateObject` publication path
- `bd-wcs1.6.1` - generate/register TLB from descriptors for one class
- `bd-wcs1.6.2` - add Office/VBA early-bound oracle for wrapped class calls
- `bd-wcs1.7.1` - generate one Automation-safe dual-interface vtable projection
- `bd-wcs1.7.2` - prove dispatch/vtable equivalence for the same wrapped member
- `bd-wcs1.8.1` - publish event source dispinterfaces and connection-point
  descriptors
- `bd-wcs1.8.2` - implement `Advise`/`Unadvise` and fire `RaiseEvent` into a sink
- `bd-wcs1.8.3` - add VBA/controlled sink event oracle evidence
- `bd-wcs1.9.1` - add host-callable UDF descriptors to bundle/project metadata
- `bd-wcs1.9.2` - expose typed host UDF catalog/invoke API with caller context and
  volatile/dependency sinks
- `bd-wcs1.9.3` - expose OxIde build-plan/build-result/registration DTOs for
  WrappedComServer
- `bd-wcs1.9.4` - make direct-host WrappedComServer build execute and verify
  artifacts
- `bd-wcs1.10.1` - refresh COM/project-hosting validation matrices and traceability
- `bd-wcs1.10.2` - publish final evidence and terminal audit for the implemented
  subset

## Bead execution policy

- Capability lanes do not close on support-only documentation beads.
- Any bead that exposes a new required capability gap must leave behind a child
  delivery bead before closing.
- Windows/Office-dependent evidence may be guarded, but claims involving real COM
  activation, early-bound Office/VBA calls, or events require a live Windows
  evidence artifact before completion language is allowed.

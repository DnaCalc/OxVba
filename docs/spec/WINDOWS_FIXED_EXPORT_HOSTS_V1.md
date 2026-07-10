# Windows Fixed-Export Hosts v1

> [!NOTE]
> **Supporting historical design.** Current wrapper/native-output architecture is [`OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md`](OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md) §§12-13.

Status: `design-draft`
Date: 2026-04-14
Scope owner: OxVBA runtime/host/tooling
Canonical path: `docs/spec/WINDOWS_FIXED_EXPORT_HOSTS_V1.md`

Related docs:
- `docs/spec/BUILD_TARGET_AND_WRAPPER_BOUNDARY_V1.md`
- `docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md`
- `docs/spec/COM_CLIENT_SERVER_SCOPE_V1.md`
- `docs/spec/CLASS_MODULE_COM_ALIGNMENT_PLAN_V1.md`
- `docs/spec/BASPROJ_SPEC_V1.md`

Informative public reference:
- `Excel-DNA/ExcelDna` native host structure for fixed XLL/COM exports plus runtime dispatch

---

## 1. Purpose

Define the Windows-native host shapes that can safely late-bind OxVBA behavior from a canonical `.oxb` payload while exposing only a fixed native export surface.

This document exists to separate three materially different product shapes:

1. generic native DLLs with caller-chosen exported function names,
2. XLL hosts with Excel-defined fixed exports plus runtime registration,
3. COM server hosts with fixed `Dll*` exports plus runtime class activation.

The central conclusion is:

- generic DLLs still require build-time named exports,
- XLL and COM hosts can use fixed native exports with late-bound `.oxb` internals.

---

## 2. Executive Summary

OxVBA should not try to force one export strategy across all Windows-native outputs.

The Excel-DNA style fixed-export thunk-table pattern is a strong fit for:

- XLL hosting,
- COM server hosting,
- and a combined XLL + COM host binary.

It is not a useful general solution for arbitrary native DLL consumers that expect:

- specific exported symbol names,
- specific signatures discoverable through `GetProcAddress`,
- and no host-specific runtime registration protocol.

Therefore the Windows native-output plan should be split into two families:

1. `WrapperLibrary` / `NativeLibrary` for generic DLL consumers
   - exports are determined at build time,
   - names/signatures must be real OS exports,
   - not covered by this note except as an explicit non-goal.

2. fixed-export Windows hosts over `.oxb`
   - `XllHost`
   - `ComHost`
   - optional combined `XllComHost`
   - internals late-bound from reflective bundle metadata at runtime.

This note defines the second family.

---

## 3. Canonical Rules

1. `.oxb` remains the canonical compiled semantic artifact.
2. Fixed-export Windows hosts are packaging/hosting layers over `.oxb`; they do not redefine VBA semantics.
3. A fixed-export host may late-bind procedure/class behavior from `.oxb` only when the consumer protocol itself is late-bound or registration-driven.
4. XLL and COM qualify as such protocols.
5. Arbitrary native DLL consumption does not qualify and still requires real build-time exported names.
6. Host-specific metadata required for XLL or COM activation must be carried in bundle metadata or a tightly coupled sidecar manifest, not rediscovered from source.
7. One shared Windows host core should own bundle loading, metadata inspection, engine/session lifecycle, marshaling, and diagnostics.
8. XLL and COM frontends should be thin export-surface adapters over that shared host core.

---

## 4. Why Generic DLL Is Different

For a generic DLL, the native consumer normally expects this shape:

1. `LoadLibrary("foo.dll")`
2. `GetProcAddress("SpecificName")`
3. call the function using a predefined ABI

That means the export names and signatures must already exist in the PE export table when the binary is linked. A runtime thunk table with names like `Thunk0` or `f17` is only useful if the consumer also understands an external registration or slot-binding protocol.

Excel and COM do understand such protocols:

- Excel XLL registration maps user-visible worksheet function names to preexisting exported procedure names.
- COM activation uses fixed `Dll*` exports to obtain class factories, after which interface behavior is late-bound through COM dispatch/vtable mechanics.

A generic DLL consumer does not provide such a protocol. Therefore:

- fixed-export + runtime binding is correct for XLL and COM,
- generated named exports remain the correct path for generic DLLs.

---

## 5. Product Shapes

### 5.1 Generic `WrapperLibrary` / `NativeLibrary`

Status in this note: non-goal except for explicit boundary.

Characteristics:

- arbitrary native export names,
- direct caller control over ABI surface,
- intended for plain DLL consumers.

Required export strategy:

- build-time generated real exports.

### 5.2 `XllHost`

Characteristics:

- fixed native entrypoints required by Excel,
- runtime function registration using exported procedure names,
- worksheet/UDF/command surface bound dynamically from `.oxb`.

Required export strategy:

- fixed native Excel-facing exports,
- bounded thunk export table for registered functions.

### 5.3 `ComHost`

Characteristics:

- fixed COM DLL exports,
- runtime class-factory and object activation from `.oxb` metadata,
- object behavior dispatched dynamically through OxVBA runtime.

Required export strategy:

- fixed `DllRegisterServer`, `DllUnregisterServer`, `DllGetClassObject`, `DllCanUnloadNow`.

### 5.4 Combined `XllComHost`

Characteristics:

- one binary that serves both Excel add-in hosting and COM server activation,
- closest to the Excel-DNA deployment model,
- shared bundle/session/class-factory state.

Required export strategy:

- union of XLL and COM fixed export sets.

Recommended direction:

- support a combined host after standalone `ComHost` and `XllHost` are stable,
- but keep the internal architecture unified from the start so separate and combined packaging are trivial policy choices.

---

## 6. Host Artifact Model

### 6.1 Canonical payload

Every fixed-export Windows host consumes:

- one `.oxb` bundle,
- optional host-specific sidecar manifest,
- optional bundled resources/icons/registration data.

The `.oxb` remains the source of truth for:

- project identity,
- modules,
- procedures,
- runtime/export metadata,
- COM class metadata,
- and future Excel registration metadata.

### 6.2 Packaging forms

Allowed packaging shapes:

1. embedded payload
   - host binary embeds `.oxb` bytes as a PE resource or static blob,
   - easiest deployment for single-file shipping.

2. sidecar payload
   - host binary loads adjacent `.oxb`,
   - simpler update story and easier artifact inspection.

3. manifest-directed payload
   - host binary loads `.oxb` from a path specified in a sidecar manifest,
   - allows richer packaging but increases deployment complexity.

Recommended order:

1. sidecar payload first,
2. embedded payload second,
3. manifest-directed payload later if needed.

### 6.3 Host manifest

If a sidecar manifest is used, it should carry only host-packaging concerns, not semantic compilation state.

Proposed fields:

- `host_kind`: `xll`, `com`, `xll-com`
- `bundle_path`
- `display_name`
- `progids` / registration overrides if needed
- `excel_addin_name`
- `com_registration_scope`
- `diagnostic_policy`
- `runtime_profile_override`
- `policy_preset_override`

This manifest must not duplicate:

- procedure semantics,
- type information already present in the bundle,
- COM member signatures already present in bundle metadata.

---

## 7. Shared Windows Host Core

Define a shared crate or internal module layer, informally referred to here as `oxvba-host-win`.

Responsibilities:

- locate/load `.oxb`,
- deserialize bundle metadata,
- create and cache `Engine`,
- create and manage `ProjectRuntimeSession`,
- surface diagnostics,
- manage lifetime/unload state,
- provide common marshaling helpers,
- maintain dispatch maps for XLL and COM adapters,
- coordinate registration state and reference counts.

### 7.1 Core state object

Conceptual structure:

```text
WindowsHostState
- load mode: embedded | sidecar | manifest
- bundle bytes / parsed OxBundle
- Engine
- ProjectRuntimeSession
- runtime profile / policy
- xll registry state
- thunk slot table
- com class catalog
- com class-factory cache
- outstanding object count
- diagnostics sink state
- unload state flags
```

### 7.2 Initialization modes

The core must support lazy initialization:

- first XLL entrypoint call,
- first COM activation/registration call,
- explicit host preload call in tests.

Initialization must be idempotent within one process and safe against duplicate open paths.

### 7.3 Error reporting

The core must classify errors as:

- bundle load failure,
- bundle metadata validation failure,
- host registration failure,
- runtime invocation failure,
- COM activation failure,
- Excel registration failure,
- unsupported-profile/policy failure.

The frontend host must map these into:

- Excel-friendly failures,
- COM `HRESULT`s,
- deterministic diagnostic logs,
- and test-visible structured results.

---

## 8. Required Bundle Metadata Extensions

Current `.oxb` metadata is already sufficient for basic project inspection, but fixed-export hosts need richer host-specific inventories.

### 8.1 Existing useful metadata

Already present or partially present:

- manifest snapshot,
- procedure runtime metadata,
- host export inventory,
- COM class export inventory.

### 8.2 New `ExcelExportInventory`

Add an explicit Excel registration inventory to the bundle.

Each row should identify:

- module name,
- procedure name,
- public function/command name,
- export slot kind: `worksheet-function`, `command`, `macro-helper`, other host-defined role,
- argument descriptors,
- return descriptor,
- category,
- description/help topic,
- volatility,
- thread-safety intent,
- macro-type intent,
- async intent if later supported,
- hidden/internal registration flags.

This inventory is not the same as native named-export metadata. It describes Excel registration intent, not PE export names.

### 8.3 `ComClassInventory` completion

The COM class inventory must be rich enough to support runtime registration and activation without source-level reanalysis.

Required fields:

- CLSID,
- ProgID / version-independent ProgID,
- creatable flag,
- public class name,
- implemented interface list,
- dispatch exposure policy,
- registration metadata needed for `DllRegisterServer`,
- module/procedure/member dispatch map,
- threading model declaration,
- optional typelib linkage or generated typelib path if later supported.

### 8.4 Host bootstrap metadata

The bundle or manifest must also carry:

- designated startup procedure if any,
- runtime profile requirement,
- policy fingerprint,
- reference/bootstrap requirements for deterministic loading,
- target bitness constraints if host-specific.

---

## 9. XLL Host Design

### 9.1 Scope

`XllHost` provides an Excel add-in surface over `.oxb`.

It does not expose arbitrary VBA procedures as OS exports. Instead it:

- exports the fixed XLL entrypoint set expected by Excel,
- exports a bounded set of thunk procedures,
- registers OxVBA procedures with Excel at runtime using those thunk exports.

### 9.2 Fixed native exports

Initial export set:

- `xlAutoOpen`
- `xlAutoClose`
- `xlAutoRemove`
- `xlAutoFree12`
- `xlAddInManagerInfo12`
- `SetExcel12EntryPt` if required for supported Excel lanes

Optional exports:

- `SyncMacro`
- `RegistrationInfo`
- `CalculationCanceled`
- `CalculationEnded`

These should be treated as host protocol exports, not user-defined exports.

### 9.3 Thunk table

The host should export a bounded set of thunk procedures:

- `f0`
- `f1`
- ...
- `fN`

Where each thunk:

- has a fixed XLL-callable ABI,
- looks up slot `N` in the host dispatch table,
- forwards the call through a common invocation path.

The dispatch table is populated at runtime from `ExcelExportInventory`.

### 9.4 Registration model

At `xlAutoOpen`:

1. initialize host core,
2. load `.oxb`,
3. validate `ExcelExportInventory`,
4. assign each exportable Excel procedure to a thunk slot,
5. call Excel registration APIs with exported proc names `f{slot}`,
6. retain returned registration IDs for later unregister.

This yields:

- dynamic worksheet function names,
- dynamic descriptions/categories,
- fixed native export surface.

### 9.5 Invocation flow

Conceptual call path:

```text
Excel -> f37 export -> thunk slot 37 ->
XLL dispatcher -> OxVBA invocation adapter ->
Engine::invoke_procedure(session, module, procedure, args) ->
marshal result back to XLL ABI
```

### 9.6 State and lifecycle

`XllHost` must track:

- open/closed state,
- registration IDs,
- slot-to-procedure map,
- allocated return-memory state for `xlAutoFree12`,
- host session reference state,
- whether COM activation has also occurred in the same process.

Unload policy must be explicit:

- safe re-open,
- deterministic unregister,
- no use-after-free in thunk table,
- defined behavior when Excel closes while COM clients still hold objects.

### 9.7 Marshaling policy

Initial support should be limited and explicit:

- scalar numerics,
- booleans,
- strings,
- empty/missing/error carriers as supported by the XLL boundary,
- bounded array support only when the marshal contract is fully specified.

Async functions, cluster-safe flags, and advanced Excel registration features should remain out of scope until the sync path is stable.

### 9.8 Non-goals for v1

- arbitrary runtime growth of thunk count,
- per-method deregistration and slot reuse sophistication,
- macro sheet compatibility breadth beyond explicitly supported subset,
- automatic support for every Excel-DNA registration feature.

---

## 10. COM Host Design

### 10.1 Scope

`ComHost` provides a classic in-proc COM DLL over `.oxb`.

It does not expose user procedures as PE exports. Instead it:

- exports the standard COM DLL entrypoints,
- loads COM class metadata from `.oxb`,
- creates runtime-backed class factories and COM objects dynamically.

### 10.2 Fixed native exports

Required export set:

- `DllRegisterServer`
- `DllUnregisterServer`
- `DllGetClassObject`
- `DllCanUnloadNow`

These are the only PE exports COM activation requires from the server.

### 10.3 Activation model

At first relevant COM entrypoint:

1. initialize host core,
2. load `.oxb`,
3. validate COM class inventory,
4. materialize class-factory descriptors from metadata,
5. serve activation requests through generic class factories.

### 10.4 Object model strategy

Recommended first implementation:

- `IDispatch`-first automation server model.

Reasons:

- aligns with VBA/COM late-bound usage,
- minimizes early ABI surface,
- allows dynamic member dispatch from bundle metadata,
- avoids premature vtable/interface-emission complexity.

Possible later expansion:

- dual interfaces,
- generated type library,
- vtable-backed early-bound interfaces.

### 10.5 Generic class factory behavior

Each class factory should:

- identify the target class metadata row,
- create an OxVBA-backed object instance in host core,
- wrap it in a COM-visible facade,
- expose `IUnknown` and `IDispatch`,
- forward member calls into OxVBA runtime dispatch.

### 10.6 Registration model

`DllRegisterServer` should:

- load bundle metadata,
- write registry entries for each exposed class,
- register ProgIDs/CLSIDs and server path,
- write threading model according to host declaration,
- optionally register typelib information if supported.

`DllUnregisterServer` should reverse those writes deterministically.

### 10.7 Lifetime model

The host must track:

- server lock count,
- outstanding object count,
- class-factory references,
- session references,
- whether XLL mode is also active in the same process.

`DllCanUnloadNow` must report unload eligibility according to those counts and explicit policy.

### 10.8 Threading model

Initial recommendation:

- support STA semantics first,
- reject or explicitly constrain MTA activation until runtime/object safety is demonstrated.

This must be documented in both metadata and registration output.

### 10.9 Error mapping

Member invocation failures must map to deterministic `HRESULT`s and `EXCEPINFO`/automation error details where appropriate.

This mapping must distinguish:

- member not found,
- wrong arity,
- type conversion failure,
- runtime exception,
- object state failure,
- unsupported feature.

---

## 11. Combined XLL + COM Host

### 11.1 Purpose

One binary may need to serve both:

- Excel XLL hosting,
- COM activation for RTD servers, automation exposure, or VBA-facing COM services.

This model is proven viable in the public ecosystem and is a good fit for OxVBA if the internals are cleanly separated.

### 11.2 Export set

The combined binary exports the union of:

- XLL fixed exports,
- COM fixed exports,
- thunk table exports.

### 11.3 Shared state rules

The combined host must not create fragmented runtime state for XLL and COM lanes.

Canonical behavior:

- one loaded bundle,
- one host core state object,
- one policy/profile context,
- coordinated reference counting across both lanes.

### 11.4 Lifecycle interactions

Important cases:

1. Excel opens add-in first, then COM activation happens.
2. COM activation happens first, then Excel opens add-in.
3. Excel closes add-in while COM objects are still live.
4. COM clients release all objects while Excel add-in remains open.
5. Excel reloads the add-in in-process.

The host must define these cases explicitly so unload behavior is deterministic and testable.

Recommended rule:

- no full teardown while either XLL-open state or COM outstanding-object state remains active.

---

## 12. Host ABI and Marshaling Boundaries

### 12.1 Principle

Host frontends own boundary marshaling. OxVBA core continues to own VBA semantics and runtime values.

### 12.2 XLL boundary

The XLL adapter owns:

- conversion from Excel call ABI into OxVBA runtime arguments,
- return-value allocation policy,
- `xlAutoFree12` memory ownership,
- Excel error/empty/missing translation.

### 12.3 COM boundary

The COM adapter owns:

- `VARIANT` translation,
- `BSTR` allocation/ownership,
- `SAFEARRAY` translation where supported,
- `IDispatch` name and `DISPID` resolution,
- `HRESULT` and automation exception mapping.

### 12.4 Internal invocation contract

Both frontends should converge on one internal dispatch contract:

```text
invoke(target_kind, target_id, args) -> result
```

Where:

- `target_kind` may be procedure, command, class member, property getter, property setter, event sink, or host helper,
- `target_id` is resolved from bundle metadata,
- `args` are OxVBA runtime values.

---

## 13. Deployment and Build Plan

### 13.1 Build products

Proposed build targets:

- `XllHost`
- `ComHost`
- `XllComHost`

These are distinct from `WrapperExe` and `WrapperLibrary`.

### 13.2 Packaging modes

First delivery preference:

1. build generic fixed-export host binaries,
2. place `.oxb` beside the host binary,
3. load by adjacent filename convention.

Later options:

- embed bundle bytes,
- manifest-selected bundle,
- signed deployment packages.

### 13.3 Naming conventions

Example packaging names:

- `MyAddIn.xll` + `MyAddIn.oxb`
- `MyComServer.dll` + `MyComServer.oxb`
- `MyExcelHost.xll` + `MyExcelHost.oxb`

If a combined host uses `.xll` extension while also serving COM, COM registration still points to the same underlying binary path.

---

## 14. Validation and Test Matrix

### 14.1 XLL tests

Required coverage:

- bundle load and metadata validation,
- thunk slot assignment,
- registration parameter generation,
- function invocation success/failure,
- `xlAutoOpen` / `xlAutoClose` / `xlAutoRemove`,
- reload behavior,
- return-memory ownership through `xlAutoFree12`,
- multiple registered functions with shared host state.

### 14.2 COM tests

Required coverage:

- registration/unregistration,
- `DllGetClassObject`,
- class factory creation,
- `IUnknown` lifetime behavior,
- `IDispatch` member invocation,
- property get/set,
- object creation failure modes,
- `DllCanUnloadNow`,
- multiple concurrent COM objects.

### 14.3 Combined-host tests

Required coverage:

- XLL open before COM activation,
- COM activation before XLL open,
- add-in close with outstanding COM objects,
- COM release with add-in still open,
- deterministic teardown at final release.

### 14.4 Cross-cutting tests

Required coverage:

- sidecar bundle missing/corrupt,
- metadata mismatch,
- policy/profile rejection,
- diagnostics emission,
- repeated initialization races,
- 32-bit vs 64-bit packaging behavior for supported Windows targets.

---

## 15. Implementation Phases

### Phase 1: Metadata closure

Deliver:

- `ExcelExportInventory` spec and bundle carriage,
- completed COM inventory shape,
- host bootstrap metadata needed for fixed-export hosts.

Exit condition:

- `.oxb` fully describes XLL and COM activation surfaces without source reanalysis.

### Phase 2: Shared Windows host core

Deliver:

- bundle loader,
- host core state,
- initialization/lifetime logic,
- diagnostics surface,
- internal invocation adapter.

Exit condition:

- one process-local host core can load `.oxb` and invoke procedures/classes from metadata.

### Phase 3: `ComHost`

Deliver:

- fixed `Dll*` export binary,
- registration and activation path,
- `IDispatch` object façade,
- deterministic unload policy.

Exit condition:

- COM server activation works end to end from `.oxb`.

### Phase 4: `XllHost`

Deliver:

- fixed XLL export binary,
- thunk table export set,
- Excel registration from `ExcelExportInventory`,
- invocation and unload path.

Exit condition:

- Excel-visible functions and commands register and invoke from `.oxb`.

### Phase 5: `XllComHost`

Deliver:

- combined packaging option,
- shared-state lifetime coordination,
- evidence for mixed XLL/COM lifecycle cases.

Exit condition:

- one binary can safely serve both lanes over one `.oxb`.

---

## 16. Open Decisions

1. Sidecar versus embedded bundle as first shipping mode.
2. Whether XLL thunk count is fixed globally or selectable per build.
3. Whether `ComHost` v1 is strictly `IDispatch`-only or allows limited dual-interface support.
4. Whether a combined `XllComHost` ships in the first release or only after separate hosts are stable.
5. Whether host-specific registration metadata lives entirely in `.oxb` or partly in a sidecar manifest.
6. Whether COM registration writes are performed by the host binary itself or an external registration tool.

Until these are resolved, status for the overall lane remains `in-progress`.

---

## 17. Non-Goals

This spec does not define:

- generic DLL named-export generation,
- Linux shared-library hosting,
- macOS bundle/plugin hosting,
- true native-code compilation from Cranelift object output,
- Office internals beyond public XLL/COM-facing protocol behavior,
- full Excel add-in feature parity on day one.

---

## 18. Summary

The fixed-export late-bound host pattern is the correct Windows-native architecture for OxVBA when the consumer protocol already provides a late-bound or registration-driven surface.

That yields a clean split:

- generic DLLs: build-time named exports,
- XLL hosts: fixed exports plus thunk-table registration,
- COM hosts: fixed `Dll*` exports plus runtime class activation,
- combined XLL + COM host: one binary over one `.oxb` with coordinated state.

The next concrete engineering move is not native-code compilation. It is metadata closure plus a shared Windows host core that both XLL and COM frontends can sit on top of.

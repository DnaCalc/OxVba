# Build Target And Wrapper Boundary v1

Status: `draft`
Date: 2026-04-03
Scope owner: OxVBA project/build system
Canonical path: `docs/spec/BUILD_TARGET_AND_WRAPPER_BOUNDARY_V1.md`

Related docs:
- `docs/spec/BASPROJ_SPEC_V1.md`
- `docs/worksets/WORKSET_2026-04-02_WRAPPER_BUILD_TARGET_AND_NATIVE_HOSTING_EXECUTION.md`
- `docs/worksets/WORKSET_2026-05-09_WRAPPED_COM_SERVER_INTERFACE_EVENT_UDF_EXECUTION.md`

---

## 1. Purpose

Define the explicit boundary between:
- semantic project shape, selected by `.basproj` `OutputType`
- physical packaging/build shape, selected by `.basproj` `BuildTarget`

This separation is required so wrapper/native-hosting lanes do not overload semantic project meaning.

---

## 2. Canonical Rules

1. `.oxb` remains the canonical compiled semantic artifact.
2. `OutputType` controls semantic project behavior.
3. `BuildTarget` controls emitted packaging shape.
4. Wrapper outputs are packaging layers over a canonical `.oxb` payload.
5. Wrapper builders consume existing project/runtime/export metadata; they do not reinterpret VBA semantics independently.

---

## 3. BuildTarget Values

| BuildTarget | Meaning | Current expectation |
|-------------|---------|---------------------|
| `Bundle` | Canonical OxVBA bundle artifact | emits `.oxb`; current stable default except `OutputType=Addin`, where `oxvba build` packages the bundle into a generated `.xll` |
| `WrapperExe` | Native executable wrapper over a canonical `.oxb` payload | planned delivery lane |
| `WrapperLibrary` | Native DLL/shared-library wrapper over a canonical `.oxb` payload | planned delivery lane |
| `WrappedComServer` | Windows in-process COM DLL wrapper over a canonical `.oxb` payload for `OutputType=ComServer` projects | bounded Windows DLL lane active: package/descriptor/IDL/shim source, compiled `.tlb`, and compiled in-process COM DLL with per-user class/typelib registration, late-bound `IDispatch` activation/dispatch, dispatch-backed connection-point event publication, and one Automation-safe dual-interface vtable method shape |

Default: `Bundle`

`WrappedComServer` is the canonical build-target spelling. `WrapperComServer` is reserved only as a compatibility alias for human input if project parsers or host surfaces choose to accept it; canonical generated `.basproj` files and host DTOs should emit `WrappedComServer`.

`OutputType=ComServer` and `BuildTarget=WrappedComServer` are intentionally separate. `OutputType=ComServer` declares semantic COM server shape: exposed/creatable classes, class/interface metadata, and COM server rules for project execution. `BuildTarget=WrappedComServer` declares the physical packaging lane: compile the canonical `.oxb` plus reusable descriptor metadata into a Windows in-process COM DLL wrapper.

---

## 4. Boundary Contract

The wrapper boundary must receive enough information from the canonical OxVBA side to package a hostable artifact without reconstructing semantic meaning:

- project semantic kind (`OutputType`)
- build packaging kind (`BuildTarget`)
- canonical compiled bundle payload
- startup/entry metadata when applicable
- native export descriptors when applicable
- project/runtime policy metadata required at launch
- reference metadata needed for deterministic host bootstrap
- COM class/interface/member/event descriptors when `BuildTarget=WrappedComServer`
- registration metadata: CLSIDs, ProgIDs, type library identity, bitness, registration scope, and manifest or registry output plan

Descriptor compatibility policy: wrappers and hosts must consume COM and
host-call descriptor truth from the serialized `BundlePackage` and the
export-surface-derived COM descriptor artifacts. The strict package reader
rejects unsupported package formats/versions and invalid entry-bundle metadata;
wrappers must fail with typed package diagnostics rather than reparsing source
files or silently inventing metadata.

This contract intentionally keeps:
- compiler/runtime semantics in the existing OxVBA core
- packaging mechanics in wrapper/native-hosting lanes

---

## 5. Non-Goals

This spec does not by itself define:
- EXE wrapper binary layout
- DLL/shared-library ABI/export layout
- COM server registration details
- XLL entrypoint layout

---

## 6. WrappedComServer Boundary

The WrappedComServer lane produces a Windows desktop in-process COM server wrapper around the canonical OxVBA bundle. The expected artifact set is:

- `.oxb` canonical semantic payload
- compiled DLL containing the COM entry points and wrapper runtime
- generated type library when the selected tier requires early binding
- registration plan and, where selected, registry script or registration-free manifest
- build transcript, diagnostics, and debug symbols where available

The first physical tier must provide standard in-process COM activation and late-bound dispatch for the scoped class set before any broader server/export row can move beyond planned/subset status. Later tiers add generated type libraries, dispatch-backed connection-point events, and then dual-interface vtable projection.

Current `oxvba build --target WrappedComServer` output emits the canonical
`.oxb` package, deterministic COM descriptor JSON, IDL, compiled `.tlb`,
auditable shim source, and a compiled Windows in-process COM DLL. The generated
DLL embeds the package and descriptor, exports `DllGetClassObject`,
`DllCanUnloadNow`, `DllRegisterServer`, and `DllUnregisterServer`, registers
creatable classes and the generated type library under `HKCU\Software\Classes`,
supports late-bound `IDispatch` activation/member dispatch through the clean
package-backed runtime session, exposes source dispinterfaces through standard
COM connection points for project `RaiseEvent` publication, and emits a real
dual-interface vtable face for classes whose default interface fits the first
implemented Automation-safe shape: one no-argument `Long`-returning method
published as `HRESULT Method([out, retval] long*)`.

The active subset is still intentionally bounded. Early-bound client parity,
registration-free manifests, broader dual-interface argument/property/byref/
object/array/error parity, and broader Office/VBA client parity remain follow-on
work beyond the current dispatch-backed DLL, registered typelib,
connection-point event, and bounded raw-COM dual-slot slice. Excel/VBA evidence
for the event class uses a typed dispatch-only interface and `WithEvents` sink
for member invocation and connection-point subscription; the dual-interface
evidence is a controlled raw-COM `QueryInterface` plus vtable call on the
eligible scalar method class.

The lane is Windows-only unless a future workset defines a portable equivalent. Bitness, toolchain availability, registration scope, and administrative requirements must be reported through build-plan/build-result surfaces rather than inferred from CLI text.

Those are downstream delivery lanes built on this boundary.

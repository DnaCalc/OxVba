# Windows VBA 7.1 x64 Interface, Event, and Layout-Sensitive Fact Pack

Date: 2026-04-20
Owner: Codex
Status: published
Workset: `WORKSET_2026-04-20_VALUE_MODEL_MIGRATION_COMPARISON_AND_PERF_PLAN.md`
Bead: `bd-t8rr.2.4` / `vmm-b3`

## Scope

This note records the current evidence-backed fact pack for:

- COM interface identity and Automation-facing interface conventions
- COM event payload transport relevant to VBA `WithEvents`
- layout-sensitive behavior that intersects the value-model migration but is not
  yet a fully closed ABI-lane claim in OxVba

Normative precedence remains:

1. actual Windows/VBA observable behavior where we can establish it
2. published Microsoft specifications and API documentation
3. current OxVba behavior only as baseline evidence

## Primary Source Set

Microsoft primary sources used here:

- `Rules for Implementing QueryInterface`:
  https://learn.microsoft.com/en-us/windows/win32/com/rules-for-implementing-queryinterface
- `IUnknown::QueryInterface`:
  https://learn.microsoft.com/en-us/windows/win32/api/unknwn/nf-unknwn-iunknown-queryinterface%28refiid_void%29
- `IDispatch interface (oaidl.h)`:
  https://learn.microsoft.com/en-us/windows/win32/api/oaidl/nn-oaidl-idispatch
- `ActiveX Client and Object Interaction`:
  https://learn.microsoft.com/en-us/previous-versions/windows/desktop/automat/activex-client-and-object-interaction
- `oleautomation attribute`:
  https://learn.microsoft.com/en-us/windows/win32/midl/oleautomation
- `IConnectionPointContainer interface (ocidl.h)`:
  https://learn.microsoft.com/en-us/windows/win32/api/ocidl/nn-ocidl-iconnectionpointcontainer
- `IConnectionPoint interface (ocidl.h)`:
  https://learn.microsoft.com/en-us/windows/win32/api/ocidl/nn-ocidl-iconnectionpoint
- `CONNECTDATA structure (ocidl.h)`:
  https://learn.microsoft.com/en-us/windows/win32/api/ocidl/ns-ocidl-connectdata
- `[MS-OAUT] 2.2.34 EXCEPINFO`:
  https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/a7bb989f-5c55-49c7-98e1-24ab2593a9fa
- `[MS-OAUT] 2.2.49.4.3 Dispinterface Interfaces`:
  https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/07829751-cb56-4eec-88ef-476f8a09dd43

Checked-in OxVba source/test/spec evidence used here:

- [pointer_helpers_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/pointer_helpers_end_to_end.rs)
- [windows_connection_point.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_connection_point.rs)
- [windows_runtime_state.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_runtime_state.rs)
- [com_client_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/com_client_end_to_end.rs)
- [com_early_project_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/com_early_project_end_to_end.rs)
- [typelib_catalog.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/fixtures/typelib_catalog.rs)
- [OXVBA_POINTER_HELPERS_CONTRACT_V1.md](/C:/Work/DnaCalc/OxVba/docs/spec/OXVBA_POINTER_HELPERS_CONTRACT_V1.md)
- [HAL_DECLARE_ABI_SPEC_V1.md](/C:/Work/DnaCalc/OxVba/docs/spec/HAL_DECLARE_ABI_SPEC_V1.md)
- [HAL_DECLARE_MARSHAL_AMBIGUITIES_2026-03-02.md](/C:/Work/DnaCalc/OxVba/docs/evidence/hal/HAL_DECLARE_MARSHAL_AMBIGUITIES_2026-03-02.md)
- [WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md](/C:/Work/DnaCalc/OxVba/docs/worksets/WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md)

Local x64 ABI-size probe used here:

- a one-off PowerShell/C# layout probe derived from the documented
  `CONNECTDATA` fields reported:
  - `CONNECTDATA_COMPAT size=16`
  - `IntPtr size=8`

That probe is supporting evidence for x64 size consequences, not a
Microsoft-published size table.

## Confirmed Windows/Automation Facts

### `IFACE-F1`: COM object identity is defined through `IUnknown`

- Microsoft documents that for any given COM object, querying `IID_IUnknown`
  from any supported interface must always return the same physical pointer
  value
- Microsoft also documents the static/reflexive/symmetric/transitive
  `QueryInterface` rules

Migration implication:

- any migrated canonical interface carrier must preserve a stable identity rule
  that can be compared through `IUnknown` semantics
- wrapper churn or per-call rebinding must not destroy the ability to identify
  one logical COM object instance across interface views

### `IFACE-F2`: `IDispatch` is an `IUnknown`-derived Automation interface

- Microsoft documents that `IDispatch` inherits from `IUnknown`
- `IDispatch` exposes `GetTypeInfoCount`, `GetTypeInfo`, `GetIDsOfNames`, and
  `Invoke`
- COM components implement `IDispatch` to support Automation clients such as
  Visual Basic

Migration implication:

- the migrated value model must continue to treat Automation object transport as
  interface transport, not as generic boxed runtime values
- `IUnknown` identity and `IDispatch` invocation remain linked concerns

### `IFACE-F3`: Dual interfaces place `IUnknown` then `IDispatch` at the front of the vtable

- Microsoft documents that in a dual interface:
  - the first three entries are `IUnknown`
  - the next four entries are `IDispatch`
  - later entries are the direct interface members
- Microsoft also documents that a dispinterface is an Automation interface that
  specifies the properties and methods the server's `IDispatch` implementation
  must implement

Migration implication:

- any future direct early-bound/native-interface work must preserve that front
  vtable ordering
- even if OxVba keeps a semantic-first object model internally, the native
  interface boundary cannot ignore these ordering facts

### `IFACE-F4`: Automation-compatible interfaces can derive from `IUnknown` or `IDispatch`

- Microsoft documents that an interface is Automation-compatible if it derives
  from `IDispatch` or `IUnknown`, has `[oleautomation]`, and uses
  Automation-compatible member types
- `SAFEARRAY(TypeName)` and interface pointers are part of the allowed type
  matrix

Migration implication:

- the migration must keep interface pointers, strings, variants, and arrays
  coherent as one type family rather than as isolated rewrites

### `EVENT-F1`: VBA-style COM events use connection points

- Microsoft documents that a connectable object implements
  `IConnectionPointContainer`
- `FindConnectionPoint` locates the connection point for a specific outgoing IID
- the returned `IConnectionPoint` is then used for `Advise` and `Unadvise`

Migration implication:

- canonical event-capable COM object identity must coexist with connection-point
  subscription state
- if the migration changes interface ownership, it must not break the
  `FindConnectionPoint -> Advise -> callback -> Unadvise` lifecycle

### `EVENT-F2`: A connection point is itself an `IUnknown`-derived object

- Microsoft documents that `IConnectionPoint` inherits from `IUnknown`
- its methods include:
  - `Advise`
  - `EnumConnections`
  - `GetConnectionInterface`
  - `GetConnectionPointContainer`
  - `Unadvise`

Migration implication:

- event transport has its own identity/lifetime layer, not just a callback hook
- subscription bookkeeping should be treated as native transport state, not just
  a semantic event flag

### `EVENT-F3`: `CONNECTDATA` carries sink identity plus cookie

- Microsoft documents `CONNECTDATA` as:
  - `IUnknown *pUnk`
  - `DWORD dwCookie`
- the sink is tracked through its `IUnknown` pointer
- `dwCookie` is the same token returned by `Advise` and used by `Unadvise`

Migration implication:

- connection-point state is identity-bearing native state
- on x64, the pointer-size consequence is larger than a pure 32-bit tuple; the
  local probe reported 16 bytes for a compatible sequential layout

### `EVENT-F4`: `EXCEPINFO` is the Automation exception payload

- Microsoft documents that `EXCEPINFO` is filled in by the Automation server to
  describe an exception that occurred during `Invoke`
- documented fields include:
  - `wCode`
  - `bstrSource`
  - `bstrDescription`
  - `bstrHelpFile`
  - `dwHelpContext`
  - `pvReserved`
  - `pfnDeferredFillIn`
  - `scode`
- if no exception occurred, the server must set both `wCode` and `scode` to 0

Migration implication:

- rich invoke failure behavior is not only an HRESULT question
- any internal representation migration that touches strings, variants, or
  interface transport must preserve `EXCEPINFO` extraction and ownership
  semantics

## Current OxVba Baseline Findings

### `OLD-IFACE-1`: Current `ObjPtr` is an honest stable-identity lane, not a raw leaked COM pointer claim

- [OXVBA_POINTER_HELPERS_CONTRACT_V1.md](/C:/Work/DnaCalc/OxVba/docs/spec/OXVBA_POINTER_HELPERS_CONTRACT_V1.md)
  explicitly defines `ObjPtr` in terms of stable object identity, not raw Rust
  addresses or blanket COM-pointer exposure
- host evidence in
  [pointer_helpers_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/pointer_helpers_end_to_end.rs)
  shows:
  - `ObjPtr(obj)` is non-zero for a live COM-backed object
  - repeated `ObjPtr(obj)` calls are stable
  - `ObjPtr` accepts object-valued `Variant`
  - `ObjPtr(Nothing)` returns `0`

Migration implication:

- the migrated object carrier must preserve stable identity semantics even if
  the exact internal pointer ownership changes

### `OLD-IFACE-2`: Current COM object transport is still selective about native interface truth

- [windows_variant.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_variant.rs)
  resolves object handles through `IDispatch`
- the existing code and tests already distinguish:
  - `VT_DISPATCH`
  - `VT_UNKNOWN` that can be rebound through `IDispatch`
  - unsupported plain-unknown/object cases

Migration implication:

- the current baseline is already identity-aware, but it is still centered on a
  constrained `IDispatch`-driven subset rather than a fully native canonical
  interface representation

### `OLD-EVENT-1`: Current Windows event lane already owns real connection-point subscription state

- [windows_connection_point.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_connection_point.rs)
  performs:
  - `QueryInterface(IConnectionPointContainer)`
  - `FindConnectionPoint`
  - `Advise`
  - `Unadvise`
- [windows_runtime_state.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_runtime_state.rs)
  tracks subscription transport as either:
  - `Projection`
  - `NativeConnectionPoint(WindowsConnectionPointTransport)`

Migration implication:

- the event model is already partly native in the current old implementation
- the migration must preserve this native transport ownership while aligning the
  underlying interface/value carriers

### `OLD-EVENT-2`: Current required COM event lane is dispatch-style connection-point callbacks

- repo design resolution
  [WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md](/C:/Work/DnaCalc/OxVba/docs/worksets/WORKSET_2026-03-20_IP-07_EPD_DESIGN_RESOLUTIONS.md)
  states:
  - `COM-EVT-A` is required
  - `COM-EVT-B` is tiered/deferred
- the same document ties VBA's COM event model to
  `IConnectionPointContainer` / `IConnectionPoint` with `IDispatch`-based sink
  interfaces

Migration implication:

- event compatibility for the migration should prioritize the dispatch-style
  connection-point lane first
- broader source-interface/custom-interface event parity remains a separate
  revisit point

### `OLD-EVENT-3`: Current source-interface event support is explicitly narrow

- [typelib_catalog.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/fixtures/typelib_catalog.rs)
  currently treats source-interface event support as a narrow special case:
  source interface, arity 1, and one specific test IID
- `windows_runtime_state.rs` rejects unsupported source-interface event shapes
  deterministically

Migration implication:

- source-interface event transport must be treated as a bounded discretionary
  area during migration, not as a silently complete parity surface

### `OLD-EVENT-4`: Current event payload transport is observably correct in the checked-in registered-server lane

- host evidence in
  [com_early_project_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/com_early_project_end_to_end.rs)
  shows that a registered `OxVba.TestEventServer` callback can carry the value
  payload `7` into a `WithEvents` handler
- the same lane proves the callback body executes and can surface a runtime
  error from inside the handler

Migration implication:

- the migration must preserve callback payload shape and instance routing, not
  just the ability to subscribe

### `OLD-EXCEPINFO-1`: Current invoke-failure transport already preserves rich exception payloads

- host evidence in
  [com_client_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/com_client_end_to_end.rs)
  and
  [com_early_project_end_to_end.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-host/tests/com_early_project_end_to_end.rs)
  shows deterministic propagation of:
  - `hresult`
  - `excep_scode`
  - `excep_source`
  - `excep_description`
- `windows_invoke.rs` owns extraction of BSTR-backed `EXCEPINFO` fields

Migration implication:

- any representation changes for strings/interfaces must preserve exception-path
  field ownership and cleanup, not just success-path value transport

### `OLD-LAYOUT-1`: Layout-sensitive UDT/native-ABI truth is still explicitly boundary-scoped

- [HAL_DECLARE_ABI_SPEC_V1.md](/C:/Work/DnaCalc/OxVba/docs/spec/HAL_DECLARE_ABI_SPEC_V1.md)
  keeps native ABI and pointer-string marshaling in explicit contract layers
- [HAL_DECLARE_MARSHAL_AMBIGUITIES_2026-03-02.md](/C:/Work/DnaCalc/OxVba/docs/evidence/hal/HAL_DECLARE_MARSHAL_AMBIGUITIES_2026-03-02.md)
  records open ambiguity around pointer-string ownership and mixed
  COM/dynlink-output contracts
- [SPEC_CHECKLIST.md](/C:/Work/DnaCalc/OxVba/docs/evidence/SPEC_CHECKLIST.md)
  says UDT runtime semantics are implemented for the non-boundary deterministic
  subset, with boundary interop parity deferred

Migration implication:

- layout-sensitive native/UDT closure is not part of the old baseline truth yet
- this migration should capture the facts now but still leave broader UDT/native
  layout closure to the later ABI/layout epic

## Observable Old/New Compatibility Requirements

The migrated implementation must preserve or intentionally re-document at least
the following externally observable truths:

1. object identity remains stable across repeated access and through
   object-valued variants
2. `IDispatch`/`IUnknown` rebinding rules remain consistent with COM identity
   expectations
3. dispatch-style connection-point subscription and payload delivery continue to
   work across `Advise`/callback/`Unadvise`
4. rich `EXCEPINFO` payloads remain available and correctly owned on exception
   paths
5. layout-sensitive UDT/native boundary work remains explicitly bounded where
   full parity is still deferred

## Initial Discretionary-Decision Seeds

These are not resolved by this bead, but this fact pack establishes the input
to later decisions:

1. whether canonical object/interface storage should preserve a stable
   `IUnknown` anchor directly or via a retained wrapper model
2. whether source-interface event transport beyond the current narrow lane is in
   scope for the migration or remains a post-migration extension
3. how much connection-point transport state should live inside the canonical
   object representation versus adjacent runtime subscription state
4. when layout-sensitive UDT/native closure should move from explicit boundary
   scope into the migrated canonical value model

## Evidence Commands Run

The following focused checks were run against the current old implementation on
2026-04-20:

```text
cargo test -p oxvba-host --test pointer_helpers_end_to_end windows_pointer_helper_e2e::objptr_is_stable_for_same_object_in_vm_and_jit -- --exact --nocapture
cargo test -p oxvba-host --test com_client_end_to_end dispatchinvoke_exception_details_surface_deterministically -- --nocapture
cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload -- --nocapture
```

The third test is environment-dependent because it uses the registered
`OxVba.TestEventServer` lane.

Local x64 ABI size probe run:

```text
PowerShell Add-Type probe derived from documented field layout:
CONNECTDATA_COMPAT size=16
IntPtr size=8
```

# Workset - General COM vtable calls from typelib references

**Opened:** 2026-06-19
**Status:** in-progress
**Parent context:** `WORKSET_2026-06-12_COM_VTABLE_EARLY_BOUND_DISPATCH.md`

## Goal

Make typelib-backed early-bound COM invocation a general runtime facility: imported
typelibs must compile into durable member/interface descriptors, and the Windows COM
runtime must lower eligible descriptors into safe vtable calls at execution time, with
IDispatch fallback for shapes that are dispatch-only, proxy-unsafe, or outside the
marshaller's supported ABI surface.

This is not a Cranelift/JIT workset. The callable binary shape is a Windows COM
stdcall/this-call made through the existing libffi-backed ABI engine. The runtime work
is descriptor fidelity, vtable pointer selection, argument/result marshalling, and
explicit admission/fallback policy.

## Scope Now

1. Descriptor audit and completion: keep the typelib-to-runtime projection lossless
   enough for vtable calling decisions.
2. Table-driven vtable admission: replace hidden boolean declines with structured
   reasons so each unsupported shape has an explicit fallback classification.
3. Runtime call-shape tables: document and enforce the mapping from typelib VARTYPE /
   TYPEDESC / FUNCDESC to `ComMemberSpec`, `TypeLibWireType`, vtable ABI args, and
   result cells.
4. Marshaller expansion in meaningful ABI tranches: add coherent groups of shapes
   backed by fixture tests and, where relevant, live Office/DAO evidence.
5. Verification and evidence: fixture tests for every ABI shape; live tests only where
   host safety is already bounded by QI, slot bound, proxy CLSID, and value-oracle
   evidence.

## Residual

- Full SAFEARRAY support, UDT/record types, non-Automation custom marshaling, and
  arbitrary by-ref writeback are residual unless a scoped caller requires them.
- Cranelift-generated stubs are residual and not required for correctness. They may be
  considered later for performance only after the libffi path is complete and stable.
- Foreign in-process allowlists remain evidence-driven, not assumed from typelib slots.

## Work Structure

| Phase | Outcome | Main files | Gate |
|---|---|---|---|
| 1. Descriptor inventory | Runtime specs preserve the COM facts needed for call safety | `typelib.rs`, `windows_typelib_loader.rs`, `runtime_state.rs`, `typelib_catalog.rs` | Unit tests show metadata is not dropped |
| 2. Admission table | Every vtable decline has a typed reason | `windows_invoke.rs` | Existing behavior unchanged; tests assert reason classes |
| 3. ABI mapping table | Param/result shapes have one authoritative support matrix | `windows_vtable.rs`, `windows_invoke.rs`, docs | Gate and marshaller agree for every shape |
| 4. Shape expansion | Add supported shapes one family at a time | `windows_vtable.rs`, `windows_typelib_loader.rs` | Fixture tests plus live evidence where applicable |
| 5. Runtime integration | Early-bound dispatch uses the shared mechanism without AV risk | `windows_bridge.rs`, `windows_runtime_state.rs`, host tests | IDispatch fallback remains default for ineligible calls |

## Descriptor Tables

| Typelib source | Runtime field | Meaning | Required for vtable |
|---|---|---|---|
| `TYPEATTR.guid` on source interface | `ComMemberSpec.interface_iid` | QI target for the callable interface pointer | yes |
| `TYPEATTR.typekind` | `ComMemberSpec.source_typekind` | Distinguishes custom interface from dispinterface | yes |
| `TYPEFLAG_FDUAL` | `ComMemberSpec.is_dual` | Proves dual Automation interface intent | yes |
| `FUNCDESC.oVft` | `ComMemberSpec.vtable_slot` | Slot index after dividing by live pointer size | yes |
| Partner `TYPEATTR.cbSizeVft` | `ComMemberSpec.vtable_slot_bound` | In-bounds guard before indexing vtable | yes |
| `FUNCDESC.callconv` | `ComMemberSpec.callconv_is_stdcall` | ABI convention check | yes |
| `ELEMDESC.tdesc` | `parameter_types`, `return_type` | Automation semantic type | yes |
| `TYPEDESC` / `HREFTYPE` | `parameter_wire_types`, `return_wire_type`, `parameter_iids` | Exact wire/interface shape | yes for object and non-scalar shapes |
| optional/default flags | `parameter_optional_defaults` | Synthesis for omitted trailing args | yes for omitted arg widening |

## Runtime Call Pattern

1. Resolve the member through the existing early-bound binding path.
2. Reject named arguments and interior omissions before vtable admission.
3. Compute `return_type` from invoke kind: property-get/method use the member return;
   property-put and admitted object/interface property-putref are HRESULT-only;
   unsupported putref shapes remain fallback.
4. Ask the admission table for a result. `Admit` continues; any decline falls back to
   IDispatch unless a real COM HRESULT is produced after the call starts.
5. QI the object for `interface_iid`; reject null/misaligned pointers.
6. Exclude proxy vtables whose interface proxy/stub is not PSOA.
7. Recheck `slot < bound`, then call `windows_vtable::vtable_invoke`.
8. Release the QI'd interface reference on every path.

## Current Slice

Completed code slices make phases 1 through 4 concrete for the current vtable
surface:

- `ComMemberSpec` now carries `parameter_wire_types` and `return_wire_type` from
  `TypeLibMemberMetadata`, with regression coverage in `runtime_state.rs`.
- `windows_typelib_loader.rs` preserves interface-pointer wire shape for return
  values as well as parameters, instead of collapsing interface returns to plain
  `Automation(Object)`.
- `windows_invoke.rs` introduces a typed `VtableDeclineReason` and a concrete
  `VtableInvocationPlan`; the legacy boolean gate is now only a test wrapper over
  the same plan builder.
- When wire metadata is present, the vtable gate now rejects unsupported wire shapes
  such as SAFEARRAY before the marshaller sees a collapsed semantic `Variant`.
- `windows_vtable::vtable_invoke` now accepts the admitted `VtableInvocationPlan`
  instead of loose slot/type/wire/IID arrays, and performs the same
  unsupported-wire-shape validation before reading the vtable slot so direct
  marshaller callers get the same validation fallback as the runtime gate.
- The semantic vtable ABI support table lives on `TypeLibParamType`, the wire-shape
  support table lives on `TypeLibWireType`, and `validate_vtable_wire_signature` is
  the shared descriptor-level validator used by both the runtime gate and the vtable
  marshaller. Object-parameter IID validation now happens through that same path
  before resolving objects or reading slots.
- Explicit `InterfacePointer` wire metadata for object return values is admitted
  and fixture-proven through the vtable marshaller; adjacent unsupported return
  wire shapes such as SAFEARRAY still decline to IDispatch fallback.
- The first broad ABI tranche is fixture-proven through real vtable calls:
  inbound `Byte`, `Integer`, `Long`, `LongLong`, `Single`, `Double`, `Currency`,
  `Date`, `Boolean`, `String`, `Variant`, and explicit interface-pointer `Object`
  parameters; return cells for `Byte`, `Integer`, `LongLong`, `Single`, `Double`,
  `String`, and `Variant`; with the existing `Long`, `Boolean`, `Currency`,
  `Date`, and `Object` return tests covering the rest of the current v1 surface.
- `PropertyPutRef` is no longer globally deferred. The admission table now admits
  the covered object/interface assignment shape only when the typelib descriptor
  carries explicit `InterfacePointer` wire metadata and a non-null parameter IID;
  every other putref shape declines once in the plan builder and falls back to
  IDispatch.
- The shared runtime dispatch path has an integration proof for that putref
  boundary: supported object/interface putref increments the vtable transport
  counter, while scalar putref declines and succeeds through the IDispatch fallback.
- The widened `ComMemberSpec` is boxed in sparse enum variants that only sometimes
  carry a spec, keeping clippy's large-enum guard clean without weakening lint policy.

Residual after this slice: SAFEARRAY, records/UDTs, arbitrary ByRef/writeback,
Decimal, and non-object putref remain explicit fallback until their ABI and
ownership semantics are implemented with the same fixture and live-evidence discipline.

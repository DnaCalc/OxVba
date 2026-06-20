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

- SAFEARRAY, Decimal, ByRef/writeback, and UDT/record support are active accepted
  scope for this workset. They must not be pre-excluded as "low-risk later" work;
  fallback is valid only for a specific missing fact or unsupported ownership case
  such as foreign/unknown record layout, malformed SAFEARRAY descriptors, missing
  object/interface IID, unsafe proxy boundaries, or unsupported custom marshaling.
- Non-Automation custom marshaling with no recoverable public layout/ownership
  facts remains fallback until those facts are recovered and fixture-proven.
- Cranelift-generated stubs are residual and not required for correctness. They may be
  considered later for performance only after the libffi path is complete and stable.
- Foreign in-process allowlists remain evidence-driven, not assumed from typelib slots.

## Review Instructions

After each commit-sized tranche:

1. Run the relevant checks for the touched runtime path.
2. Perform a fresh-eyes review that actively looks for blunders, mistakes, errors,
   oversights, omissions, logical issues, misconceptions, confusion, bugs,
   regressions, hidden assumptions, and compatibility/parity gaps.
3. Rework every material issue found by the review.
4. Rerun the relevant checks after rework.
5. Repeat the fresh-eyes review and rework loop until no material issue remains.
6. Only then update docs/status, commit the tranche, push it, and continue.

Partial subsets must not be described as implemented, closed, complete, or parity
unless the scoped parity claim is actually proven by tests or evidence. A useful
subset remains `in-progress` when the broader accepted COM vtable surface still
has unowned ABI or descriptor facts.

## Work Structure

| Phase | Outcome | Main files | Gate |
|---|---|---|---|
| 1. Descriptor inventory | Runtime specs preserve the COM facts needed for call safety | `typelib.rs`, `windows_typelib_loader.rs`, `runtime_state.rs`, `typelib_catalog.rs` | Unit tests show metadata is not dropped |
| 2. Admission table | Every vtable decline has a typed reason | `windows_invoke.rs` | Existing behavior unchanged; tests assert reason classes |
| 3. ABI mapping table | Param/result shapes have one authoritative support matrix | `windows_vtable.rs`, `windows_invoke.rs`, docs | Gate and marshaller agree for every shape |
| 4. Shape expansion | Add meaningful ABI tranches across SAFEARRAY, Decimal, ByRef/writeback, and UDT/record shapes | `windows_vtable.rs`, `windows_typelib_loader.rs` | Fixture tests plus live evidence where applicable; fallback only for specific missing facts |
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
   property-put and admitted property-putref shapes are HRESULT-only; unsupported
   putref shapes remain fallback.
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
  before the marshaller sees a collapsed semantic `Variant`.
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
  and fixture-proven through the vtable marshaller.
- Explicit `SafeArrayVariant` wire metadata is now admitted for semantic
  `Variant` parameters and returns. The vtable marshaller lowers inbound array
  arguments to `SAFEARRAY*` through the existing Windows `VARIANT`/SAFEARRAY
  conversion helper, and decodes transferred retval `SAFEARRAY*` payloads through
  a temporary owning `VARIANT` so OLE SAFEARRAY ownership is released correctly.
- SAFEARRAY wire metadata is no longer implicitly `VT_VARIANT`-only. The live
  typelib loader preserves `ARRAYDESC.tdescElem.vt` as explicit
  `SafeArray { element_vt }` / `ByRefSafeArray { element_vt }` metadata when the
  descriptor supplies a typed element VARTYPE, and malformed SAFEARRAY descriptors
  preserve an unsupported zero VARTYPE rather than inferring `VT_VARIANT`.
- Typed SAFEARRAY parameters, ByRef SAFEARRAY writebacks, and SAFEARRAY retvals
  now validate their actual COM element VARTYPE against the admitted wire plan
  before calling or decoding. `SAFEARRAY(I4)` is fixture-proven through real
  vtable inbound and retval slots, while the existing `SAFEARRAY(VARIANT)` tests
  remain a distinct wire-shape proof.
- `Decimal` return values are admitted and fixture-proven as caller-owned
  `[out,retval] DECIMAL*` cells decoded into the runtime `Decimal96` carrier.
  Decimal inbound parameters are also admitted and fixture-proven through explicit
  `Automation(Decimal)` metadata, lowered as caller-owned `DECIMAL*` cells that
  stay alive for the duration of the vtable call.
- The first broad ABI tranche is fixture-proven through real vtable calls:
  inbound `Byte`, `Integer`, `Long`, `LongLong`, `Single`, `Double`, `Currency`,
  `Date`, `Boolean`, `String`, `Variant`, and explicit interface-pointer `Object`
  parameters; return cells for `Byte`, `Integer`, `LongLong`, `Single`, `Double`,
  `String`, and `Variant`; with the existing `Long`, `Boolean`, `Currency`,
  `Date`, and `Object` return tests covering the rest of the current v1 surface.
- `PropertyPutRef` is no longer globally deferred. The admission table now admits
  putref through the shared signature table: object/interface assignment still
  requires explicit `InterfacePointer` wire metadata and a non-null parameter IID,
  while scalar Automation putref shapes use the same ABI rules as property-put.
- The shared runtime dispatch path has integration proof for both putref families:
  supported object/interface putref and scalar `Long` putref increment the vtable
  transport counter and do not also dispatch through IDispatch.
- `ComInvokeArg` can now carry a `RuntimeByRefSlot`, and the vtable marshaller has
  a writeback-capable executor returning `VtableInvokeResult { value, writebacks }`.
  The value-only wrapper rejects unexpected writebacks instead of discarding them.
- ByRef scalar, Decimal, and Variant cells are fixture-proven through real vtable
  calls. The broad covered family is `ByRefInteger`, `ByRefByte`, `ByRefBoolean`,
  `ByRefLong`, `ByRefLongLong`, `ByRefSingle`, `ByRefDouble`, `ByRefCurrency`,
  `ByRefDate`, `ByRefDecimal`, and `ByRefVariant`. Admission requires a concrete
  runtime writeback slot; missing slots decline before any slot call.
- The existing value-only shared dispatch path still declines supplied ByRef
  arguments before vtable execution because its API cannot return
  `RuntimeCallResult.writebacks`. That guard remains intentional so value-only
  callers never silently lose mutations.
- The shared runtime bridge now has an opt-in `RuntimeCallResult` execution path
  for early-bound COM calls. Eligible ByRef vtable calls propagate
  `RuntimeCallResult.writebacks` through the bridge and fixture-prove transport
  counter behavior. The value-only path remains unchanged and still declines
  ByRef vtable execution before mutation; the writeback-capable path refuses
  ByRef fallback through value-only IDispatch rather than silently dropping
  mutations.
- The remaining ByRef Automation families are now admitted and fixture-proven
  through the writeback-capable vtable marshaller: `ByRefString` uses BSTR*
  ownership, `ByRefObject` requires explicit `InterfacePointer` wire metadata
  and a declared IID, `ByRefLongPtr` uses pointer-width cells on x64, and
  explicit `ByRefSafeArrayVariant` wire metadata passes SAFEARRAY** cells and
  decodes the final SAFEARRAY payload into the runtime array carrier.
- `TKIND_RECORD` / `VT_USERDEFINED` records now survive typelib descriptor
  projection as explicit `TypeLibParamType::Record` / `ByRefRecord` plus
  `TypeLibWireType::Record` / `ByRefRecord` metadata instead of collapsing to
  generic `Variant` or `Object` facts.
- The runtime now has an opaque `ComRecord` carrier for COM `VT_RECORD` payloads.
  Windows VARIANT conversion clones records through `IRecordInfo::RecordCreateCopy`,
  releases them through `IRecordInfo::RecordDestroy`/`Release`, and can write a
  record value back into a Windows `VARIANT` as a distinct owned record copy.
  A fake `IRecordInfo` fixture proves the clone, transfer, and destruction
  ownership path without depending on a foreign typelib.
- Typed inbound record parameters are admitted when explicit `Record` wire
  metadata is present. The vtable marshaller borrows the `ComRecord` data pointer
  for the duration of the call while the runtime carrier keeps the payload alive,
  and a real fixture vtable slot proves the pointer reaches the callee as record
  data rather than a collapsed `VARIANT` or object pointer.
- Typed ByRef record parameters are admitted when explicit `ByRefRecord` wire
  metadata and a runtime writeback slot are present. The vtable marshaller passes
  a deep-cloned mutable record payload to the callee and returns the mutated record
  through `RuntimeByRefWriteback`, fixture-proven by a real slot that mutates the
  record data in place and by the shared runtime bridge `dispatch_invoke_call_result`
  path using vtable transport without an IDispatch fallback.
- Live typelib record wire metadata now carries optional record allocation identity
  (`LIBID`, version, LCID, record type GUID). Descriptor-backed record retvals are
  admitted and the vtable executor allocates caller-owned record storage through
  OleAut `GetRecordInfoFromGuids`/`IRecordInfo::RecordCreate`; name-only record
  metadata still declines with an explicit missing-record-return-info reason. The
  record retval path is fixture-proven with a temporary registered OleAut typelib
  that supplies a real `IRecordInfo`: the vtable slot writes into the allocated
  record cell and the runtime returns the populated `ComRecord` carrier.
- The widened `ComMemberSpec` is boxed in sparse enum variants that only sometimes
  carry a spec, keeping clippy's large-enum guard clean without weakening lint policy.

Residual after this slice: record/UDT live-typelib breadth still needs external
evidence across more than the controlled single-field OleAut fixture before claiming
foreign-record parity. Name-only records may fall back only with the specific missing
allocation metadata fact, and any remaining record fallback must identify the exact
unowned ABI, layout, registration, or ownership rule. SAFEARRAY record-element
metadata remains an explicit unsupported wire shape until record-array allocation,
copy, and destruction ownership are implemented and fixture-proven; malformed
SAFEARRAY descriptors now decline rather than guessing `VT_VARIANT`.

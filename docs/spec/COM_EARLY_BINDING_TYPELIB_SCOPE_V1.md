# COM Early Binding and Type Library Support V1

Status: `working-draft`
Date: 2026-03-05
Primary scope: Windows (`HalProfileId::Windows`) for native COM early binding; deterministic unsupported behavior on non-Windows profiles.
Companion docs:
- `docs/spec/COM_CLIENT_SERVER_SCOPE_V1.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_TYPELIB_IMPORTLIB_HAL_DRAFT_V1.md`
- `docs/spec/COM_EARLY_BINDING_TYPELIB_CONFORMANCE_V1.md`

Implementation snapshot (`v417..v426`):
- PMR now supports type-library identity hints (`importlib/libid/version/lcid`) and deterministic bind statuses for unresolved/ambiguous identity outcomes.
- HAL now exposes `TypeLibraryHal` for resolve/load/invalidate operations, with Windows deterministic subset implementation and deterministic unsupported behavior on non-Windows profiles.
- Compiler project-lowering now supports a constrained early-bound bridge that rewrites:
  - `Dim x As Lib.Type` -> `Dim x As Object`,
  - `Dim x As New Lib.Type` -> object declaration plus deterministic `CreateObject` selector initialization,
  - `x.Member(...)` for supported member subset -> `DispatchInvoke` tokenized calls.
- Runtime execution for this tranche intentionally reuses existing late-bound COM transport (`CreateObject`/`DispatchInvoke`) to preserve deterministic behavior while full early-bound runtime lanes are still staged.

## 1. Objective

Define a rigorous, implementation-ready design for:

1. consuming COM type libraries,
2. integrating type library references into OxVba ProjectGraph/PMR,
3. supporting early-bound type/member binding in parser+binder+IR+runtime,
4. preserving deterministic diagnostics and policy behavior,
5. preparing formal verification and conformance pipelines.

Priority order follows `CHARTER.md` and `MACH1000_PLAN.md`:

1. robustness,
2. compatibility,
3. performance.

## 2. Primary source set (online + local canonical mirror)

### 2.1 Microsoft Open Specifications / API references

- MS-VBAL (language semantics, object typing, project references):
  - https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal
- MS-OAUT (Automation protocol, ITypeLib/ITypeInfo/IDispatch):
  - https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut
  - IDispatch::Invoke opnum 6: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/5c2a1997-60d7-496d-8d9a-ed940bbb82eb
  - IDispatch::GetIDsOfNames opnum 5: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/fc4fbe2b-c49d-4ea2-a7a7-8f8a28e487ef
  - ITypeLib::GetTypeInfoOfGuid opnum 8: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/e56684ba-14c8-4699-ae08-0be1f73894c8
  - ITypeLib::FindName opnum 10: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/c7ce7eb4-7c99-48d9-8582-959487cb1971
  - ITypeInfo::GetRefTypeOfImplType opnum 13: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/f6de8a6d-93d2-4edb-8692-551b75b2d814
  - ITypeInfo::GetRefTypeInfo opnum 17: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/327b35e0-af43-4857-8fb6-44c72d5d5f20
- MS-OVBA (VBA project storage and reference records):
  - overview/index: https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba
  - ProjectReferences section index: https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/4bb8612c-6ffa-4ec5-8d26-6f4f84254088
  - REFERENCEREGISTERED record: https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/49e142e2-3e44-4a2f-a3ee-47020da76265
  - REFERENCEPROJECT record: https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/98299efc-8eb4-4879-8174-c275886499b5
- Win32 OleAut APIs for loading/registration behavior:
  - LoadRegTypeLib: https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-loadregtypelib
  - LoadTypeLibEx: https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-loadtypelibex
  - RegisterTypeLib: https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-registertypelib
- VBA host reference APIs (test harness and project integration):
  - References.AddFromGuid: https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/addfromguid-method-vba-add-in-object-model
  - References.AddFromFile: https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/addfromfile-method-vba-add-in-object-model
  - Reference.IsBroken: https://learn.microsoft.com/en-us/office/vba/api/access.reference.isbroken

### 2.2 Local canonical extracted anchors

Canonical roots:

- `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/`
- `../Foundation/reference/runs/20260301-ms-oaut-pass02/outputs/`
- `../Foundation/reference/runs/20260301-ms-ovba-pass01/outputs/` (currently sparse extraction; direct spec pages used where extractor coverage is shallow)

Anchor families to use directly in implementation and conformance mapping:

- MS-VBAL:
  - `SPEC-discovered-ms-vbal-250520-f945507e-01489` (`as-auto-object = "as" "new" class-type-name`)
  - `SPEC-discovered-ms-vbal-250520-f945507e-01497` (`as-auto-object` type MUST be named class)
  - `SPEC-discovered-ms-vbal-250520-f945507e-01498` (Public Not Creatable constraint)
  - `SPEC-discovered-ms-vbal-250520-f945507e-01229`, `...-01230` (ordered project references and precedence)
  - `SPEC-discovered-ms-vbal-250520-f945507e-05318` (CreateObject signature)
- MS-OAUT:
  - `CONF-discovered-ms-oaut-240423-b76f9b41-0123` (`TYPEFLAG_FDUAL`)
  - `CONF-discovered-ms-oaut-240423-b76f9b41-0125` (`TYPEFLAG_FOLEAUTOMATION`)
  - `CONF-discovered-ms-oaut-240423-b76f9b41-0708..0718` (`ITypeComp::Bind` result-shape obligations)
  - `CONF-discovered-ms-oaut-240423-b76f9b41-0851` (`GetRefTypeOfImplType` + `GetRefTypeInfo` for interface relationships)
  - `CONF-discovered-ms-oaut-240423-b76f9b41-1023` (`GetTypeInfoOfGuid`)
  - `CONF-discovered-ms-oaut-240423-b76f9b41-1024` (`FindName`)

## 3. Scope and non-goals

### In scope (this planning series)

1. Early-bound object type resolution from project references/type libraries.
2. Binder-level member lookup against type library metadata.
3. IR/runtime contract for early-bound invocation lanes.
4. Dual-interface policy (prefer vtable lane where safe; controlled fallback to IDispatch where required).
5. Deterministic diagnostics for missing/broken/ambiguous references and member mismatch.
6. Type library cache/load/invalidation model.
7. Formal/property/conformance plan and test fixture architecture.

### Out of scope (deferred)

1. Full COM server-side early-bound interface publication parity for class modules.
2. Cross-process DCOM/remoting semantics.
3. Non-Windows native COM early binding (explicit deterministic unsupported).
4. Complete Office host parity on every obscure automation edge case before baseline closure.

## 4. Terminology and model boundaries

- Early binding: compile-time resolution to a concrete member signature from type information.
- Late binding: runtime `IDispatch::GetIDsOfNames` + `Invoke` member resolution.
- Dual interface: interface exposing both vtable contract (`TYPEFLAG_FDUAL`) and dispatch surface.
- Type library identity: tuple `(LIBID, major, minor, LCID, syskind, source provenance)`.
- PMR reference: project-level reference entry (project, host-injected, type-library kinds).

Boundary ownership:

- PMR / ProjectGraph owns reference declarations, ordering, and binding states.
- HAL owns platform access for type library discovery/load and COM activation.
- Compiler binder owns symbol/type/member resolution and typed call lowering decisions.
- VM/runtime owns execution of the lowered call plan and deterministic error propagation.

## 5. Project reference integration design

## 5.1 Reference representation

Extend type-library reference representation to carry stable identity and provenance:

- `reference_name`
- `importlib_hint` (already present)
- optional `libid`
- optional `major/minor`
- optional `lcid`
- source/provenance (`registered`, `file`, `embedded`, `host-injected`)
- bound fingerprint hash.

Compatibility rule:

- keep `importlib_hint` path as the deterministic floor already implemented,
- enrich only as additive fields so existing tests stay valid.

## 5.2 Resolution precedence

Resolution order (Windows profile):

1. explicit `(libid, major, minor, lcid)` if provided,
2. explicit importlib hint,
3. host-provided deterministic reference descriptor map,
4. no silent fallback beyond declared profile policy.

Default strict mode:

- unresolved, ambiguous, or broken references are compile-time binding failures for early-bound members.

Implementation-defined mode (planned):

- optionally allow runtime deferred resolution with deterministic runtime failure if unresolved.

## 5.3 Broken reference behavior

Broken reference criteria include:

- registry entry exists but typelib cannot be loaded,
- requested version cannot be resolved by policy,
- typelib loads but required type/member cannot be resolved.

Expose deterministic diagnostic families (`PMR-E-TYPELIB-*`, `BIND-E-TYPELIB-*`) and map to `Err` bridge where runtime path exists.

## 6. Type library ingestion and cache model

## 6.1 HAL contract extension (planned)

Add Windows-only HAL surface for typelib access:

```rust
trait TypeLibraryHal {
    fn resolve_typelib_reference(&self, request: &TypeLibResolveRequest) -> HalResult<TypeLibResolvedIdentity>;
    fn load_typelib_metadata(&self, identity: &TypeLibResolvedIdentity) -> HalResult<TypeLibMetadataBlob>;
    fn invalidate_typelib_cache(&self, scope: TypeLibCacheScope) -> HalResult<()>;
}
```

Non-Windows behavior:

- deterministic `unsupported` for these calls.

## 6.2 Metadata blob schema (logical)

Metadata contains enough to bind members without live COM object creation:

- type library attributes (`TLIBATTR` projection),
- type records (interfaces, dispinterfaces, coclasses, enums, aliases, records),
- method records (DISPID, invoke kind, params, return type, optional/default metadata),
- implemented interface graph (`GetRefTypeOfImplType` / `GetRefTypeInfo` closure),
- dual flags and automation compatibility flags.

## 6.3 Caching and invalidation

Cache key:

- normalized identity tuple + source provenance + load policy version.

Invalidation triggers:

1. explicit user/build command (`--refresh-typelib-cache` planned),
2. reference list mutation,
3. fingerprint mismatch (registry/path metadata drift),
4. cache schema version bump.

Determinism requirement:

- cache hit/miss must not change semantic result; only performance.

## 7. Parser, binder, and type system integration

## 7.1 Syntax and name resolution

Target syntax families:

1. `Dim x As MyLib.MyThing`
2. `Dim x As New MyLib.MyThing`
3. `Dim x As New MyThing` where `MyThing` is imported from a referenced typelib/project and not ambiguous.
4. existing runtime intrinsics remain available for controlled fallback lanes.

Parsing note:

- parser mostly already supports qualified names; binder must classify typelib-qualified class names distinctly from project module/class names.

## 7.2 Binder pipeline additions

1. Build external type namespaces from resolved type libraries and project reference precedence.
2. Resolve declared types to one of:
   - project class,
   - external coclass/interface type,
   - ambiguous/error.
3. Resolve member accesses/calls for early-bound typed receivers using metadata tables.
4. Emit typed call plan with explicit dispatch strategy and expected signature/coercion plan.

## 7.3 Early/late bridge strategy

- If receiver static type is specific external interface/class and binding succeeded: early-bound lane.
- If static type is `Object` or unresolved dynamic shape: late-bound lane.
- Dual interface policy:
  - default `prefer_vtable` in strict performance profile,
  - `dispatch_fallback_allowed` in compatibility profile when vtable mapping unavailable.

All branches must preserve deterministic errors and auditable trace metadata.

## 8. IR and runtime design

## 8.1 New IR intent (logical)

Add IR forms that separate semantic intent from call mechanism:

- `ComEarlyBindResolveType`
- `ComEarlyBindConstruct`
- `ComEarlyBindInvoke`
- `ComEarlyBindGet/Set`

Each carries:

- resolved type/member identity,
- coercion plan id,
- dispatch strategy (`vtable`, `idispatch`, `dual-adaptive`),
- diagnostic fallback map.

## 8.2 Runtime execution contracts

Preconditions:

- early-bind descriptor exists and matches runtime object type constraints.

Postconditions:

- success returns value consistent with declared signature/coercion plan.
- failure maps to deterministic diagnostic family with stable code/source/member context.

No hidden fallback rule:

- fallback from vtable to dispatch can occur only when policy explicitly allows and is traceable.

## 9. Diagnostics and error handling

Diagnostic families to introduce/extend:

- `PMR-E-TYPELIB-REFERENCE-*` (reference acquisition/loading failures)
- `BIND-E-TYPELIB-TYPE-NOT-FOUND`
- `BIND-E-TYPELIB-MEMBER-NOT-FOUND`
- `BIND-E-TYPELIB-MEMBER-AMBIGUOUS`
- `BIND-E-TYPELIB-SIGNATURE-MISMATCH`
- `RUN-E-COM-EARLYBIND-*` (runtime contract failures)

Design rule:

- every HRESULT/protocol error path gets stable OxVba code + contextual payload.

## 10. Formal verification and model-checking plan

## 10.1 Formal properties

1. Deterministic binding:
   - same source + same reference set + same cache snapshot -> same resolved member identity.
2. Precedence monotonicity:
   - changing reference order only affects ambiguous competing symbols as per precedence rules.
3. Signature safety:
   - runtime arg marshaling never violates resolved parameter count/byref/optional constraints for supported subset.
4. Dual strategy coherence:
   - vtable and dispatch lanes (where both valid) agree on member identity and arity constraints.
5. Cache soundness:
   - invalidation events cannot return stale metadata as fresh.

## 10.2 Tooling lanes

- Kani harnesses (deferred-gate eligible, non-blocking by policy):
  - binder determinism for reduced state spaces,
  - precedence and ambiguity transitions,
  - marshaling precondition guards.
- Property tests:
  - cache key/invalidation invariants,
  - deterministic diagnostic mapping totality.
- Miri/unsafe checks:
  - COM pointer/type metadata handling once vtable lane lands.
- Lean-ready model sketch (phase artifact):
  - minimal formal relation for `resolve_member(project_refs, typelibs, receiver_type, member_name)`.

## 11. Conformance and test architecture

## 11.1 Test server and typelib strategy

Build/extend controlled COM test components with attached type library containing:

1. pure dispatch interface,
2. dual interface,
3. versioned type changes (minor-compatible and breaking forms),
4. optional/default/byref parameter cases.

Planned lane split:

- registration-free deterministic lane (preferred CI lane),
- registered realism lane.

## 11.2 End-to-end scenarios

1. compile-only early bind diagnostics for missing references.
2. compile+run early-bound method/property calls.
3. `Dim x As New MyLibrary.MyThing` auto-instantiation path.
4. dual interface parity checks (vtable vs dispatch strategy).
5. cache invalidation behavior under typelib version/path mutation.
6. project reference precedence interactions between project classes and typelib classes.

## 12. Performance strategy

Planned optimizations (post-correctness lock):

1. typelib metadata interning and shared immutable blobs,
2. stable member lookup indexes keyed by `(type_id, name, invoke_kind)`,
3. call-site inline cache for early-bound member handles,
4. optional ahead-of-time binding manifest generation for repeated builds.

## 13. Uncertainties and implementation-defined registry additions

Track in:

- `docs/evidence/hal/HAL_IMPLEMENTATION_DEFINED.md`
- `docs/evidence/hal/HAL_UNCERTAINTY_REGISTER.md`
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`

New topics to register:

1. exact `LoadRegTypeLib` version fallback alignment vs Office host behavior.
2. fallback policy from early-bound to late-bound in dual interface failures.
3. `ServerName` behavior interactions with early-bound creation pathways.
4. typelib identity normalization details (case, path canonicalization, locale).

## 14. Three cross-reference iterations (completed in planning pass)

## 14.1 Iteration A: protocol/source grounding

Checks performed:

- MS-VBAL object typing and auto-object clauses,
- MS-OAUT interface/type library protocol obligations,
- Win32 API loading semantics.

Resulting plan changes:

- explicit dual-interface policy section,
- explicit cache identity tuple and invalidation triggers,
- explicit `ITypeComp`/`GetTypeInfoOfGuid`/`FindName` use in binder ingest pipeline.

## 14.2 Iteration B: current OxVba architecture alignment

Checks performed:

- existing HAL COM late-bound contract,
- PMR current `importlib_hint` flow and diagnostics,
- compiler/VM intrinsic surfaces.

Resulting plan changes:

- additive evolution from existing `importlib_hint` floor,
- no forced rewrite of intrinsic late-bound path,
- explicit IR intent layer between binder and runtime to avoid semantic drift.

## 14.3 Iteration C: host/reference operational behavior

Checks performed:

- VBA `References.AddFromGuid/AddFromFile` and `Reference.IsBroken`,
- MS-OVBA reference record structure pages,
- test harness implications for registered/registration-free lanes.

Resulting plan changes:

- explicit broken-reference model,
- explicit test lane split and cache invalidation scenarios,
- explicit requirement for deterministic compile-time diagnostics under strict mode.

## 15. Decision summary

1. Build early binding on top of PMR+HAL reference resolution, not ad hoc runtime probing.
2. Keep late-bound path as explicit sibling capability; do not conflate semantics.
3. Treat dual interfaces as policy-controlled strategy, not hidden fallback.
4. Introduce type library metadata ingestion + cache with deterministic invalidation.
5. Gate behavior with formal/property/conformance evidence before broad compatibility claims.

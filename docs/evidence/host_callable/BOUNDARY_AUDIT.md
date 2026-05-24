# Host Callable Boundary Audit

Date: 2026-05-24
Bead: `bd-hjys.1`
Workset: `docs/worksets/WORKSET_2026-05-24_HOST_PROJECT_CALLABLE_REFLECTION_AND_WRAPPER_GENERATION_REWORK.md`

## Scope

This audit inventories the current host-UDF, host-call, XLL, native-wrapper, and
wrapper-generation surfaces before the first-pass no-compatibility rework. The
workset rule is binding here: deprecated old-shape code is to be deleted or
directly replaced, not preserved through compatibility adapters.

## Inventory commands

```powershell
rg -n "HostUdf|host_udf|host-call|host_call|BundleHostCall|HostCall|UDF|udf|XLL|xll|WrapperGeneration|wrapper generation|WrappedNativeLibrary|native export|NativeExport|Generate.*wrapper|wrapper" crates docs scripts -g '*.rs' -g '*.md' -g '*.ps1' > temp/bd-hjys1-rg-inventory.txt
rg -n "HostUdf|host_udf|BundleHostCall|HostCall" crates -g '*.rs'
rg -n "xll|XLL|WrappedNativeLibrary|BuildTarget|Wrapper|generate_.*shim|native export|NativeExport|HostCallDescriptor|DescriptorInventory" crates/oxvba-build crates/oxvba-compiler crates/oxvba-project crates/oxvba-cli -g '*.rs'
```

Observed counts from the audit pass:

| Query | Count |
| --- | ---: |
| Broad host/wrapper inventory | 2003 |
| `HostUdf` / `host_udf` code refs | 114 |
| Runtime/host `HostCall` refs | 109 |
| Bundle host-call refs | 26 |
| XLL build refs | 197 |
| Wrapper/native build refs | 223 |

The broad count includes historical docs and evidence. The classification below
focuses on live source and active workset/evidence surfaces that can affect the
rework.

## Boundary classifications

### A. Neutral facts to keep and generalize

| Surface | Current location | Current role | Classification | Replacement / destination |
| --- | --- | --- | --- | --- |
| Procedure runtime metadata | `crates/oxvba-compiler/src/emit.rs`, `crates/oxvba-compiler/src/project.rs` | Carries module/procedure names, entry PCs, param slots/types, return slots/types. | Neutral compiler/runtime fact. | Feed `ProcedureDescriptor` / `ProcedureSignature` without UDF policy. |
| `HostProcedureExport` | `crates/oxvba-compiler/src/project.rs` | Public procedural export inventory for host-facing procedures. Currently includes Functions and Subs. | Mostly neutral fact, but name is host-call oriented. | Replace or project into neutral public procedure descriptors; keep Function/Sub distinction. |
| `RuntimeCallFrame`, `RuntimeCallSelector::HostCall`, `RuntimeCallKind::HostCall` | `crates/oxvba-runtime/src/call_frame.rs` | Generic call frame and host-call selector. | Neutral runtime primitive. | Rename/refine to neutral callable invocation if needed; keep general call-frame semantics. |
| `NativeExportDescriptor` type metadata | `crates/oxvba-project/src/model.rs`, `crates/oxvba-project/src/validate.rs` | Explicit build-profile export selection and validated ABI types. | Wrapper/build policy plus neutral signature facts. | Consume via wrapper-generation selection plans; do not treat as UDF. |
| COM class/member descriptors | `crates/oxvba-compiler/src/bundle.rs`, `crates/oxvba-project/src/validate.rs`, `crates/oxvba-build/src/typelib_gen.rs` | COM-specific descriptor and typelib data. | Boundary-specific wrapper facts. | Keep as COM wrapper plan input; share neutral procedure signature substrate where possible. |

### B. Misplaced host/UDF policy to remove from compiler/runtime/bundle

| Surface | Current location | Problem | Proposed destination |
| --- | --- | --- | --- |
| `BundleHostCallDescriptor.selection_policy = "public-procedural-functions"` | `crates/oxvba-compiler/src/bundle.rs` | Compiler/bundle descriptor invents a host selection policy and mixes Function/Sub inventory under function-oriented wording. | Remove from neutral bundle descriptor; selection belongs in `CallableSelectionPlan` or host policy. |
| `BundleHostCallDescriptor.category`, `description`, `argument_descriptions`, `volatile`, `dependency_policy`, `side_effect_policy`, `thread_safety_policy`, `allowed_contexts` | `crates/oxvba-compiler/src/bundle.rs` | Bundle fabricates worksheet/UDF policy: `worksheet-cell`, `host-formula-evaluator`, `single-threaded-vba-compatible`, etc. | Explicit source annotations may stay neutral; synthesized host policy moves to host-owned UDF example or wrapper profile. |
| `RuntimeCallSource::HostUdf` | `crates/oxvba-runtime/src/call_frame.rs` | Runtime knows a UDF-specific source. | Replace with neutral host-call source/provenance, e.g. `RuntimeCallSource::HostCallable` or context metadata. |
| Host-call descriptor truth restored indirectly from `export_inventory.host_exports` | `crates/oxvba-host/src/engine.rs` | Bundle descriptor inventory exists but prepared sessions rebuild host-UDF descriptors from host exports and hardcoded policy. | Bundle-loaded reflection should consume neutral descriptor inventory as source of truth. |

### C. Old-shape HostUdf code to delete or directly replace

| Surface | Current location | Problem | Direct replacement |
| --- | --- | --- | --- |
| `HostUdfCatalog` | `crates/oxvba-host/src/engine.rs`, re-exported from `crates/oxvba-host/src/lib.rs` | Host-facing API hardcodes UDF terminology and Function-only policy. | `ProjectReflection` / `CallableCatalog` with host-owned filtering. |
| `HostUdfFunctionDescriptor` and nested registration/callable/invocation/capability DTOs | `crates/oxvba-host/src/engine.rs` | Mixes neutral descriptor facts with OxFunc/W093 and worksheet-like policy. | `ProcedureDescriptor`, `ProcedureSignature`, `CallableCapability`; W093 projection in host-owned example only. |
| `HostUdfCallContext` | `crates/oxvba-host/src/engine.rs` | UDF-specific name; context is only echoed by result and the built frame is discarded. | `HostCallContext` delivered to callable invocation path or documented host-service observation point. |
| `HostUdfTypedValue`, `HostUdfTypedSignature`, `HostUdfTypedInvokeResult`, `HostUdfTypeMapEvidence` | `crates/oxvba-host/src/engine.rs` | Typed invocation is framed as UDF-specific and Double-only first slice. | Neutral `TypedValue`, `CallableTypedSignature`, `TypedInvocationResult`, conversion-lane evidence. |
| `Engine::host_udf_catalog` | `crates/oxvba-host/src/engine.rs` | Rebuilds descriptors from exports and hardcoded policy. | `VbaProject::reflect()` / `PreparedProject::reflection()` over neutral descriptors. |
| `Engine::invoke_host_udf_with_variants`, `Engine::invoke_host_udf_typed` | `crates/oxvba-host/src/engine.rs` | UDF-specific API; context frame is not delivered. | `invoke_callable_with_variants` / `invoke_callable_typed`. |
| `host_udf_*` helper functions | `crates/oxvba-host/src/engine.rs` | Generate descriptor fingerprints and registration identities under UDF semantics. | Neutral descriptor/fingerprint helpers plus host-owned W093 projection example. |
| Host-UDF tests | `crates/oxvba-host/tests/invoke_procedure_tests.rs` | Assert old API names and synthesized policy fields. | Rewrite to neutral reflection/invocation tests and separate host-owned UDF policy tests. |
| Host-UDF evidence docs | `docs/evidence/HOST_UDF_W093_METADATA_DESCRIPTOR_2026-05-22.md`, `docs/evidence/conformance/WRAPPED_COM_SERVER_HOST_UDF_*.md` | Historical evidence for old shape. | Mark superseded or remove from active truth during evidence refresh; keep only if archived as history. |

### D. Wrapper/build-target surfaces to refactor onto wrapper plans

| Surface | Current location | Current role | Classification | Rework destination |
| --- | --- | --- | --- | --- |
| `BuildTarget::{WrapperExe, WrapperLibrary, WrappedComServer}` | `crates/oxvba-project/src/model.rs`, parse/generate/load/CLI | Project-level wrapper output selection. | Wrapper policy. | Keep names as build profiles only if backed by `WrapperGenerationPlan`; avoid compiler policy. |
| `OutputType=Addin` / `.xll` defaulting | `crates/oxvba-cli/src/main.rs`, `crates/oxvba-project/src/load.rs` | Routes Addin projects to XLL packaging. | XLL wrapper policy currently too direct. | Refactor as future XLL wrapper profile over generic wrapper plans. |
| `generate_exe_shim` | `crates/oxvba-build/src/exe.rs` | Generates simple OXB runner executable. | Wrapper generator. | Add introspection-printer/reflection-caller EXE as first generic wrapper example. |
| `generate_dll_shim` | `crates/oxvba-build/src/dll.rs` | Generates native exports from `NativeExportDescriptor`. | Wrapper generator with explicit native ABI policy. | Refactor over `CallableSelectionPlan` + native thunk conversion lanes. |
| `generate_com_server_shim` / `generate_com_exe_shim` | `crates/oxvba-build/src/comserver.rs`, `comserver_exe.rs` | Generates COM wrapper artifacts. | COM wrapper policy. | Consume neutral descriptor inventory where available; COM-specific details remain COM wrapper plan. |
| `generate_xll_shim` and `xloper` helpers | `crates/oxvba-build/src/xll.rs`, `xloper.rs` | Generates XLL registration/runtime bridge from native export metadata. | XLL wrapper policy, not generic UDF substrate. | Keep future-XLL as wrapper profile; no Excel/XLL parity claim in host-callable core. |
| CLI wrapper/native-ready runner code | `crates/oxvba-cli/src/main.rs` | Builds/runs wrapper EXE/library and inspects exported call smoke evidence. | Wrapper orchestration. | Move toward plan-driven wrapper generation and generated CLI reflection-caller example. |

### E. Neutral host callback surfaces not targeted for deletion

| Surface | Location | Reason |
| --- | --- | --- |
| `HostCallbacks`, `DefaultHostCallbacks`, `UiVirtualizationMode::HostCallback` | `crates/oxvba-hal`, `crates/oxvba-host` tests | These are generic host integration/callback surfaces, not host-UDF policy. They may participate in `VbaHost` options but are not deprecated by this rework. |
| Host project/VBIDE callback tests | `crates/oxvba-host/tests/host_project_*` | These cover host project interactions separate from UDF naming. Keep unless later descriptor/API work finds concrete overlap. |

## Misplaced policy summary

The highest-risk boundary leakage is currently concentrated in three places:

1. `crates/oxvba-host/src/engine.rs`: public `HostUdf*` DTO/API surface combines
   neutral reflection facts, W093 registration-ish identity, worksheet-like
   contexts, typed call conversion, and invocation.
2. `crates/oxvba-compiler/src/bundle.rs`: `BundleHostCallDescriptor` synthesizes
   host/UDF policies (`public-procedural-functions`, `worksheet-cell`,
   `single-threaded-vba-compatible`, etc.) inside serialized compiler/bundle
   metadata.
3. `crates/oxvba-build/src/xll.rs` and `crates/oxvba-cli/src/main.rs`: XLL/Addin
   packaging is currently a direct build path over `NativeExportDescriptor`; it
   should become a wrapper profile over generic reflection/wrapper generation.

## Removal / replacement table

| Old shape | Action | New shape |
| --- | --- | --- |
| `HostUdfCatalog` | Delete | `CallableCatalog` / `ProjectReflection` |
| `HostUdfFunctionDescriptor` | Delete | `ProcedureDescriptor` + `ProcedureSignature` |
| `HostUdfRegistrationIdentity` | Delete from core host API | Host-owned W093 projection/example output |
| `HostUdfCallableMetadata` | Delete | Neutral procedure signature + explicit source annotations |
| `HostUdfInvocationTarget` | Delete | `CallableInvocationTarget` / prepared session route |
| `HostUdfCapabilityConstraints` | Delete | Neutral `CallableCapability`; host policies outside core |
| `HostUdfArgumentDescriptor` | Delete | `ProcedureParameterDescriptor` |
| `HostUdfTypeMapEvidence` | Delete | Generic conversion-lane evidence if still needed |
| `HostUdfTypedValue` | Delete | `TypedValue` |
| `HostUdfTypedSignature` | Delete | `CallableTypedSignature` |
| `HostUdfTypedInvokeResult` | Delete | `TypedInvocationResult` |
| `HostUdfCallContext` | Delete | `HostCallContext` |
| `HostUdfInvokeResult` | Delete | `InvocationResult` |
| `Engine::host_udf_catalog` | Delete | `VbaProject::reflect()` / `PreparedProject::callables()` |
| `Engine::invoke_host_udf_with_variants` | Delete | `invoke_callable_with_variants` |
| `Engine::invoke_host_udf_typed` | Delete | `invoke_callable_typed` |
| `RuntimeCallSource::HostUdf` | Delete | neutral host-call/provenance source |
| `BundleHostCallDescriptor` | Replace | neutral `BundleCallableDescriptor` or descriptor inventory entry |
| Synthesized bundle `selection_policy`, `volatile`, dependency/side-effect/thread policy, `allowed_contexts` | Delete from compiler-owned metadata | Host policy or wrapper plan |
| XLL as generic UDF substrate | Reframe | future `XllWrapperPlan` profile over wrapper-generation substrate |

## Follow-on bead mapping

- `bd-hjys.2`: freeze the neutral descriptor/API names and exact replacement
  contract before code deletion starts.
- `bd-hjys.3`: add neutral project reflection descriptors.
- `bd-hjys.4`: replace bundle host-call descriptor truth with neutral callable
  descriptor truth.
- `bd-hjys.6`: replace UDF-named invocation with neutral typed/variant callable
  invocation and actual context delivery.
- `bd-hjys.7`: delete the old `HostUdf*` public API surface and migrate tests.
- `bd-hjys.9` through `bd-hjys.12`: introduce wrapper plans and refactor EXE,
  native library, COM, and future-XLL surfaces over those plans.
- `bd-hjys.14`: refresh PH-0011 and historical evidence truth.

## Fresh-eyes review notes

Read-through findings after drafting:

- The audit distinguishes generic `HostCallbacks` from host-UDF code so the
  rework does not accidentally delete unrelated host integration callbacks.
- The bead description originally allowed compatibility adapters; this was
  corrected to match the workset's first-pass/no-compatibility rule.
- The removal table names deletion/replacement actions, not compatibility shims.
- XLL is classified as a wrapper profile/future special case, not as the generic
  UDF foundation.

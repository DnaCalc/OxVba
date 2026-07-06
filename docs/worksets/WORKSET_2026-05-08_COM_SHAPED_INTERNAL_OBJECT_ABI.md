# Workset — COM-Shaped Internal Object ABI And Dispatch Model

Date: 2026-05-08
Status: complete for the scoped internal descriptor/call-model floor; COM server packaging/export remains out of scope

## Objective

Move OxVba's internal object/interface representation to the same semantic shape required for true COM early binding and future COM publication of OxVba classes as dual interfaces, while preserving OxVba-owned optimized late-bound dispatch paths.

This is Workset 1 in the Access/JET strict early-binding recovery sequence.

## Rationale

Current runtime value truth is already strongly OLE Automation-shaped:

- runtime strings are `BStr` wrappers over BSTR-style UTF-16 allocations;
- runtime `Variant` is a 16-byte VARIANT-shaped cell with VARTYPE/reserved words/payload;
- object payloads already carry an `ObjectRef` backed by an IUnknown-like base pointer.

The previous mismatch was object/interface semantics. The scoped floor now provides descriptor-backed `ObjectRef` identities (`RuntimeClassDescriptor`, `RuntimeInterfaceDescriptor`, and `RuntimeMemberDescriptor`), raw descriptor-aware `QueryInterface` projection, typed project-member call metadata, dual-dispatch metadata for pure project objects, default/indexed/property descriptor coverage, and cached OxVba-owned dynamic dispatch. Native COM server packaging/export remains a separate out-of-scope lane.

## Design Direction

Use a COM/dual-interface-shaped internal model for OxVba objects:

- stable object identity;
- AddRef/Release-compatible lifetime;
- QueryInterface-like interface projection;
- class descriptors that expose implemented interfaces;
- interface descriptors with stable member IDs/slots;
- typed early-bound member calls when receiver type/interface is known;
- optimized internal dynamic dispatch when receiver type is `Variant`, `Object`, or otherwise not statically known;
- dual-interface publication shape so the same descriptors can later be exported as COM vtable + dispatch interfaces.

The goal is not to make every pure OxVba internal call pay Windows `IDispatch` costs. The goal is to make the object model semantically and structurally compatible with COM while allowing OxVba to use faster internal dispatch implementations.

## Late-Bound Dispatch Policy

OxVba should maintain its own late-bound dispatch plans rather than blindly using native `IDispatch` semantics internally.

Required properties:

- member-name lookup caching per concrete class/interface descriptor;
- stable normalized-name keys matching VBA case-insensitive lookup rules;
- cached default-member and indexed-property resolution;
- call-kind-sensitive lookup (`Method`, `PropertyGet`, `PropertyLet`, `PropertySet`);
- argument-shape cache entries for common arities/named-argument sets;
- efficient dynamic invoke over OxVba-native objects;
- boundary adapter to native COM `IDispatch` only when the receiver is a native COM object or a COM export/import boundary requires it.

This means internal late binding may be faster and more direct than COM `IDispatch`, while retaining equivalent observable VBA semantics for supported cases.

## Scope

### In scope

1. Replace/extend the current minimal `ObjectRef`/`RawRuntimeIUnknown` representation with a descriptor-backed object/interface runtime model.
2. Define internal interface identifiers compatible with future GUID projection.
3. Define class descriptors and interface descriptors for OxVba classes/modules.
4. Lower known OxVba class/interface calls to typed descriptor/member calls rather than ad hoc member handling.
5. Lower unknown `Variant`/`Object` receiver calls to internal dynamic dispatch with lookup caching.
6. Represent default members and indexed properties in the descriptor model.
7. Preserve `Property Get` / `Property Let` / `Property Set` distinction through lowering and runtime dispatch.
8. Keep BSTR/VARIANT/SAFEARRAY carriers compatible with existing runtime value truth.
9. Add red/green tests proving pure OxVba classes use the new typed and dynamic paths.

### Out of scope

- Binding arbitrary imported COM typelibs into this model. That is Workset 2.
- Full native COM export registration/packaging. This workset prepares the internal descriptor/runtime model but does not need to ship external COM servers.
- Replacing optimized OxVba internal dynamic dispatch with native `IDispatch` for pure OxVba objects.

## Required Test Shapes

Pure OxVba typed early-bound path:

```vb
Dim widget As New Widget
valueOut = widget.Value(5)
Set childOut = widget.Child
```

Pure OxVba interface path:

```vb
Dim iface As IWidget
Set iface = New Widget
valueOut = iface.Value(5)
```

Pure OxVba late-bound path:

```vb
Dim v As Variant
Set v = New Widget
valueOut = v.Value(5)
```

Default/indexed member path:

```vb
Dim fields As New FieldBag
Set field = fields("Name")
valueOut = field.Value
```

Property assignment-intent path:

```vb
Let widget.Value = 42
Set widget.Child = child
```

## Current Evidence

First implementation slice:

- `crates/oxvba-runtime/src/object_ref.rs` defines runtime class/interface/member descriptors and a compatibility class descriptor for existing `ObjectRef` values.
- `ObjectRef::class_descriptor()` and `ObjectRef::query_interface_descriptor(...)` expose descriptor-backed interface metadata for the current compatibility object floor, and the raw runtime `QueryInterface` implementation now projects descriptor-advertised interfaces instead of hard-coding IUnknown-only behavior.
- `ObjectRef::from_compat_identity_with_descriptor(...)` allows future class/object constructors to attach a descriptor-backed interface table while preserving the existing IUnknown-like lifetime floor.
- `RuntimeDispatchPlanCache` provides the first optimized internal late-bound plan cache keyed by normalized member name, interface id, call kind, and arity; runtime member descriptors now carry arity so cached plans do not conflate indexed/default call shapes.
- `Vm::set_project_dynamic_objects(...)` now constructs descriptor-backed `ObjectRef` identities for pure OxVba project dynamic/class objects, with dual-dispatch metadata derived from `ProjectDynamicObjectRoute`/`ProjectDynamicMemberRoute`, including parameter descriptors and return type descriptors.
- VM project-object name and default-member dispatch now consult a per-object descriptor-backed `RuntimeDispatchPlanCache` before falling back to the existing route scan, preserving the optimized late-bound dispatch direction without routing through native `IDispatch` and without caching ambiguous default metadata. Unhinted dynamic calls, including explicit `DispatchInvoke(...)` traffic without a property/method hint, now cache descriptor plans when the member/default lookup is unique for the supplied arity and continue to reject ambiguous shapes.
- `Instruction::CallProc` now carries optional `ProjectMemberCallDescriptor` metadata for compiler-lowered pure OxVba project member calls (`Method`, `PropertyGet`, `PropertyLet`, `PropertySet`). This preserves the existing direct-call execution path while making known receiver calls explicit typed project-member calls in bytecode rather than opaque ordinary procedure calls.
- Validation after descriptor-cache integration:
  - `cargo test -p oxvba-runtime --quiet` -> 79 passed.
  - `cargo test -p oxvba-vm --quiet` -> 7 passed.
  - `cargo test -p oxvba-jit --quiet` -> 8 passed.
  - `cargo test -p oxvba-compiler --quiet` -> 818 passed.
  - `cargo test -p oxvba-host --test project_entry_point_end_to_end -- --test-threads=1` -> 2 passed.
  - `cargo test -p oxvba-host --test com_early_project_end_to_end pure_oxvba -- --nocapture` -> 3 passed after pure OxVba descriptor-cache indexed/property coverage and interface receiver execution coverage.
  - `cargo test -p oxvba-host --test com_early_project_end_to_end -- --test-threads=1` -> 124 passed after project-member call descriptor metadata, pure OxVba descriptor-cache indexed/property get/let/set coverage, and interface receiver execution coverage.
  - Consolidated validation after property-set descriptor coverage: `cargo test -p oxvba-runtime --quiet` -> 79 passed; `cargo test -p oxvba-vm --quiet` -> 7 passed; `cargo test -p oxvba-compiler --quiet` -> 819 passed; `cargo test -p oxvba-jit --quiet` -> 8 passed; full COM host E2E -> 124 passed.
  - Serialization/broader host validation after `CallProc` metadata change: `cargo test -p oxvba-compiler bundle --quiet` -> 10 passed; `cargo test -p oxvba-host --quiet` -> all host test binaries passed (with existing ignored lanes unchanged).
- `compat_object_exposes_descriptor_backed_iunknown_interface` proves that existing compat objects expose the internal IUnknown descriptor without falsely claiming dual dispatch support.
- `descriptor_backed_object_can_advertise_dual_dispatch_shape` proves an object can advertise a dual-dispatch interface descriptor with default member metadata and vtable slot metadata.
- `descriptor_backed_object_supports_raw_query_interface_projection` proves raw runtime `QueryInterface` succeeds for descriptor-advertised `IDispatch` and preserves AddRef/Release balance.
- `iunknown_identity_is_intrinsic_even_when_descriptor_omits_it` hardens the runtime ABI so
  OxVba-owned object boxes always expose stable `IUnknown` descriptor/projection and raw
  `QueryInterface(IUnknown)` identity even if a malformed/incomplete class descriptor omits the
  copied `IUnknown` interface row; the existing foreign-IUnknown wrapper test remains green and
  proves native/foreign objects are not given OxVba runtime descriptors by this fallback.
- `verify_program` now treats the hidden `Me` receiver as part of the executable class/member ABI:
  class lifecycle hooks and class method descriptor targets must start with an explicit parameter
  local named `Me`. VM runtime member descriptor extraction and project-member argument name
  mapping skip only that explicit receiver, so a malformed hand-built class proc cannot silently
  lose its first source-visible parameter if it reaches descriptor construction directly.
- `verify_program` also rejects class metadata kind drift: lifecycle hooks must target `Sub`
  procedures, `PropertyGet`/`PropertyLet`/`PropertySet` descriptor rows must target matching
  property procedure kinds, and ordinary `Method` rows may target either `Sub` or `Function`.
  This keeps runtime/COM-facing invoke-kind metadata consistent with executable procedure
  descriptors rather than relying on source reconstruction.
- `verify_program` now preserves the package contract for class property setter descriptors:
  `PropertyLet` and `PropertySet` rows must expose an assigned-value parameter as the final
  source-visible parameter after hidden `Me`, and that value parameter must be stored as runtime
  ByVal. This mirrors the binder's lowering rule for VBA setter values without broadening the
  current claim to full `PropertySet` object-type compatibility.
- `verify_program` now also rejects contradictory class default/enumerator metadata before VM3 or
  COM-facing descriptor construction: a class may not mark differently named members as default,
  explicit default-member DISPIDs must be `0`, `_NewEnum` rows must use get/method semantics with
  no source-visible parameters, explicit `_NewEnum` DISPIDs must be `-4`, and a class may not
  publish multiple enumerator rows. Property Get/Let/Set rows for the same logical default
  property remain valid.
- `runtime_dispatch_plan_cache_normalizes_and_reuses_member_lookup` proves normalized case-insensitive member lookup caching, descriptor-backed default member lookup caching, and distinct call-kind/arity plans.
- `runtime_dispatch_plan_cache_caches_unhinted_unique_member_lookup` proves unhinted member/default lookup caches unique arity-matched descriptor plans, covering explicit dynamic call traffic that does not carry a property/method hint.
- `runtime_dispatch_plan_cache_rejects_unhinted_ambiguous_member_lookup` and `runtime_dispatch_plan_cache_rejects_ambiguous_default_member` prove ambiguous unhinted/default metadata is not cached as a single plan.
- `compile_project_internal_dynamic_routes_do_not_keep_transitional_token_table` proves known pure OxVba receiver calls carry `ProjectMemberCallDescriptor` bytecode metadata while route metadata remains free of transitional dispatch tokens.
- `project_dynamic_objects_advertise_dual_dispatch_descriptors` proves VM-registered pure project objects advertise descriptor-backed `IDispatch` member metadata, including default member, dispatch id, invoke kind, and vtable slot shape.
- `pure_oxvba_class_object_exposes_runtime_descriptor_metadata` proves a real compiled `Dim widget As New Widget` project dynamic object can be registered into the VM and queried for descriptor-backed default-member metadata while the project still executes correctly.
- `as_new_local_reinstantiates_after_set_nothing` and `as_new_field_reinstantiates_after_set_nothing`
  prove binder-emitted `As New` slot metadata is lazy rather than eager: local slots and class
  fields that are cleared with `Set ... = Nothing` instantiate a fresh initialized project object
  on the next read.
- `predeclared_instance_reset_to_nothing_recreates_active_singleton` and
  `cross_project_predeclared_reset_to_nothing_recreates_referenced_singleton` prove
  `VB_PredeclaredId` singleton storage follows the same reset rule in active and referenced
  projects: `Set ClassName = Nothing` clears the cached singleton and the next class-name access
  constructs a fresh instance in the owning bundle.
- `pure_oxvba_variant_receiver_uses_descriptor_cache_for_default_indexed_and_properties` proves compiled pure OxVba indexed/default property get, property let, property set, and property get routes expose unique unhinted descriptor-cache plans and still execute to the expected values, including natural `widget(5)` default-member syntax.
- `pure_oxvba_interface_receiver_executes_through_project_descriptor_shape` covers the required pure OxVba interface receiver path (`Dim iface As IWidget`, `Set iface = widget`, `iface.Value(5)`) and validates dispatch to the implementing `IWidget_Value` member.

## Current COM-compatible ABI boundary policy

- The internal ABI is COM-shaped at the identity/descriptor layer: objects expose class descriptors, interface descriptors, member dispatch IDs, vtable slot metadata where known, default-member flags, call kind, arity, parameter descriptors, and return descriptors.
- It is not a blanket commitment to implement all internal dispatch by calling native `IDispatch`; pure OxVba dynamic dispatch remains OxVba-owned and cache-backed for portability and optimization.
- Native COM publication/registration remains out of scope for this workset, but the descriptor shape is intentionally compatible with future dual-interface export work.

Validation:

```powershell
cargo test -p oxvba-runtime --quiet
cargo test -p oxvba-compiler --quiet
```

## Completion Criteria

Completion audit result: the scoped workset criteria are satisfied by the evidence above. This workset is complete only when:

- internal OxVba objects expose descriptor-backed interface tables;
- known receiver calls lower to typed internal interface/member calls;
- unknown receiver calls lower to cached dynamic dispatch plans;
- default and indexed member tests pass for pure OxVba objects;
- property get/let/set tests pass through the new descriptor path;
- no current BSTR/VARIANT/SAFEARRAY runtime layout truth regresses;
- docs explain where the internal ABI is intentionally COM-compatible and where it remains OxVba-owned for optimization/portability.

## Follow-on

Workset 2 projects imported COM typelibs into this same descriptor model and uses the strict Access/JET early-bound test as a primary red-to-green target.

# Workset — COM-Shaped Internal Object ABI And Dispatch Model

Date: 2026-05-08
Status: in-progress

## Objective

Move OxVba's internal object/interface representation to the same semantic shape required for true COM early binding and future COM publication of OxVba classes as dual interfaces, while preserving OxVba-owned optimized late-bound dispatch paths.

This is Workset 1 in the Access/JET strict early-binding recovery sequence.

## Rationale

Current runtime value truth is already strongly OLE Automation-shaped:

- runtime strings are `BStr` wrappers over BSTR-style UTF-16 allocations;
- runtime `Variant` is a 16-byte VARIANT-shaped cell with VARTYPE/reserved words/payload;
- object payloads already carry an `ObjectRef` backed by an IUnknown-like base pointer.

The remaining mismatch is object/interface semantics. `ObjectRef` now has the first descriptor-backed compatibility floor (`RuntimeClassDescriptor`, `RuntimeInterfaceDescriptor`, and `RuntimeMemberDescriptor`) and exposes the compat object's internal IUnknown descriptor, but it does not yet provide full typed vtable/member call, dual-interface dispatch, default/indexed property, and QueryInterface-style projection model needed for natural early-bound COM and COM publication.

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
- `ObjectRef::class_descriptor()` and `ObjectRef::query_interface_descriptor(...)` expose descriptor-backed interface metadata for the current compatibility object floor.
- `ObjectRef::from_compat_identity_with_descriptor(...)` allows future class/object constructors to attach a descriptor-backed interface table while preserving the existing IUnknown-like lifetime floor.
- `RuntimeDispatchPlanCache` provides the first optimized internal late-bound plan cache keyed by normalized member name, interface id, call kind, and arity; runtime member descriptors now carry arity so cached plans do not conflate indexed/default call shapes.
- `Vm::set_project_dynamic_objects(...)` now constructs descriptor-backed `ObjectRef` identities for pure OxVba project dynamic/class objects, with dual-dispatch metadata derived from `ProjectDynamicObjectRoute`/`ProjectDynamicMemberRoute`.
- VM project-object name dispatch now consults a per-object descriptor-backed `RuntimeDispatchPlanCache` before falling back to the existing route scan, preserving the optimized late-bound dispatch direction without routing through native `IDispatch`.
- `compat_object_exposes_descriptor_backed_iunknown_interface` proves that existing compat objects expose the internal IUnknown descriptor without falsely claiming dual dispatch support.
- `descriptor_backed_object_can_advertise_dual_dispatch_shape` proves an object can advertise a dual-dispatch interface descriptor with default member metadata and vtable slot metadata.
- `runtime_dispatch_plan_cache_normalizes_and_reuses_member_lookup` proves normalized case-insensitive member lookup caching and distinct call-kind/arity plans.
- `project_dynamic_objects_advertise_dual_dispatch_descriptors` proves VM-registered pure project objects advertise descriptor-backed `IDispatch` member metadata, including default member, dispatch id, invoke kind, and vtable slot shape.

Validation:

```powershell
cargo test -p oxvba-runtime --quiet
cargo test -p oxvba-compiler --quiet
```

## Completion Criteria

This workset is complete only when:

- internal OxVba objects expose descriptor-backed interface tables;
- known receiver calls lower to typed internal interface/member calls;
- unknown receiver calls lower to cached dynamic dispatch plans;
- default and indexed member tests pass for pure OxVba objects;
- property get/let/set tests pass through the new descriptor path;
- no current BSTR/VARIANT/SAFEARRAY runtime layout truth regresses;
- docs explain where the internal ABI is intentionally COM-compatible and where it remains OxVba-owned for optimization/portability.

## Follow-on

Workset 2 projects imported COM typelibs into this same descriptor model and uses the strict Access/JET early-bound test as a primary red-to-green target.

# VMR-06 Descriptor Driven Behavior Selection v1

Status: `working-draft`
Date: 2026-05-27
Scope owner: OxVBA compiler/VM/runtime/native-readiness

## Purpose

Select the first behavior-affecting VM change that may consume executable
semantic package metadata. This document is the entry record for VMR-06. It
defines the selected slice, the descriptor inputs it may use, the fixture gates,
and the rollback/gap policy before implementation starts.

The selection is intentionally narrow. VMR-06 is allowed to prove that one
well-evidenced VM behavior can be descriptor-driven; it is not a broad call
binding rewrite and does not change JIT status.

## Selected Behavior Slices

### Call-Entry Coercion

Selection id: `VMR06-CALL-BYVAL-COERCE-001`

Behavior:

Use package call/signature/coercion facts to initialize a direct callee
`ByVal` parameter with the declared target type at call entry.

First implemented shape:

- caller argument source: direct local variable slot;
- source declared type: `Long`;
- target parameter mechanism: `ByVal`;
- target declared type: `Double`;
- target callable: direct project procedure/function with known target entry;
- storage kind: `ArgumentBindingKindDescriptor::ByValCopy`;
- coercion family: call-entry Let coercion from `Long` to `Double`;
- expected callee entry observation: `VarType(value)=5` for the `Double`
  parameter, not the current `VarType(value)=2` observation.

The scoped fixture is
`conformance/vm_package/identity_seed/vmr04_call_argument_binding.bas`, function
`TakeDouble(ByVal value As Double, ByRef observedType As Long) As Double`.
Current VM evidence proves the gap: the callee observes `VarType(value)=2` even
though the signature descriptor knows `value As Double`. The runtime scalar
carrier and helper support already exist, so the selected slice is a VM
call-entry binding limitation rather than a primitive carrier limitation.

Implementation evidence:

- `bd-iave.9.2` wires package execution through
  `Vm::apply_descriptor_driven_call_entry_bindings`.
- The VM applies the behavior only while executing a loaded
  `VmExecutionPackage`; raw bytecode execution remains the pre-VMR06 baseline.
- The implemented match requires a known direct project function/procedure
  target, `ByValCopy`, variable argument expression, known source and parameter
  slots, caller source slot metadata of local `Long`/`I32`, target parameter
  signature metadata of `ByVal Double`, target parameter slot metadata of
  `Parameter Double`/`F64`, and a runtime `Long` value.
- `VMR04_CALL_ARGUMENT_BINDING` now records package execution with
  `main:byvaltype = 5` while the fixture harness also asserts the raw bytecode
  baseline remains `main:byvaltype = 2`.
- `bd-iave.9.4` makes the selected table-backed helper choice observable in
  call-site evidence: `COERCE-CALL-BYVAL-DECLARED-TARGET`,
  `COERCE-LET-NUMERIC-WIDEN`, and runtime helper
  `oxvba_runtime::coerce_to`.

### Static Array Bounds

Selection id: `VMR06-ARRAY-STATIC-BOUNDS-001`

Behavior:

Use package array-shape facts to answer `LBound` and `UBound` for rank-1
fixed/static local arrays whose runtime base slot remains unallocated because
the current VM lowers the elements into compiler-generated scalar element
slots.

First implemented shape:

- array storage kind: `ArrayStorageKind::StaticFixed`;
- rank: `1`;
- descriptor source: `ArrayShapeDescriptor.bounds[0]`;
- target operation: intrinsic `LBound` or `UBound`;
- operand mapping: the intrinsic's materialized temporary may be traced through
  the immediate pre-intrinsic `CopySlot` back to the descriptor base slot;
- package gate: only `Vm::execute_package` may consume the descriptor;
- raw bytecode gate: ordinary `Vm::execute` must keep the pre-VMR06 runtime
  error when the base slot is unallocated.

The scoped fixture is
`conformance/vm_package/identity_seed/vmr05_array_shape_bounds.bas`, procedure
`ArrayWorker`. The fixture now observes `LBound(fixed)`, `UBound(fixed)`,
`LBound(explicit)`, and `UBound(explicit)` through package execution while the
raw bytecode baseline fails with runtime error 13 on the fixed/static
unallocated base slot.

Implementation evidence:

- `bd-iave.9.3` wires package-only fixed/static rank-1 array bound lookup
  through `Vm::descriptor_declared_array_bound_for_intrinsic`.
- Runtime SAFEARRAY-backed dynamic arrays continue to use the existing runtime
  helper path.
- Descriptor absence or mismatch still produces the existing runtime error;
  the VM does not infer array bounds from compiler-generated element slot
  names or from ambient runtime state.

### UDT Owning Field Cleanup Evidence

Selection id: `VMR06-UDT-OWNING-FIELD-CLEANUP-001`

Behavior:

Use package UDT cleanup facts to build a VM lifecycle evidence lane for owning
UDT fields. This selected slice does not rewrite Rust carrier drop behavior or
introduce an explicit cleanup stack yet. It consumes `UdtTypeDescriptor.cleanup`
and owning `UdtFieldDescriptor` rows to make the VM-visible cleanup obligation
map explicit for success exit, branch exit, error exit, helper cleanup, and
future deopt materialization.

First implemented shape:

- descriptor source: `UdtTypeDescriptor` plus `UdtFieldDescriptor`;
- selected lifecycle row: `LIFE-UDT-FIELD-OWNING`;
- carrier rows surfaced for selected fields:
  `LIFE-BSTR-VARIABLE-STRING` and `LIFE-BSTR-FIXED-STRING`;
- selected field carrier: `RuntimeCarrierKind::BStr`;
- selected fixtures: `VMR02_UDT_FIELD_SLOTS` and
  `VMR05_UDT_DESCRIPTOR_MEMBERS`;
- evidence path: `VmPackageIdentityEvidence.lifecycle_evidence`;
- raw value behavior: unchanged.

Implementation evidence:

- `bd-iave.9.5` adds VM lifecycle evidence derived from descriptor-backed UDT
  cleanup ownership flags.
- `VMR02_UDT_FIELD_SLOTS` records variable string UDT field cleanup evidence for
  `Point.Caption`.
- `VMR05_UDT_DESCRIPTOR_MEMBERS` records fixed string UDT field cleanup evidence
  for `Record.Name`.
- The evidence records current runtime carrier observations for known field
  alias slots while keeping canonical cleanup descriptor ids and explicit VM
  cleanup-stack execution as later work.

## Call-Entry Descriptor Inputs

Required descriptor facts:

- `ProcedureSignatureDescriptor`
  - target procedure name and entry PC;
  - parameter order;
  - `value` parameter declared type `Double`;
  - resolved mechanism `ByVal`;
  - parameter slot.
- `CallSiteDescriptor`
  - target kind `Function`;
  - target entry known;
  - argument binding for `value`;
  - binding kind `ByValCopy`;
  - source expression kind `Variable`;
  - source slot known;
  - parameter slot known.
- `CoercionDescriptor` seed rows
  - `COERCE-CALL-BYVAL-DECLARED-TARGET`;
  - `COERCE-LET-NUMERIC-WIDEN`;
  - runtime helper id `oxvba_runtime::coerce_to`.
- `SlotTypeDescriptor`
  - caller source slot declared `Long`;
  - callee parameter slot declared `Double`;
  - return slot declared `Double`.

## Static Array Bound Descriptor Inputs

Required descriptor facts:

- `ArrayShapeDescriptor`
  - base slot known;
  - storage kind `StaticFixed`;
  - rank `1`;
  - one declared bounds row;
  - declared lower and upper bounds;
  - element type/carrier evidence retained for the fixture.
- Bytecode/intrinsic context
  - intrinsic operation is `LBound` or `UBound`;
  - intrinsic operand can be mapped to the array base slot through the current
    materialized-argument `CopySlot` lowering.
- Package execution state
  - descriptor metadata is active only for `VmExecutionPackage`;
  - raw VM execution with merely loaded metadata remains behaviorally raw.

Descriptor absence or mismatch is not an invitation to infer behavior from
bytecode shape. If any required descriptor fact is missing, the first VMR-06
implementations must leave the current VM path unchanged and keep the gap
classified. For the static array bound slice, the only bytecode-shape fact the
VM may use is the immediate argument-temp copy that maps the intrinsic operand
back to the descriptor base slot; the bound value itself must come from
`ArrayShapeDescriptor`.

## UDT Cleanup Descriptor Inputs

Required descriptor facts:

- `UdtTypeDescriptor`
  - descriptor id;
  - storage kind;
  - fieldwise copy classification;
  - cleanup ownership flags for BSTR, ObjectRef, SAFEARRAY, and Variant.
- `UdtFieldDescriptor`
  - field name and index;
  - carrier kind;
  - fixed string length where present;
  - alias slot names and slot ids.
- Runtime package evidence context
  - final runtime carrier observation for known alias slots;
  - lifecycle evidence digest for the selected cleanup scope.

Descriptor absence or mismatch leaves lifecycle evidence absent for the
selected cleanup scope. The VM must not invent cleanup obligations from field
names or runtime values alone.

## Acceptance Fixtures

Primary fixture:

- `VMR04_CALL_ARGUMENT_BINDING`
  - Raw bytecode baseline: `main:byvaltype:Local:Long` snapshot records `2`.
  - Package execution: `main:byvaltype` records `5`.
  - Call-site evidence records the selected coercion row ids and runtime
    helper id for the `value` argument.
  - `main:byvalcoerced` remains `f64:4.5`.
  - caller `seed` remains unchanged.
  - ByRef alias/writeback observations remain unchanged.
  - ByRef expression temporary/no-writeback behavior remains unchanged.
  - Optional default, Optional `Variant` missing metadata, empty/non-empty
    `ParamArray`, named arguments, and return copyout observations remain
    unchanged.

- `VMR05_ARRAY_SHAPE_BOUNDS`
  - Raw bytecode baseline: runtime error 13 on fixed/static `LBound`.
  - Package execution: `fixedL=1`, `fixedU=3`, `explicitL=0`, and
    `explicitU=2` are produced from package array-shape descriptors.
  - Dynamic `ReDim 2 To 4` `LBound`/`UBound` still use runtime SAFEARRAY
    bounds and remain `2`/`4`.
  - Total element snapshot remains `72`.
  - Loading package metadata into a raw VM and calling `execute` must not
    activate descriptor-driven array bounds.

- `VMR02_UDT_FIELD_SLOTS`
  - Lifecycle evidence records `LIFE-UDT-FIELD-OWNING` for `udt:point`.
  - The `Caption` field records carrier lifecycle id
    `LIFE-BSTR-VARIABLE-STRING`.
  - Success, branch, and error exit obligations are present as evidence tokens.
  - Known field alias slots record the current runtime carrier as
    `variant-string`.

- `VMR05_UDT_DESCRIPTOR_MEMBERS`
  - Lifecycle evidence records `LIFE-UDT-FIELD-OWNING` for `udt:record`.
  - The fixed-length `Name` field records carrier lifecycle id
    `LIFE-BSTR-FIXED-STRING`.
  - Success, branch, and error exit obligations are present as evidence tokens.
  - The scoped value snapshot remains unchanged.

Required check commands for the implementation bead:

```text
cargo test -p oxvba-vm --test package_identity_fixtures vm_package_identity_seed_fixtures_emit_identity_values_and_slot_descriptors -- --exact --nocapture
cargo test -p oxvba-vm semantics::tests:: -- --nocapture
./scripts/run-jit-v2-tracer-fixtures.ps1
./scripts/check-governance.ps1
git diff --check
./scripts/invoke-br-serialized.ps1 -- dep cycles --json
```

If the implementation touches compiler lowering, also run the relevant
`oxvba-compiler` call-binding tests.

## Out Of Scope

The first slice does not change:

- `ByRef` alias/writeback behavior;
- ByRef expression temporary behavior;
- Optional missing `Variant` behavior;
- `ParamArray` packing;
- property/default-member binding;
- object, class, interface, or COM member binding;
- native Declare or exported callable ABI projection;
- string, Date, Currency, Boolean, UDT, object, array, or Variant-wide
  call-entry coercion;
- dynamic array runtime SAFEARRAY bound behavior;
- multi-rank array bound behavior;
- bounds-error routing;
- fixed/static array allocation or element storage;
- explicit cleanup stack execution;
- cleanup lifetime counters;
- runtime slot storage representation;
- `oxvba-jit` behavior.

Those areas remain separate VMR-06 or JIT-readiness work and must cite their
own descriptor rows and fixtures before behavior changes.

## Rollback And Gap Policy

The implementation must be easy to back out:

- keep the behavior behind the narrow descriptor match listed above;
- do not rewrite general `CallProc` binding as part of this slice;
- do not infer target types from runtime `Variant` values when descriptors are
  missing;
- do not convert unsupported shapes silently;
- if fixture behavior changes outside the selected observation, revert the VM
  behavior change and leave a classified gap in the completion map;
- if the coercion helper produces a compatibility mismatch, classify it as an
  oracle/runtime/coercion gap before expanding scope.

## Downstream Beads

This selection feeds:

- `bd-iave.9.2`: implement the selected descriptor-driven call binding path;
- `bd-iave.9.3`: implement the selected descriptor-driven fixed/static array
  bound path;
- `bd-iave.9.4`: bind the selected helper choice to table/coercion row ids in
  VM evidence;
- `bd-iave.9.5`: add selected UDT owning-field lifecycle evidence from
  descriptor cleanup facts;
- JIT readiness gates: use the changed VM evidence only after the VM fixture
  proves the new descriptor-driven behavior.

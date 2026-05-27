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

## Selected Slice

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

## Descriptor Inputs

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
  - `COERCE-LET-NUMERIC-WIDEN`.
- `SlotTypeDescriptor`
  - caller source slot declared `Long`;
  - callee parameter slot declared `Double`;
  - return slot declared `Double`.

Descriptor absence or mismatch is not an invitation to infer behavior from
bytecode shape. If any required descriptor fact is missing, the first VMR-06
implementation must leave the current VM path unchanged and keep the gap
classified.

## Acceptance Fixtures

Primary fixture:

- `VMR04_CALL_ARGUMENT_BINDING`
  - Before behavior change: `main:byvaltype:Local:Long` snapshot records `2`.
  - After behavior change: `main:byvaltype` must record `5`.
  - `main:byvalcoerced` remains `f64:4.5`.
  - caller `seed` remains unchanged.
  - ByRef alias/writeback observations remain unchanged.
  - ByRef expression temporary/no-writeback behavior remains unchanged.
  - Optional default, Optional `Variant` missing metadata, empty/non-empty
    `ParamArray`, named arguments, and return copyout observations remain
    unchanged.

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
- `bd-iave.9.4`: bind the selected helper choice to table/coercion row ids
  where the implementation needs a canonical coercion lookup;
- JIT readiness gates: use the changed VM evidence only after the VM fixture
  proves the new descriptor-driven behavior.

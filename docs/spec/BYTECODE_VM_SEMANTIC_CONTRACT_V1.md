# Bytecode And VM Semantic Contract v1

Status: `working-draft`
Date: 2026-05-26
Scope owner: OxVBA compiler/VM/native-readiness
Primary package reference:
[`EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](EXECUTABLE_SEMANTIC_PACKAGE_V1.md)

## Purpose

Define what bytecode means when it is interpreted as part of the executable
semantic package, and what metadata the VM must consume or expose as evidence.

This document folds together the bytecode semantic catalog, VM executable
package consumption contract, and VM evidence schema. The goal is to prevent a
future JIT from learning bytecode meaning informally while the VM remains the
reference execution engine.

## Bytecode Catalog Row Shape

Every opcode family should eventually have rows with this shape:

```text
opcode_or_family
operand_shape
slot_reads
slot_writes
declared_type_requirements
expression_call_descriptors
helper_descriptor
error_edges
cleanup_edges
source_map_requirement
VM_snapshot_obligation
unsupported_or_deopt_policy
current_gap
test_anchor
```

Rows can start at family granularity and split when semantics diverge.

## Initial Bytecode Family Catalog

| Family | Semantic facts required | VM consumption requirement | Current gap |
|---|---|---|---|
| Constants and copies | declared target slot type, value state, carrier init | initialize/copy with declared carrier semantics | Slot descriptor metadata incomplete. |
| Primitive arithmetic | operand/result declared types, operator row, overflow/coercion policy | execute helper or typed path matching operator descriptor | Operator table not package metadata yet. |
| Variant/dynamic arithmetic | Variant payload states, Let coercion, Null/Error behavior | retain VM helper behavior and snapshot payload | Helper behavior needs table-backed evidence. |
| Boolean/control tests | truthiness/coercion row and error policy | branch with VM-equivalent value-state handling | Truthiness table needs extraction. |
| String/BSTR | variable/fixed string descriptor, allocation, concat/Len helpers | preserve BStr ownership and failure cleanup | Fixed string and cleanup maps incomplete. |
| Arrays/SAFEARRAY | shape, bounds, element type, resize/preserve, enumeration | consume runtime shape and emit bounds/error evidence | Bounds metadata/evidence incomplete. |
| UDT fields/copy | nominal UDT id, field order, field carriers, copy/drop rules | execute descriptor-backed field operations | Full UDT descriptor missing. |
| Procedure calls | target signature, call-site descriptor, ByRef/ByVal, optional/defaults | bind args using descriptor, expose alias/writeback evidence | Call-site descriptors missing. |
| Properties/default members | accessor group, value param, default member binding | distinguish Let/Set/Get and object default value | Descriptor and VM evidence missing. |
| Error flow | `On Error`, Err state, resume target maps | use package error maps and snapshot Err state | Runtime exists; package maps incomplete. |
| Host services | host capability, policy, deterministic unsupported diagnostics | route through host policy and evidence | Capability digest missing. |
| COM | late/early descriptor, named/default args, HRESULT/EXCEPINFO | call COM bridge and capture boundary observations | Descriptor unification incomplete. |
| Native Declare | ABI descriptor, marshal descriptors, ByRef writeback | route through host/native lane and capture writeback | Wider ABI gaps remain. |
| Exported callable | inbound ABI, return/error policy, cleanup | not first-class VM path yet | Planned. |

## VM Package Consumption Contract

The VM should consume package metadata where it changes execution or evidence:

- procedure signature descriptors for argument binding and return slots;
- slot descriptors for initialization, carrier choice, snapshots, and
  declared-type evidence;
- expression and call descriptors for ByRef aliasing, temporary locals,
  optional/default handling, ParamArray construction, and default-member paths;
- array/UDT/object descriptors for shape, field, identity, copy, and cleanup
  behavior;
- error maps for `On Error`, `Resume`, `Resume Next`, and diagnostics;
- COM/native/export descriptors for boundary projection and writeback;
- host capability descriptors for deterministic unsupported behavior.

If the VM currently executes a behavior from hardcoded interpreter logic, that
is acceptable as current truth, but the fact must be classified as
`implemented-runtime-only` in the completion map until package metadata carries
it.

## First VM Rework Batch

The first batch should be additive and reversible at the behavior level:

1. Define borrowed or in-memory package descriptor views that can exist beside
   current `Bytecode`, `ProcedureRuntimeMetadata`, and `OxBundle` paths.
2. Attach package, procedure, bytecode, slot descriptor, and signature
   descriptor digests to VM evidence.
3. Load procedure signatures and slot descriptors during VM setup without using
   them to change slot storage or execution decisions.
4. Add fixture evidence that shows current VM behavior beside the descriptors
   it should eventually consume.
5. Use the evidence to classify each gap in the completion map before making
   behavior-affecting changes.

## Code Touchpoints For First Batch

Expected code surfaces for the metadata/evidence batch:

- `crates/oxvba-compiler/src/bytecode.rs`: opcode/family catalog anchors and
  bytecode digest inputs.
- `crates/oxvba-compiler/src/emit.rs`: procedure, slot, and temporary metadata
  preservation during emission.
- `crates/oxvba-compiler/src/project.rs`: package/project descriptor assembly
  and `OxBundle` integration.
- `crates/oxvba-vm/src/interpreter.rs`: descriptor loading, VM setup, and
  evidence capture.
- `crates/oxvba-vm/src/lib.rs`: public package execution helpers and evidence
  return shape.
- `crates/oxvba-host/src/engine.rs`: host execution paths that must pass the
  same package views into the VM.
- `conformance/jit_v2` or a new package-strengthening fixture area: VM-runnable
  descriptor/evidence tests.

## Do Not Change In First Batch

- VM runtime slot storage model.
- `Variant`, `BStr`, `SafeArray`, and `ObjectRef` runtime representations.
- COM/native execution behavior or boundary ownership.
- Error routing behavior.
- `oxvba-jit` disabled-placeholder behavior.

## VM Evidence Schema

VM evidence for package strengthening should capture:

```text
run_id
fixture_id
package_digest
bytecode_digest
procedure_id
opcode_family_coverage
slot_snapshot
slot_descriptor_digest
declared_carrier_layout
expression_call_descriptor_digest
call_binding_observations
byref_alias_writeback_observations
err_state
cleanup_lifetime_observations
array_shape_observations
udt_field_observations
object_identity_observations
interop_observations
host_policy_observations
unsupported_diagnostics
```

The snapshot can continue to materialize observable values as retained
`Variant` evidence, but descriptor digests must prove that primitive, UDT,
array, object, and call-site metadata were preserved in the package.

## Strengthening Rule

When a VM-runnable fixture exposes a semantic gap, classify it before changing
JIT planning:

- `test-shortcoming`: add evidence only.
- `metadata-missing`: add package metadata and make VM/evidence consume it.
- `VM-limitation`: implement or explicitly defer VM behavior.
- `runtime-limitation`: fix the runtime carrier/helper first.
- `interop-limitation`: fix COM/native/export projection before claiming that
  boundary.

The JIT cannot close a semantic area that the VM cannot execute or describe
under this contract.

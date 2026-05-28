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
| Primitive arithmetic | operand/result declared types, operator row, overflow/coercion policy | execute helper or typed path matching operator descriptor | Selected v14 operator descriptors and evidence exist; full truth table and descriptor-driven execution remain incomplete. |
| Variant/dynamic arithmetic | Variant payload states, Let coercion, Null/Error behavior | retain VM helper behavior and snapshot payload | Selected v14 coercion/operator/value-state descriptors exist; full helper truth table evidence remains incomplete. |
| Boolean/control tests | truthiness/coercion row and error policy | branch with VM-equivalent value-state handling | Selected v14 truthiness/logical/branch descriptors exist; broader extraction and behavior consumption remain incomplete. |
| String/BSTR | variable/fixed string descriptor, allocation, concat/Len helpers | preserve BStr ownership and failure cleanup | Fixed string and cleanup maps incomplete. |
| Arrays/SAFEARRAY | shape, bounds, element type, resize/preserve, enumeration | consume runtime shape and emit bounds/error evidence | Bounds metadata/evidence incomplete. |
| UDT fields/copy | nominal UDT id, field order, field carriers, copy/drop rules | execute descriptor-backed field operations | Seed UDT descriptors and selected lifecycle evidence exist; descriptor-backed offsets/layout/copy/drop execution remains incomplete. |
| Procedure calls | target signature, call-site descriptor, ByRef/ByVal, optional/defaults | bind args using descriptor, expose alias/writeback evidence | Seed descriptors and evidence exist; descriptor-driven binding and Optional-missing runtime behavior incomplete. |
| Properties/default members | accessor group, value param, default member binding | distinguish Let/Set/Get and object default value | Selected v14 property/default-member binding descriptors and evidence exist; broader object/default-member execution remains incomplete. |
| Error flow | `On Error`, enabled/active handler state, Err state, resume target maps, fallible operation edges | use package error maps, snapshot Err state, and expose handler/resume evidence | Selected error-routing/deopt evidence exists for the TB03 Resume Next division seed; broader maps and oracle-backed edge evidence remain incomplete. |
| Host services | host capability, policy, deterministic unsupported diagnostics | route through host policy and evidence | Host capability descriptor evidence exists; behavior-driving policy evaluation descriptors remain incomplete. |
| COM | late/early descriptor, named/default args, HRESULT/EXCEPINFO | call COM bridge and capture boundary observations | Seed interop descriptor evidence exists; descriptor unification and full boundary result evidence remain incomplete. |
| Native Declare | ABI descriptor, marshal descriptors, ByRef writeback | route through host/native lane and capture writeback | Seed interop descriptor evidence exists; wider ABI gaps remain. |
| Exported callable | inbound ABI, return/error policy, cleanup | expose package export descriptors and route hosted invocation through VM package facts | Export descriptor evidence exists for Variant-positional callable packages; external ABI execution breadth remains incomplete. |

## VM Package Consumption Contract

The VM should consume package metadata where it changes execution or evidence:

- procedure signature descriptors for argument binding and return slots;
- slot descriptors for initialization, carrier choice, snapshots, and
  declared-type evidence;
- expression, operator, coercion, name-binding, object-member, and call
  descriptors for ByRef aliasing, temporary locals, helper choice,
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

### Error Flow Consumption

The VM error lane must treat VBA errors as control flow plus mutable frame and
`Err` state. VM evidence for package-backed error behavior must record, where
the fixture reaches those states:

- the current procedure error mode and target label/PC;
- whether the current frame has an enabled handler and whether that handler is
  active;
- the faulting operation, fault PC/source mapping, resume target, and selected
  `Resume` form;
- `Err` fields before and after the fault, after `Err.Clear`, after `On Error`
  statements, and after procedure exit or handler resume points;
- caller-frame unwinding when a handler is already active;
- call-out behavior under `On Error Resume Next`;
- COM/native/host projection fields that feed `Err`, including
  `HRESULT`/`EXCEPINFO`/`ArgErr` and native return/`LastDLLError` policy where
  supported;
- cleanup/deopt state that must be preserved if execution transfers, resumes,
  or falls back.

Uncertain VBA behavior is an evidence gap, not a backend freedom. The
completion map must classify those rows explicitly before JIT lowering consumes
the corresponding error descriptor.

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
- `conformance/jit_v2` and `conformance/vm_package/identity_seed`:
  VM-runnable descriptor/evidence tests.

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
project_context_evidence
compile_context_evidence
procedure_id
opcode_family_coverage
slot_snapshot
slot_descriptor_digest
declared_carrier_layout
expression_semantics_evidence
operator_semantics_evidence
coercion_evidence
name_binding_evidence
object_member_binding_evidence
expression_call_descriptor_digest
call_binding_observations
byref_alias_writeback_observations
err_state
cleanup_lifetime_observations
array_shape_observations
udt_field_observations
object_identity_observations
interop_descriptor_evidence
interop_observations
error_descriptor_evidence
deopt_snapshot_evidence
host_policy_evidence
vm_consumption_evidence
source_map_evidence
package_diagnostics
gap_classifications
host_policy_observations
unsupported_diagnostics
```

The snapshot can continue to materialize observable values as retained
`Variant` evidence, but descriptor digests must prove that primitive, UDT,
array, object, and call-site metadata were preserved in the package.

Current VMR-01 seed evidence in `oxvba-vm` is
`VmPackageIdentityEvidence`. It records:

- package origin (`in-memory` package view or `OxBundle`);
- package digest and bytecode digest;
- bundle-backed project context evidence when an `OxBundle` v11 supplies
  project/module/reference/import, module option, compile context, source-map,
  native library, host capability, package diagnostic, and gap classification
  facts;
- package slot count and user slot count;
- per-procedure id, package-owned procedure descriptor id/digest, module name,
  procedure name, entry PC, slot descriptor digest, and slot descriptor rows;
- package-owned descriptor identity rows for bytecode, procedures, procedure
  signatures, slots, call sites, array shapes, UDTs, object routes, interop
  descriptors, lifecycle descriptors, error-routing descriptors, deopt
  snapshot descriptors, host-policy descriptors, expression semantics,
  operator semantics, coercion, name binding, and object/member binding
  descriptors reached by current evidence.

Canonical descriptor identity helpers live in
`crates/oxvba-compiler/src/descriptor_identity.rs`, not in the VM. The current
seed also exposes a stable `VbaTypeId` registry and default-carrier rows for the
implemented declared type enum. VM evidence consumes those package helpers and
therefore reports descriptor ids/digests without allocating semantic identity
inside the interpreter.

`oxvba-host` now threads the same evidence through package-backed source,
project, bundle, and callable-session VM execution paths. Existing snapshot APIs
remain value-only compatibility surfaces, while package-identity variants expose
the recorded VM package identity for evidence and future JIT gates.

This began as package identity plus first slot descriptor evidence. The current
surface now also includes signature comparison, call-site, array-shape, UDT,
object, interop, bundle project/compile context, selected lifecycle,
error-routing, deopt-snapshot, and host-policy evidence. Host-policy behavior
consumption and explicit cleanup-stack execution remain future rows under the
strengthening sequence above.

Current VMR-02 seed surface is a metadata view, not execution behavior:
`ProcedureRuntimeMetadata::slot_type_descriptors` and
`VmExecutionPackage::slot_type_descriptors` expose `SlotTypeDescriptor` rows for
the procedure slots known today. The first view preserves slot index, optional
name, role, known parameter/return declared type, initial-state classification,
and carrier hint. Facts the current compiler metadata cannot yet preserve are
reported as `Unknown`; they are not treated as `Variant` by default and are not
JIT-ready until later VMR-02 evidence fills them.

The populated VMR-02 compiler/package pass now records descriptor facts on
`ProcedureRuntimeSlotMetadata` for parameters, locals, return slots,
compiler-generated fixed-array element slots, and expression temporaries.
`OxBundle` format v12 also carries package-owned carrier layout descriptors
and value-state descriptors derived from slot metadata, procedure signatures,
bound intrinsics, and emitted value-state opcodes. The compatibility reader
preserves v10/v11 bundles and upgrades older v3/v4/v5/v6/v7/v8/v9 metadata
into the current descriptor shape with the new v12 descriptor vectors empty.
This remains metadata/evidence-only: VM slot storage, helper choice, and
runtime behavior do not consume these descriptors yet.
Host/project value snapshots continue to exclude `Temporary` descriptor rows so
metadata enrichment does not make compiler scratch slots user-visible.
The VM package evidence now reports descriptor roles, declared type ids, initial
states, and carrier hints per procedure while retained value snapshots remain
unchanged.
It also reports carrier-layout evidence for primitive, String/BSTR, Variant,
object, UDT, and Decimal96 Variant-subtype carriers, and value-state evidence
for Empty, Null, Error/CVErr, Nothing, missing optional arguments, omitted
defaults, vbNullString, and Decimal as a Variant subtype extension.
The package identity seed fixtures now assert descriptor tokens and value
snapshots for primitive scalar, `String`/`BStr`, declared `Variant`, and the
current VM-runnable UDT field-alias shape. UDT aggregate base slots now carry
`RuntimeCarrierKind::UdtFields` hints where the resolver knows the nominal
type; descriptor-backed UDT execution is still deferred.

Current VMR-03 seed surface is metadata/evidence, not call-binding behavior:
`ProcedureRuntimeMetadata::procedure_signature_descriptor` and
`VmExecutionPackage::procedure_signature_descriptors` expose
`ProcedureSignatureDescriptor` rows for procedure kind, parameter order, slots,
declared parameter types, parsed ByRef/ByVal mode, source parameter mechanism
where known, resolved mechanism, Optional/default/missing policy, ParamArray
shape, return type, return slot, property group, property value ByVal
semantics, and class hidden-receiver/`Me` metadata where current compiler
metadata knows them. `OxBundle` format v10 carries those facts, upgrades v6/v7
bundles with empty call-site rows, upgrades v5 bundles with `Unknown` source
mechanism where v5 lacked that distinction, and upgrades v4 bundles with
`Unknown` parameter passing mode where v4 had no serialized ByRef/ByVal fact.
VM call execution does not consume these descriptors yet.

The VMR-03 package evidence now also compares current `CallProc` lowering with
signature metadata for VM-runnable seed calls. `VmPackageIdentityEvidence`
includes signature/call observation rows that classify existing ByVal
no-copyback, ByRef copyback, Optional default materialization, ParamArray
packing, property value ByVal no-copyback, and function return-slot copyout.
These rows are evidence over current bytecode shape and value snapshots; they
are not a substitute for the VMR-04 `CallSiteDescriptor` and
`ArgumentBindingDescriptor` rows, and they must not be treated as proof that the
VM consumes signature descriptors for call binding.

Current VMR-04 seed surface is package metadata, not call-binding behavior:
`ProcedureRuntimeMetadata::call_sites` carries first `CallSiteDescriptor` rows
with `ArgumentBindingDescriptor` children for top-level project procedure call
sites. The seed rows represent target kind, call PC, target entry PC after
patching, named/positional/omitted/ParamArray argument source shape, ByRef
alias/writeback, ByRef expression temporary/no-writeback, ByVal copy, Optional
default, ParamArray pack, fixed-array materialization, default-member fallback
policy, invocation syntax (`Call` keyword, no-`Call`, expression-call, and
synthetic property-assignment forms), source argument evaluation order,
diagnostic-policy ownership for the current compiler-owned 448/449/450
invalid-call cases, and return copyout. `OxBundle` format v13 carries those
call policy fields while the v12 compatibility reader upgrades older call-site
rows with unknown syntax and empty diagnostic policies. The VM exposes these
rows through
`VmExecutionPackage::call_site_descriptors`, and
`VmPackageIdentityEvidence::call_site_evidence` records descriptor digests plus
observation tokens for the VM-runnable call fixtures. Package execution now has
one deliberately narrow descriptor-driven call-entry path for
`VMR06-CALL-BYVAL-COERCE-001`; all other call binding still follows the
existing bytecode lowering.

The VMR-04 fixture evidence also classifies current limitations and the first
behavior-driving exception: raw bytecode execution still shows the old ByVal
declared-type call-entry gap for the observed `Long` to declared-`Double`
shape, while package execution consumes the selected call/signature slot
descriptors and the callee observes a `Double` value at entry. Optional
`Variant` without an explicit default is described by package metadata as a
missing-argument policy
(`VariantMissingError448`) while current VM lowering still materializes a
default local value. The fixture records those behaviors as VM/runtime
limitations to resolve before descriptor-driven call binding or JIT lowering
can claim full call-coercion or Optional-missing parity. Call-site evidence also
records the selected coercion row id, numeric-widen row id, and runtime helper
id for the descriptor-driven `Long` to `Double ByVal` path, so helper choice is
observable without generalizing coercion behavior.

Current expression/operator/coercion seed surface is package metadata and VM
identity evidence, not broad expression rewiring: `OxBundle` format v14 carries
`ExpressionSemanticsDescriptor`, `OperatorSemanticsDescriptor`,
`CoercionDescriptor`, `NameBindingDescriptor`, and
`ObjectMemberBindingDescriptor` rows. The v13 compatibility reader upgrades
older bundles with those vectors empty, preserving v13 call-site policy facts
without inventing expression semantics. VM identity evidence reports descriptor
digests and observation tokens for selected expression classifications,
operator helper choices, `Option Compare` influence, truthiness/logical/branch
rows, explicit deferred `IIf` ordering, Let/Set/property-value coercion rows,
name-binding policy, and property/default-member member binding rows. These
rows make helper choice and selected property/coercion facts package-visible;
they do not imply that all operators, default-member routes, `With`/imported
member lookup, COM/native projection, or error-routing coercions are
descriptor-driven yet.

The authoritative VMR-04 call-gap ledger is the
`VMR-04 Call Fixture Gap Classification` section in
[`EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md`](EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md).
Behavior-affecting call work must cite that ledger, add any missing fixture row
it needs, and keep ByRef expression temporaries, ByVal call-entry coercion,
Optional missing state, ParamArray packing, and COM/native/export projections
separate. In particular, the ByVal `Long` to declared-`Double` gap is a
compiler/VM call-binding limitation with existing primitive carriers; the first
package-backed VMR-06 path proves only the direct local `Long` to
declared-`Double ByVal` shape. The omitted Optional `Variant` gap also requires
a first-class missing-argument value state before `IsMissing` or call-entry
introspection can claim parity.

Current VMR-05 seed surface is package metadata plus VM evidence, not array
execution rewiring: `ProcedureRuntimeMetadata::array_shapes` carries
`ArrayShapeDescriptor` rows for arrays known to the resolver, and
`VmPackageIdentityEvidence::array_shape_evidence` reports descriptor digests
plus observations for rank, storage kind, declared bounds, `Option Base`,
element type/carrier, base-slot presence, bounds provenance, allocation/erase/
preserve policy, element lifecycle classification, and runtime SAFEARRAY bounds
when a base slot is allocated after VM execution. `VmPackageIdentityEvidence`
also records selected lifecycle evidence for fixed/static array elements,
dynamic SAFEARRAY ownership, and local SAFEARRAY slots. `OxBundle` format v10 carries these
rows, upgrades v7 bundles with an explicit empty array-shape set, upgrades v8
bundles with an explicit empty UDT descriptor set, and upgrades v9 bundles with
an explicit empty object descriptor set.

The VMR-05 seed fixture proves the current positive subset for fixed/static
local arrays, explicit `0 To 2` bounds, dynamic `ReDim 2 To 4` SAFEARRAY bounds,
and ByRef scalar observation copyback. Package execution now consumes
`ArrayShapeDescriptor` for the selected VMR-06 rank-1 fixed/static
`LBound`/`UBound` path while raw bytecode execution still records the old
runtime error 13 on the unallocated fixed-array base slot. Compiler/VM unit
evidence now covers multi-rank descriptor serialization, but VM-runnable
multi-rank execution fixtures, runtime bounds-error evidence, and COM/native
SAFEARRAY projection remain incomplete before JIT lowering may claim TB05
closure.

The VMR-05 UDT descriptor fixture adds nominal `UdtTypeDescriptor` evidence for
the current flattened UDT storage model. VM evidence now records descriptor ids,
instances, field order, field carriers, nested UDT references, fixed-length
string field lengths, fixed array field bounds, field-alias slots, fieldwise
copy classification, recursive init policy, descriptor field-order layout
tokens, and cleanup ownership flags. The selected VMR-06 cleanup slice also
records `VmPackageIdentityEvidence::lifecycle_evidence` for UDT BSTR-owning
fields, including success, branch, error, helper, deopt, and runtime
alias-carrier observations. VM UDT field access, whole-copy execution,
byte-offset layout, and explicit cleanup-stack execution still do not consume
these descriptors yet.

The VMR-05 object descriptor fixture adds `ObjectTypeDescriptor` evidence for
the current object identity seed. Procedure metadata records generic
`Object` locals as `Nothing`-initialized `ObjectRef` carriers, and VM evidence
reports descriptor ids, object kind, activation/event/default-member policy,
support classification, object identity policy, slot instance observations, and
selected `LIFE-OBJECTREF-SLOT` lifecycle evidence. Bundle-carried route tables
now also surface as package identity evidence for VM-capable source-project
class routes, `As New` generated-handle activation, default-member route
presence, implemented interface descriptor ids, and imported COM `WithEvents`
route identities with handler guards and subscription/cleanup policy. Runtime
project evidence records the same route facts when the VM is supplied the
compiled project route tables. This is still metadata/evidence only:
descriptor-driven object/member call binding, default-instance execution,
full imported COM class/interface descriptors, and explicit object cleanup-stack
execution remain incomplete.

The VMR-05 interop descriptor fixture adds `VmInteropDescriptorEvidence` for
the hosted TB06 through TB09 COM/native/export tracer seeds. VM evidence
records COM `CreateObject` activation instructions, COM dispatch invoke
instructions, the current early-bound flag, selector source/policy,
arity/named-argument counts, runtime-owned HRESULT/EXCEPINFO/ArgErr policy,
runtime result projection classification, and dispatch cleanup classification.
It also records current native `ExternalCallDescriptor` and invoke instruction
facts, including library, alias/name, return and parameter type tokens,
`DeclareParamType` projection tokens, ByRef flags, argument slots, writeback
slots, writeback projection classifications, cleanup policy, and native error
policy. Bundle-backed project packages now also record exported-callable
descriptors derived from `ExportInventory` plus callable signatures, including
Variant-positional inbound projection, ByRef export-boundary writeback,
return-slot-to-Variant projection, cleanup/error policy, and unsupported-shape
policy. This is evidence only: COM HRESULT/EXCEPINFO/ArgErr projection remains
runtime-owned rather than package-unified, external export ABI execution is not
closed, and general Automation `Variant`/`SAFEARRAY` declared-parameter ABI
support remains incomplete.

The VMR-05/bd-tvmb.9 error/cleanup/deopt fixture adds VM package evidence for
selected `ErrorRoutingDescriptor`, `DeoptSnapshotDescriptor`, and
`HostPolicyDescriptor` rows. TB03 now records `On Error Resume Next`,
division-by-zero helper failure, Err-state fields, runtime error 11, and
resume-next-consumable policy as package evidence. Deopt evidence records
helper/call/host/COM/native safepoints with slot-map, live-carrier,
cleanup-state, error-state, ByRef-state, source-PC, and host-policy obligations.
Host-policy evidence records bundle capability requirements plus deterministic
`HAL-E-CAP-UNAVAILABLE` and `HAL-E-POLICY-DENIED` diagnostic tokens. This is
still evidence only: active-handler reentry, caller unwinding, full Resume
quirks, explicit cleanup-stack execution, and behavior-driving policy
evaluation descriptors remain incomplete.

The bd-tvmb.10 VM consumption ledger records which descriptor facts the VM
currently consumes and which package-visible facts remain blocked. The selected
supported rows are VMR-06 call-entry coercion and rank-1 static/fixed array
bounds; the selected evidence-only row is UDT owning field cleanup evidence.
Optional `Variant` missing, error/deopt cleanup, boundary projection, and
host-policy behavior consumption remain classified gap rows rather than
backend-local assumptions.

The strict package-only execution workset adds `VmPackageSupportReport` as the
shared support-query surface. The initial VM gate rejects incomplete in-memory
packages that lack executable semantic package sections. Current deferred
VM-consumption rows remain VM-execution warnings until their owning delivery
beads either consume descriptors or turn the rows into deterministic VM
rejects. The same deferred rows are `ProcLoweringIr` blockers immediately.
Public VM/JIT/launcher/debug execution entry points no longer accept bare
`Bytecode`; production and test harness execution must enter through
`OxBundle` or `VmExecutionPackage`. The VM instruction loop still receives the
package bytecode internally after package metadata is loaded.

The bd-tvmb.11 implementation-entry audit is
[`../validation/TYPED_VM_METADATA_BUNDLE_IMPLEMENTATION_ENTRY_AUDIT_2026-05-28.md`](../validation/TYPED_VM_METADATA_BUNDLE_IMPLEMENTATION_ENTRY_AUDIT_2026-05-28.md).
It records that remaining VM contract gap labels are scoped to broader deferred
behavior or future JIT-entry blockers, not unstated permission for
`ProcLoweringIr` to infer semantics from raw bytecode.

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

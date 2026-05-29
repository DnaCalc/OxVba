# Executable Semantic Package v1

Status: `working-draft`
Date: 2026-05-26
Scope owner: OxVBA compiler/VM/JIT/native-readiness
Type-system reference:
[`VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md)
Expression/call semantics reference:
[`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md)
Completion and VM strengthening references:
[`EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md`](EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md),
[`BYTECODE_VM_SEMANTIC_CONTRACT_V1.md`](BYTECODE_VM_SEMANTIC_CONTRACT_V1.md),
[`VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md`](VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md)
Completion workset:
[`../worksets/WORKSET_2026-05-27_TYPED_VM_METADATA_BUNDLE_COMPLETION.md`](../worksets/WORKSET_2026-05-27_TYPED_VM_METADATA_BUNDLE_COMPLETION.md)
Current VM direction:
[`../worksets/WORKSET_2026-05-29_SINGLE_PACKAGE_DESCRIPTOR_VM.md`](../worksets/WORKSET_2026-05-29_SINGLE_PACKAGE_DESCRIPTOR_VM.md)

## Purpose

Define the next execution-layer evolution for OxVba: a complete executable
semantic package that is the shared input to the VM, JIT, wrappers, and future
native lanes.

The package is the OxVba equivalent of an IL-plus-metadata boundary. It is not
a native IR, not Cranelift-specific, and not a second source language model. It
is the compiled project/procedure artifact that preserves enough semantics for
the VM to execute directly and for JIT/native lowerers to proceed without
re-parsing source or reconstructing typing through a parallel path.

There is one VM, and it runs this package directly. There is no execution gate
and no supported/unsupported lane classification over the package: the VM is
expected to run the full build-target feature set correctly without non-object
memory leaks. Anything it runs incorrectly is a bug to fix, not a gated lane;
object reference-cycle leaks are VBA-consistent and out of scope. The current
direction is recorded in
[`../worksets/WORKSET_2026-05-29_SINGLE_PACKAGE_DESCRIPTOR_VM.md`](../worksets/WORKSET_2026-05-29_SINGLE_PACKAGE_DESCRIPTOR_VM.md).

Declared type metadata in this package follows
[`VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md). In particular, Decimal is a
Variant subtype/runtime carrier, while `Empty`, `Null`, `Error`/`CVErr`,
`Nothing`, and missing optional arguments are value/call states rather than
ordinary declared slot types.

Expression classification, Let/Set coercion, operator behavior, assignment,
property accessor semantics, call binding, Optional/ParamArray shape, and
ByRef/ByVal aliasing/writeback follow
[`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md).

## Layering Story

Target layering:

```text
Source files, project files, references, host policy
  -> parser/resolver/type checker/project binder
  -> ExecutableSemanticPackage
       - bytecode control stream
       - declared type and slot metadata
       - expression, coercion, operator, and call-site metadata
       - procedure/module/project metadata
       - UDT, array, object, COM, and native descriptors
       - error/source/debug maps
       - helper and host capability requirements
  -> VM interpreter
  -> JIT compile plan
       -> ProcLoweringIr
       -> Cranelift CLIF
       -> machine code
  -> wrapper/export/native surfaces
```

The VM remains the reference execution engine. The JIT is a lowering consumer of
the same package, not a typed side compiler with different semantic inputs.

The package completion route is tracked by
[`EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md`](EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md).
Bytecode semantics and VM evidence obligations are tracked by
[`BYTECODE_VM_SEMANTIC_CONTRACT_V1.md`](BYTECODE_VM_SEMANTIC_CONTRACT_V1.md).
Machine-readable coercion, operator, call binding, lifecycle, cleanup, and
object/member binding tables are tracked by
[`VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md`](VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md).
The current error, cleanup, deopt, and host-policy seed rows are tracked by
[`../validation/VBA_ERROR_CLEANUP_DEOPT_HOST_POLICY_SEED_TABLE_V1.csv`](../validation/VBA_ERROR_CLEANUP_DEOPT_HOST_POLICY_SEED_TABLE_V1.csv).

## JIT Lowering Entry

`ProcLoweringIr` may consume package-owned facts and must reject or classify
facts it cannot yet lower. This is the JIT's own lowering-readiness gate, not a
VM execution gate. It is not a claim that executable JIT tracer lowering is
ready for any row that the tracer matrix still marks as blocked by
`metadata-missing`, `VM-limitation`, `test-shortcoming`, `interop-limitation`,
or `oracle-required`.

## Current Baseline

Today the closest checked-in artifact is `OxBundle`:

- `Bytecode`
- `ProcedureRuntimeMetadata`
- optional manifest/export/source/toolchain/event/dynamic-object/descriptor
  inventories

The VM now also exposes `VmExecutionPackage`, a borrowed package view over
`Bytecode` plus `ProcedureRuntimeMetadata`, with package execution and retained
snapshot helpers. That API is intentionally small: it preserves existing VM
behavior while making the package boundary explicit for later VM/JIT
differential work.

That is the right direction, but it is not yet the full semantic package. The
current bundle and bytecode carry many executable facts, while some semantic
truth still lives in compiler internals, VM behavior, runtime helper contracts,
or boundary-specific descriptors. The next evolution is to make those facts
explicit, versioned, and consumable by both VM and JIT.

The current additive descriptor surface already includes first slot, signature,
call-site, array, UDT, generic object, COM/native/export interop,
error-routing, deopt-snapshot, and host-policy descriptor evidence, plus runtime
project evidence for VM-capable class/interface and COM `WithEvents` routes.
Those facts are evidence and package scaffolding, not a claim that VM
object/member, boundary, error, cleanup, or host-policy execution is fully
descriptor-driven yet.

## Required Package Contents

A complete executable semantic package must contain, or reference by stable
digest, all facts needed to execute and lower a project:

- bytecode instruction stream, slot counts, entry PCs, and procedure routes;
- procedure signatures, parameter slots, return slots, and declared calling
  shapes;
- expression classification, operator descriptors, Let/Set coercion
  descriptors, and call-site argument binding descriptors;
- declared slot metadata for parameters, locals, returns, temporaries, and
  compiler-generated control slots;
- declared carrier/layout metadata for primitive scalars, `BStr`, `ObjectRef`,
  `SafeArray`, UDT structs, and declared `Variant` cells;
- UDT descriptors, field offsets, field carriers, copy rules, and cleanup
  obligations;
- array descriptors, bounds metadata, element carriers, resize/preserve policy,
  and enumeration metadata;
- object/class/module descriptors, default members, event routes, object
  identity rules, and project dynamic member routes;
- property accessor grouping, property value-parameter semantics, default
  member binding, Optional/default argument state, ParamArray construction, and
  ByRef alias/temp/writeback descriptors;
- COM reference/type-library metadata, late-bound and early-bound member
  descriptors, default/named-argument maps, event descriptors, and invalidation
  assumptions;
- native Declare descriptors, ABI selection policy, parameter/return marshal
  descriptors, ByRef writeback rules, and dynamic-link capability requirements;
- host-service and HAL capability requirements with deterministic unsupported
  diagnostics;
- error-state maps, `On Error` targets, resume targets, source/bytecode maps,
  and debug/profiling identities;
- error transition descriptors for enabled-vs-active handlers, fault-site
  tracking, `Err` snapshot/reset behavior, handler unwinding, fallible operation
  edges, and host/COM/native error projection;
- helper ABI version requirements and semantic helper categories used by the
  bytecode;
- runtime carrier/layout version and package digest for cache and evidence
  correlation.

## Package Invariants

- Bytecode is the executable control stream, but bytecode alone is not the full
  semantic authority for JIT/native lowering.
- Declared VBA types must survive into the package. Primitive and UDT lanes are
  first-class package metadata, not inferred from retained `Variant` snapshots.
- VM execution must consume the same package facts that the JIT consumes where
  those facts affect semantics.
- `Variant` is a declared type and boundary representation, especially for COM
  VARIANT projection. It is not the universal internal carrier for JIT planning.
- COM/native descriptors are part of the semantic package, not ambient runtime
  state discovered by generated code.
- VBA errors are executable control flow plus mutable runtime state. They are
  not merely diagnostics, and the JIT must not infer or reinterpret them from
  helper failures.
- If a backend cannot legally execute a package feature, it reports a stable
  unsupported/deopt diagnostic. It must not silently choose a different
  semantic path.
- Package digests must cover every fact that can change execution or lowering.
  The bundle exposes a content digest (`OxBundle::content_digest`) over the full
  serialized payload as the package/JIT cache key, and a payload integrity digest
  embedded in the serialized header.

## Forward-Execution Package: Non-Round-Tripped Fields

The bundle is the forward execution/JIT input, not a source-reconstruction artifact.
Pure source-reconstruction facts are intentionally not serialized and are not
recoverable from a bundle: `CompiledProject::rewritten_source` and
`CompiledProject::reference_visible_exports`. This is a deliberate fidelity boundary,
not a loss of executable facts. Executable COM/native/export facts (descriptor and
export inventories, COM class exports, routes) are **not** in this category and must
be present and round-tripped.

## VBA Error Semantics Boundary

The executable semantic package must model VBA error behavior as a state
machine over the current procedure frame, caller frames, mutable `Err` fields,
fallible operations, cleanup obligations, and host/boundary projections.

At minimum, package-owned error descriptors must be able to represent:

- per-procedure error mode: no enabled handler, `On Error Resume Next`,
  `On Error GoTo <label>`, and disabled handling after `On Error GoTo 0`;
- enabled handler versus active handler state, including the rule that an error
  raised while the current handler is active must search caller frames for an
  enabled but inactive handler before becoming fatal;
- fault-site and resume-site identity for `Resume`, `Resume Next`, and
  `Resume <label>` legality;
- `Err` snapshot fields, default-property reads, explicit `Err.Clear`, and
  automatic reset points;
- call-out behavior for `On Error Resume Next`, including the caller-side resume
  point after a failed call out of the procedure containing the statement;
- fallible helper, coercion, bounds, allocation, COM, native, host, and cleanup
  edges that can update `Err`, transfer control, request deopt, or become
  fatal;
- boundary projection from COM `HRESULT`/`EXCEPINFO`/`ArgErr`, native
  return/`LastDLLError` policy, exported-callable error policy, and host
  capability diagnostics into package-visible state.

This model can be fully described in the package contract, but parity claims
require evidence. Any uncertain VBA quirk must be recorded as an oracle-needed,
test-shortcoming, VM-limitation, interop-limitation, or package-metadata gap.
It must not be filled in by `ProcLoweringIr`, Cranelift lowering, or a helper
shortcut. The JIT only lowers package-owned error maps and deopt snapshots.

Reference behavior to anchor oracle rows includes Microsoft VBA documentation
for the [`On Error` statement](https://learn.microsoft.com/en-us/office/vba/Language/Reference/User-Interface-Help/on-error-statement),
[`Err` object](https://learn.microsoft.com/en-us/office/vba/Language/Reference/user-interface-help/err-object),
and [`Err.Clear`](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/clear-method-visual-basic-for-applications).

Current VM package evidence covers the selected TB03 `On Error Resume Next`
division helper seed, selected `Resume` opcode descriptor rows, `Err.Clear`,
`Raise`, call-frame isolation, host-boundary error routing, helper/call/boundary
deopt snapshots, and host capability diagnostic descriptors. Active-handler
reentry, caller unwinding, complete Resume edge cases, explicit cleanup-stack
execution, and behavior-driving host-policy consumption remain classified
package gaps until separate VM/oracle evidence exists.

## VM Strengthening Contract

The VM is the executable truth, so package completion is also VM strengthening.
For each capability needed by JIT tracer bullets:

- if the VM already executes it, add VM-runnable fixtures and snapshot evidence;
- if the VM can execute it but lacks tests, classify the gap as a test
  shortcoming and add coverage;
- if the VM lacks the capability, classify it as a real VM/package limitation
  and implement or explicitly defer it before claiming JIT parity;
- if metadata is missing from the package, do not repair it inside the JIT
  lowerer. Add the missing package metadata first.

The VM/JIT differential harness must run the same package through both engines
with identical host services, policy, descriptors, and initial state.

## First VM Rework Boundary

The first VM rework pass should prove the package boundary before changing
execution behavior. It may add descriptor views, descriptor ids/digests, VM
evidence fields, and fixtures that compare current VM behavior with package
facts. It should not rewrite runtime slot storage, change helper behavior,
change COM/native boundary behavior, or activate JIT execution.

The ordered readiness slices and first-batch details live in
[`EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md`](EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md)
and
[`BYTECODE_VM_SEMANTIC_CONTRACT_V1.md`](BYTECODE_VM_SEMANTIC_CONTRACT_V1.md).
The delivery owner for completing the remaining full typed package is
[`../worksets/WORKSET_2026-05-27_TYPED_VM_METADATA_BUNDLE_COMPLETION.md`](../worksets/WORKSET_2026-05-27_TYPED_VM_METADATA_BUNDLE_COMPLETION.md).

## Relationship To ProcLoweringIr

`ProcLoweringIr` is downstream of the executable semantic package.

The package answers "what this compiled project/procedure means." `ProcLoweringIr`
answers "how this package procedure is lowered into a verified backend-facing
shape for one target/profile."

`ProcLoweringIr` must not contain semantic discoveries that are absent from the
package. If lowering needs slot carrier metadata, UDT layout, COM member shape,
native ABI facts, cleanup obligations, or error/resume targets, those facts must
come from the package or a versioned descriptor referenced by the package.

## Evolution Steps

1. Document the current `OxBundle` fields as the seed package.
2. Inventory which JIT tracer-bullet facts are already represented in
   bytecode/bundle metadata and which still live only in compiler/VM/runtime
   code.
3. Add package metadata for missing primitive, UDT, array, object, COM/native,
   error, source-map, and cleanup facts before using those facts in JIT
   lowering.
4. Move VM setup paths to consume package metadata consistently instead of
   reconstructing per-feature facts from side channels.
5. Make VM/JIT differential evidence cite the package digest and descriptor
   digests used for both engines.
6. Only after those gates, allow JIT implementation worksets to lower package
   procedures into `ProcLoweringIr`.

## Non-Goals

- This draft does not freeze an on-disk binary encoding.
- This draft does not require a new crate or public API immediately.
- This draft does not activate `oxvba-jit`.
- This draft does not make Cranelift IR the semantic contract.
- This draft does not require replacing current `OxBundle` in one step; it
  defines the target shape that `OxBundle` or its successor must grow into.

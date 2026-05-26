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

## Purpose

Define the next execution-layer evolution for OxVba: a complete executable
semantic package that is the shared input to the VM, JIT, wrappers, and future
native lanes.

The package is the OxVba equivalent of an IL-plus-metadata boundary. It is not
a native IR, not Cranelift-specific, and not a second source language model. It
is the compiled project/procedure artifact that preserves enough semantics for
the VM to execute directly and for JIT/native lowerers to proceed without
re-parsing source or reconstructing typing through a parallel path.

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
Bytecode/VM consumption and evidence obligations are tracked by
[`BYTECODE_VM_SEMANTIC_CONTRACT_V1.md`](BYTECODE_VM_SEMANTIC_CONTRACT_V1.md).
Machine-readable coercion, operator, call binding, lifecycle, cleanup, and
object/member binding tables are tracked by
[`VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md`](VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md).

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
- If a backend cannot legally execute a package feature, it reports a stable
  unsupported/deopt diagnostic. It must not silently choose a different
  semantic path.
- Package digests must cover every fact that can change execution or lowering.

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

# Executable Semantic Package VM Strengthening Workset

Status: `planned`
Date: 2026-05-26
Scope owner: OxVBA compiler/VM/native-readiness

## Purpose

Strengthen bytecode, metadata, and VM evidence so the executable semantic
package can become the formal input to both VM execution and future JIT
lowering. This workset is documentation and VM/package strengthening only: it
does not implement JIT execution or activate Cranelift.

## Reference Package

- Package target:
  [`../spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](../spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md)
- Completion map:
  [`../spec/EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md`](../spec/EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md)
- Bytecode and VM semantic contract:
  [`../spec/BYTECODE_VM_SEMANTIC_CONTRACT_V1.md`](../spec/BYTECODE_VM_SEMANTIC_CONTRACT_V1.md)
- Semantic tables and binding:
  [`../spec/VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md`](../spec/VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md)
- Type system:
  [`../spec/VBA_TYPE_SYSTEM_V1.md`](../spec/VBA_TYPE_SYSTEM_V1.md)
- Expression/call semantics:
  [`../spec/VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](../spec/VBA_EXPRESSION_CALL_SEMANTICS_V1.md)

## Scope

In scope:

- map current bytecode, compiler metadata, `OxBundle`, VM, runtime, COM/native,
  and host facts to the target semantic package;
- make bytecode semantics explicit enough for VM and future JIT consumers;
- define VM evidence that proves metadata use, not just final values;
- extract or plan machine-readable coercion, operator, call binding, lifecycle,
  cleanup, and object/member binding tables;
- classify each uncovered gap as test, metadata, VM, runtime, interop, oracle,
  or deferred-extension work.

Out of scope:

- JIT implementation;
- Cranelift dependency activation;
- broad runtime rewrites not directly needed for package/VM truth;
- declaring a semantic area complete without VM evidence or an explicit
  deferred/oracle classification.

## Review Findings And Repairs

This review found four unclear areas and repairs them in the referenced docs:

1. The completion map needed an ordered VM rework path, not just a descriptor
   inventory. It now defines VMR-01 through VMR-06 readiness slices and a
   metadata/evidence-only first boundary.
2. The bytecode/VM contract needed code touchpoints and a first implementation
   batch. It now identifies the compiler, bundle, VM, host, and fixture surfaces
   for the additive descriptor/evidence pass.
3. The semantic table reference needed seed targets. It now defines minimum
   coercion, operator, call, lifecycle, and object/member rows suitable for
   VM-runnable fixtures.
4. This workset needed an operational sequence for VM changes. The sequence
   below makes metadata/evidence the first delivery slice and postpones
   behavior-affecting consumption until gaps are classified.

## VM Rework Sequence

1. Metadata-only package enrichment: add or shape descriptor views for package
   identity, procedure identity, bytecode digest, signatures, slots, and
   initial carrier hints.
2. VM evidence enrichment: record descriptor digests, package facts, slot
   snapshots, and call/cleanup/interop observations for VM-runnable fixtures.
3. Metadata consumption without behavior change: load descriptors into VM setup
   and host execution paths, but keep current execution decisions unchanged.
4. Targeted behavior-driving metadata: only after fixture classification, let
   selected descriptors drive call binding, array bounds, coercion, cleanup, or
   object/member behavior.
5. Fixture expansion and gap classification: every failed or missing fixture is
   classified in the completion map before JIT planning treats the area as
   ready.

## First Implementation Batch

- Add descriptor structs or borrowed package views only where needed for the
  first fixtures.
- Populate package, procedure, bytecode, signature, and slot descriptor evidence
  for simple procedures.
- Keep current VM slot execution unchanged.
- Add tests proving descriptors are present and current snapshots are
  unchanged.
- Update completion-map rows from `metadata-missing` to the correct narrower
  status only when evidence supports the change.

## Execution Epics

1. **Completion map pass**
   - Fill the completion map rows with current source locations, support state,
     gap kind, and next action.
   - Close when every package descriptor family has an owner and gap kind.
2. **Bytecode semantic catalog pass**
   - Add opcode/family rows for slot effects, helpers, type requirements,
     error edges, cleanup edges, and snapshot obligations.
   - Close when first JIT tracer families and high-risk VM families are mapped.
3. **VM consumption and evidence pass**
   - Make VM package APIs consume or expose descriptor facts where behavior
     depends on them.
   - Close when VM evidence records package/descriptor digests and semantic
     observations for the first strengthened fixtures.
4. **Semantic tables and binding pass**
   - Extract first Let/Set, operator, call-site, lifecycle, cleanup, and
     object/member binding rows.
   - Close when rows cover primitive, Variant, string, array, UDT, object,
     Optional/ParamArray, and ByRef/ByVal seed cases.
5. **Fixture and gap classification pass**
   - Add VM-runnable fixtures for each seed semantic family.
   - Close when every fixture failure is classified as test-shortcoming,
     metadata-missing, VM-limitation, runtime-limitation, interop-limitation,
     oracle-required, or deferred-extension.

## Acceptance Gates

- The completion map has no unclassified rows for first-slice package facts.
- Bytecode family rows exist for the VM-runnable semantic seed fixtures.
- VM evidence includes slot snapshots plus descriptor/package observations.
- Coercion/operator/call/lifecycle/object rows exist for the initial seed
  families.
- The first VM batch has no runtime slot storage rewrite and no JIT behavior
  change.
- Each behavior-affecting VM change has a fixture before or with the change.
- Every first-pass gap is classified in the completion map before it becomes a
  JIT dependency.
- JIT v2 planning can cite package descriptors for each first tracer without
  inventing a parallel typed path.

## Verification

Run at least:

```text
./scripts/check-governance.ps1
git diff --check
```

When code or fixtures change, also run the impacted crate tests and
`./scripts/run-jit-v2-tracer-fixtures.ps1`.

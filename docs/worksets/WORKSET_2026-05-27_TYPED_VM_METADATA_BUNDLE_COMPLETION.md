# Typed VM Metadata Bundle Completion Workset

Status: `in-progress`
Date: 2026-05-27
Scope owner: OxVBA compiler/VM/package/native-readiness

## Purpose

Complete the OxVba executable semantic package into a lossless typed metadata
bundle for project code, declarations, imports, interop boundaries, and VM
execution. This is the next VM/package layer required before the Cranelift JIT
can treat bytecode plus metadata as its formal input.

This workset does not implement JIT execution and does not activate Cranelift.
It closes the remaining VM/package representation and evidence gaps so the VM
and future JIT consume the same semantic package instead of drifting into a
parallel typed JIT path.

## Current State

The previous VM-strengthening workset is complete:
[`WORKSET_2026-05-26_EXECUTABLE_SEMANTIC_PACKAGE_VM_STRENGTHENING.md`](WORKSET_2026-05-26_EXECUTABLE_SEMANTIC_PACKAGE_VM_STRENGTHENING.md).

That work delivered the first executable-semantic-package foundation:

- package/procedure/bytecode identity evidence;
- slot descriptor evidence for parameters, locals, returns, temporaries,
  compiler-generated slots, primitive/String/Variant slots, UDT base/field
  aliases, fixed/dynamic arrays, and generic object slots;
- procedure signature descriptors and first call-site observations;
- seed array, UDT, object, COM, native, lifecycle, coercion, operator, and
  object/member binding evidence;
- selected descriptor-driven VM behavior for:
  - direct `Long` argument to declared `Double ByVal` call entry,
  - rank-1 fixed/static `LBound`/`UBound`,
  - UDT BSTR-owning field lifecycle evidence;
- JIT readiness gates that keep TB01 through TB09 blocked until each tracer's
  consumed package/VM evidence exists.

That work intentionally did not claim a full lossless typed package, full VM
semantic parity, complete interop descriptors, or JIT readiness.

Current progress:

- `bd-tvmb.1` has introduced package-owned descriptor identity helpers in the
  compiler/package crate, a stable current `VbaTypeId` registry, canonical
  descriptor digests, and VM-visible descriptor identity evidence for the
  descriptor families reached by current package evidence. This is an identity
  and evidence step only; downstream beads still own project/import, value
  state, call, expression, aggregate, object, interop, error, cleanup, and
  descriptor-driven execution closure.
- `bd-tvmb.2` has added `OxBundle` v11 project context inventory for module
  options, `Def*` default-type families, manifest/builtin conditional
  constants, pointer-width facts, references/import resolution state,
  referenced-project summaries, compiler source maps, native Declare library
  facts, host capability requirements, and deterministic package
  diagnostics/gap classifications. VM evidence exposes this project context
  and includes it in the package digest. This preserves facts for later beads;
  it does not yet make type-library/native/host-policy behavior complete or
  descriptor-driven.
- `bd-tvmb.3` has added `OxBundle` v12 procedure carrier-layout and
  value-state descriptors. Package metadata now records primitive, String,
  Variant, object, UDT, and Decimal96-Variant-subtype carrier layout facts,
  plus VM-visible value-state rows for Empty, Null, Error/CVErr, Nothing,
  missing optional arguments, omitted defaults, vbNullString, and Decimal as a
  Variant subtype extension. This is package/evidence coverage; later beads
  still own expression/operator/call/property/interop propagation and broader
  descriptor-driven VM consumption.
- `bd-tvmb.4` has added `OxBundle` v13 call-site policy metadata for
  invocation syntax, source argument evaluation order, and diagnostic ownership
  of the current compiler-owned 448/449/450 invalid-call cases. VM package
  evidence now emits these call policy tokens alongside existing ByRef,
  Optional, ParamArray, return-copyout, and selected call-entry coercion facts.
  This keeps Optional `Variant` missing behavior and broader COM/native/export
  call binding classified as later VM/oracle/interop work.

## Reference Truth

This workset is the delivery owner for the remaining gaps already identified in:

- [`../spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](../spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md)
- [`../spec/EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md`](../spec/EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md)
- [`../spec/BYTECODE_VM_SEMANTIC_CONTRACT_V1.md`](../spec/BYTECODE_VM_SEMANTIC_CONTRACT_V1.md)
- [`../spec/VBA_TYPE_SYSTEM_V1.md`](../spec/VBA_TYPE_SYSTEM_V1.md)
- [`../spec/VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](../spec/VBA_EXPRESSION_CALL_SEMANTICS_V1.md)
- [`../spec/VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md`](../spec/VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md)
- [`../spec/JIT_V2_PROC_LOWERING_IR_V1.md`](../spec/JIT_V2_PROC_LOWERING_IR_V1.md)
- [`../validation/JIT_V2_TRACER_BULLET_MATRIX_V1.csv`](../validation/JIT_V2_TRACER_BULLET_MATRIX_V1.csv)

The VM remains executable truth. Current VM behavior is preserved until a
selected descriptor-consuming behavior change has its own fixture, evidence,
gap classification, and fresh-eyes review.

## Completion Definition

This workset is complete only when all of these are true:

1. The package has a stable, versioned, lossless typed representation of the
   project facts needed by VM execution and first JIT lowering.
2. Project modules, procedures, class/interface/object facts, references,
   imports, COM type-library facts, native Declare facts, exported-callable
   facts, and host capability requirements are package facts, not ambient
   side-channel state.
3. Every procedure has bytecode, source/bytecode maps, declared slot metadata,
   expression/call descriptors, signature descriptors, error maps, cleanup
   maps, and runtime carrier requirements where those facts affect execution.
4. Every declared value lane preserves the distinction between source type,
   declared semantic type, runtime carrier, and external ABI projection.
5. Decimal is modeled as a Variant subtype/runtime carrier, not an ordinary
   declared variable type, unless a separately gated extension explicitly says
   otherwise.
6. Empty, Null, Error/CVErr, Nothing, missing optional arguments, omitted
   defaults, and vbNullString are represented as value/call/projection states,
   not ordinary declared types.
7. VM package execution consumes descriptor facts for selected behavior only
   after VM-runnable evidence proves current and intended behavior.
8. All TB01 through TB09 package/VM evidence gates are either satisfied or
   explicitly deferred with deterministic unsupported diagnostics. A tracer
   cannot be marked executable-JIT-ready because the VM happens to run a
   source fixture.
9. Governance, VM package fixtures, JIT v2 tracer fixtures, and relevant
   compiler/runtime/host tests pass.

## Non-Goals

- No Cranelift dependency activation.
- No JIT execution.
- No replacement VM value model.
- No broad slot-storage rewrite as the first step.
- No silent VM fallback that hides unsupported JIT/package facts.
- No JIT-only rediscovery of types, imports, COM signatures, native ABI, or
  cleanup policy.

## Lossless Package Requirements

### 1. Descriptor Registry And Identity

Required:

- central `VbaTypeId` registry for scalar, String, array, UDT, enum, object,
  class, interface, imported COM, procedure, and internal control types;
- canonical descriptor IDs for slot, type, procedure signature, call site,
  expression, operator, coercion, lifecycle, array, UDT, object/member, COM,
  native, export, error, helper, and host-capability descriptors;
- stable descriptor digests covering every fact that can change VM behavior,
  JIT lowering, helper choice, cleanup, deopt, interop, or diagnostics;
- explicit `Unknown`/unsupported descriptor states that block JIT/native entry
  deterministically.

Acceptance:

- package digests change when any semantic fact changes;
- descriptor IDs are package-owned, not allocated by the VM or JIT;
- VM evidence can report descriptor IDs/digests for every procedure it runs.

### 2. Project, Module, Reference, And Import Surface

Required:

- project identity, module identity, module kind, visibility, Option flags,
  source spans, and source/bytecode maps;
- module option and compile-context facts for `Option Explicit`,
  `Option Compare`, `Option Base`, `Option Private Module`, `Def*` default type
  families, conditional compilation constants, `VBA7`, `Win64`, `PtrSafe`,
  `LongPtr`, and `LongLong`;
- reference/import graph, including source-project references, COM typelibs,
  imported COM coclasses/interfaces, event sources, and native libraries;
- host capability requirements for COM activation, dynamic library loading,
  filesystem/environment policy, debug policy, and unsupported target policy;
- deterministic diagnostics for missing or unsupported project/import facts.

Acceptance:

- a VM package run can prove which project/module/reference/import facts were
  available to the procedure;
- a JIT support query can reject unsupported imports without reparsing source.

### 3. Declared Types, Carriers, And Value States

Required:

- complete scalar declared type descriptors for Boolean, Byte, Integer, Long,
  LongLong, LongPtr, Single, Double, Currency, Date, String, and Variant;
- primitive carrier/layout descriptors for every scalar lane the JIT may place
  in a native frame;
- declared Variant descriptors tied to COM-compatible VARIANT projection where
  boundaries require it;
- Decimal as `Decimal96` Variant subtype/runtime payload, with declared
  Decimal storage rejected or extension-gated;
- explicit value/call/projection states for Empty, Null, Error/CVErr, Nothing,
  missing optional argument, omitted default, and vbNullString;
- value-state propagation descriptors for arithmetic, comparison, Boolean,
  string, call binding, property/default-member, COM/native, and error paths;
- default initialization facts per slot and per UDT/array field.

Acceptance:

- no declared slot type is inferred from retained `Variant` snapshots;
- primitive and UDT lanes are first-class metadata;
- unsupported or extension-only types produce stable diagnostics.

### 4. Slots And Procedure Signatures

Required:

- slot descriptors for parameters, locals, returns, temporaries, hidden
  receiver/`Me`, compiler-generated slots, fixed-array element slots, UDT base
  slots, and field aliases;
- full procedure signature descriptors for Subs, Functions, Property Get/Let/
  Set, events, event handlers, external Declare, and exported callables;
- parameter order, name identity, role, ByRef/ByVal source mechanism, resolved
  mechanism, Optional/default values, missing-state policy, ParamArray shape,
  property value parameter semantics, return slot/type, and source/bytecode
  entry metadata.
- VBA call syntax shape, including `Call` versus no-`Call`, parentheses that
  force expression/ByVal behavior, argument evaluation order, and no-writeback
  temporary policy.

Acceptance:

- VM call evidence can compare runtime argument binding to package signature
  and call-site descriptors;
- Optional Variant missing behavior is either VM-compatible and evidenced or
  explicitly marked unsupported/deferred.

### 5. Expressions, Operators, Coercion, And Assignment

Required:

- expression classification descriptors for value, variable, property,
  function result, member access, default member, literal, array access, and
  temporary forms;
- name/member binding descriptors for project/module/class/library precedence,
  hidden globals, `With` context, imported members, default members, and
  ambiguous name diagnostics;
- operator descriptors for arithmetic, string concatenation, comparisons,
  Boolean logic, truthiness, branch predicates, Null/Empty/Error behavior, and
  internal fast paths;
- side-effect ordering descriptors for non-short-circuit `And`/`Or`, `IIf`
  branch evaluation, property calls, COM/native calls, helper failures, and
  ByRef writebacks;
- coercion descriptors for Let/Set, ByVal call entry, ByRef compatibility,
  Optional/default assignment, property value assignment, COM/native boundary
  projection, and error cases;
- assignment/property descriptors for Set, Let, Property Get/Let/Set pairing,
  default members, and value-parameter ByVal semantics.

Acceptance:

- helper selection is driven by package-owned descriptor IDs for selected
  behavior changes;
- incompatible or oracle-required cases remain explicit and are not guessed by
  JIT lowering.

### 6. Calls, ByRef, Optional, ParamArray, And Diagnostics

Required:

- call-site and argument-binding descriptors for positional, named, omitted,
  duplicate, too-many, missing-required, ParamArray, property/default-member,
  COM/native, and exported-callable shapes;
- parse-shape and invocation-form descriptors for `Call`, no-`Call`, nested
  parentheses, statement-call versus expression-call context, and default-member
  call fallback;
- ByRef alias, ByRef expression temporary, ByVal copy, no-writeback temp, and
  writeback policy descriptors;
- ParamArray element type, lower/upper bound, empty-shape, element coercion,
  lifetime, and non-Variant projection descriptors;
- package-owned diagnostic descriptors if ownership moves from compiler
  diagnostics into VM call binding.

Acceptance:

- current compiler diagnostics 448/449/450 remain stable unless a selected VM
  behavior lane intentionally takes ownership;
- ByRef copyback behavior is VM/oracle-backed for every source form consumed by
  descriptor-driven execution.

### 7. Strings And BSTR Lifetime

Required:

- variable String and fixed-length `String * N` descriptors outside UDTs and
  inside UDT fields;
- assignment truncation/padding behavior and vbNullString projection policy;
- concat/Len/helper descriptors;
- BSTR allocation, ownership, release, branch-exit, return, error-exit,
  helper-failure, and deopt cleanup maps;
- lifetime counters or equivalent evidence strong enough for VM/JIT equality.

Acceptance:

- TB04 can close without relying on UDT-only BSTR lifecycle evidence;
- BSTR cleanup is explicit for normal strings, fixed strings, helper temps, and
  interop boundary temps.

### 8. Arrays And SAFEARRAY

Required:

- dynamic/static/fixed/local/UDT-field/ParamArray/Array-function storage
  descriptors;
- rank, declared bounds, runtime bounds, Option Base provenance, element type,
  element carrier, resize/ReDim/ReDim Preserve legality, and For Each
  enumeration facts;
- unallocated dynamic array state, `Erase`, `Array()` lower-bound behavior,
  explicit lower-bound precedence, and `ReDim Preserve` last-dimension rules;
- bounds-error behavior, multi-rank fixtures, element lifetime, and cleanup
  maps;
- COM/native SAFEARRAY projection descriptors, including lower bounds and
  element VARTYPE preservation.

Acceptance:

- TB05 has VM evidence for positive paths, bounds errors, multi-rank cases,
  lifecycle ownership, and COM/native projection;
- package execution consumes array descriptors only in selected fixture-backed
  behavior lanes.

### 9. UDTs, Enums, And Aggregate Layout

Required:

- nominal UDT IDs, project/module/name identity, visibility, ordered fields,
  nested UDT references, field types, fixed strings, fixed arrays, init/copy/
  assignment/cleanup rules, and descriptor-backed field offsets/layout;
- descriptor-backed UDT field load/store, whole-copy independence, and cleanup
  execution for selected lanes;
- enum descriptors with nominal identity, members, values, underlying Long
  behavior, and coercion rules;
- explicit separation between internal UDT semantics and any native ABI struct
  materialization.

Acceptance:

- TB02 can prove field offsets/layout, copy, cleanup/deopt materialization,
  and no accidental Variant boxing of non-Variant fields.

### 10. Objects, Classes, Interfaces, Events, And Default Instances

Required:

- descriptors for Object, VBA classes, implemented interfaces, imported COM
  classes/interfaces, WithEvents variables, As New activation, default
  instances, Nothing, default members, and object identity;
- Implements mapping from class members to interface members;
- `As New` lazy activation, class initialization/termination hooks, default
  instance lifetime, reference-cycle limits, and cleanup ordering under errors
  where in scope;
- event declaration, event handler, RaiseEvent, and WithEvents binding
  descriptors;
- object lifetime/cleanup maps and Set/Nothing compatibility facts.

Acceptance:

- object identity is package-visible and snapshot-comparable;
- default-member/property behavior is not inferred by late helper fallback;
- COM imported types and source-project types are represented in one package
  model with distinct projection descriptors.

### 11. COM, Native Declare, And Exported Callable Projection

Required:

- late-bound COM activation/dispatch descriptors, named/default member maps,
  HRESULT/EXCEPINFO/ArgErr/Err projection fields, object identity observations,
  and cleanup facts;
- early-bound COM typelib/reference descriptors, imported coclass/interface/
  member descriptors, dispatch-vtable strategy, object identity, argument/
  return projection, and invalidation assumptions;
- native Declare ABI descriptors for calling convention, platform, library/
  alias/name, scalar/BSTR/Variant/SAFEARRAY/UDT/object projections, ByRef
  writeback, cleanup buffer ownership, and error policy;
- native and COM boundary quirks for optional/missing arguments, named/default
  arguments, BSTR/SAFEARRAY ownership, `ByRef Variant`, `IErrorInfo`,
  `LastDLLError`, unsupported `VT_BYREF` results, and host apartment/policy
  requirements;
- exported callable inbound/outbound ABI projection descriptors, ByRef
  writeback, cleanup/error return policy, and unsupported-shape diagnostics.

Acceptance:

- TB06 through TB09 can consume package descriptors without ambient symbol
  lookup or ad hoc boundary rediscovery;
- unsupported COM/native/export shapes produce deterministic diagnostics.

### 12. Error Routing, Cleanup, Deopt, And Host Policy

Doctrine:

- VBA errors are executable control flow plus mutable runtime state, not merely
  diagnostics.
- The package must describe the full error state machine; VM/JIT parity claims
  require VM-runnable or oracle-backed evidence for the reached quirks.
- `ProcLoweringIr` and Cranelift may only consume package-owned error maps,
  cleanup maps, and deopt snapshots. They must not invent alternate `On Error`
  behavior or infer semantics from helper failure returns.

Required:

- package-owned On Error maps, resume targets, Err-state snapshot fields,
  failing-helper descriptors, and deopt snapshot fields;
- enabled-vs-active handler state, fault-site tracking, legal `Resume` forms,
  caller-frame unwinding, call-out `Resume Next` behavior, `Err.Clear`, and
  automatic `Err` reset points;
- explicit fallible-operation descriptors for helper, coercion, bounds,
  allocation, COM, native, host, exported-callable, cleanup, and deopt edges;
- cleanup obligation maps for every owning carrier: BSTR, SafeArray, ObjectRef,
  Variant payloads, Decimal payloads, UDT fields, ByRef temps/writebacks,
  COM/native temps, exported-callable temps, and helper temps;
- safepoint/live-carrier maps for future JIT deopt and helper calls;
- host capability requirements and unsupported diagnostics as digestable
  package facts.

Acceptance:

- TB03 can prove error state, resume target, and slot-state equality through
  package error maps;
- uncertain VBA error quirks are classified as oracle-needed, test-shortcoming,
  VM-limitation, interop-limitation, or metadata-missing rows rather than being
  delegated to the JIT;
- cleanup/deopt evidence is strong enough that the JIT never has to invent a
  parallel cleanup model.

### 13. Evidence, Harness, And Fresh-Eyes Review

Required:

- VM package evidence schema covering descriptors, digests, snapshots,
  call-binding observations, error state, cleanup/lifetime, arrays, UDT fields,
  object identity, COM/native/export descriptors, host policy, and unsupported
  diagnostics;
- differential harness plan that runs the same package through VM and future
  JIT with identical HostServices and policy;
- Office/MS-VBAL/MS-OAUT oracle references for compatibility-sensitive
  behavior such as Optional missing, ByRef expression forms, Array() bounds,
  default members, COM errors, and SAFEARRAY details;
- fresh-eyes review after each behavior-affecting descriptor consumption.

Acceptance:

- every closure claim cites VM-runnable evidence or an explicit oracle/deferred
  classification;
- every full-fidelity risk row below has an owning bead and is either covered
  by package descriptors plus evidence or explicitly classified as deferred;
- green tests are only used as evidence after they cover the claimed behavior.

## Full-Fidelity Semantic Risk Gates

These risks must be reviewed before behavior-affecting VM rework or JIT
implementation entry. Closure does not require implementing every VBA feature
immediately, but it does require that the package has a descriptor/evidence
story or an explicit limitation classification.

| Risk area | Owning beads | Required closure shape |
|---|---|---|
| Name/member binding, hidden globals, `With` context, default members, and library/project precedence | `bd-tvmb.2`, `bd-tvmb.5`, `bd-tvmb.7` | Binding descriptors and diagnostics are package-owned for selected lanes; ambiguous or unsupported cases are not resolved by JIT-local lookup. |
| Call syntax, parentheses, argument evaluation order, ByRef temps/writeback, Optional, and ParamArray | `bd-tvmb.4`, `bd-tvmb.5`, `bd-tvmb.10` | Call-site descriptors preserve parse shape and binding outcome; VM evidence or oracle rows cover every source form consumed by descriptor-driven execution. |
| Let/Set/default-member coercion and object-versus-value assignment | `bd-tvmb.5`, `bd-tvmb.7` | Let/Set/default-member behavior is table-backed; object identity, `Nothing`, and property `Get`/`Let`/`Set` pairing are explicit package facts. |
| Empty, Null, Error/CVErr, Missing, Nothing, and vbNullString propagation | `bd-tvmb.3`, `bd-tvmb.5`, `bd-tvmb.9` | Value states are not ordinary declared types; propagation through operators, calls, properties, errors, and boundaries is descriptor-backed or oracle-gated. |
| Side-effect ordering, non-short-circuit `And`/`Or`, `IIf`, helper failures, and boundary calls | `bd-tvmb.5`, `bd-tvmb.9`, `bd-tvmb.10` | Evaluation order and fallible edges are package-visible; VM evidence proves ordering before lowering may specialize. |
| Module options and compile context | `bd-tvmb.2`, `bd-tvmb.5`, `bd-tvmb.6` | `Option Base`, `Option Compare`, `Option Explicit`, `Option Private Module`, `Def*`, conditional compilation, and pointer-width facts are serialized and digestable. |
| Arrays, SAFEARRAY, aggregate layout, UDT copy/init/cleanup, and enum coercion | `bd-tvmb.6`, `bd-tvmb.8`, `bd-tvmb.10` | Bounds, layout, copy, cleanup, lower-bound, multi-rank, `ReDim Preserve`, and boundary projection facts are package-owned for selected lanes. |
| Object/class lifecycle, `As New`, default instances, `Implements`, `WithEvents`, events, and cleanup under errors | `bd-tvmb.7`, `bd-tvmb.9`, `bd-tvmb.10` | Activation, identity, event routing, interface mapping, lifecycle, and cleanup state are visible to VM evidence and future deopt snapshots. |
| Locale/host-sensitive conversion and comparison | `bd-tvmb.2`, `bd-tvmb.5`, `bd-tvmb.10` | Locale, calendar, string compare, date/string/format, and host-policy dependencies are package facts or explicit oracle/host-policy gaps. |
| COM/native/export boundary quirks | `bd-tvmb.8`, `bd-tvmb.9`, `bd-tvmb.10` | Optional/missing args, named/default args, `HRESULT`/`EXCEPINFO`/`IErrorInfo`, BSTR/SAFEARRAY ownership, ByRef writeback, `LastDLLError`, and unsupported shapes have descriptor/evidence rows. |

## Per-Bead Automated Test Obligations

Each child bead must create or extend at least three automated tests, fixture
assertions, or automated evidence checks before closure. Existing tests may
count only when the closure evidence names the test and the assertions exercise
the descriptor or semantic behavior changed by that bead. A documentation-only
update is not enough to close a bead unless the corresponding automated checks
already exist and are linked.

| Bead | Minimum automated coverage before closure |
|---|---|
| `bd-tvmb.1` | Descriptor ID determinism; digest changes when a semantic descriptor changes; VM evidence emits package/procedure/descriptor digests. |
| `bd-tvmb.2` | Module option serialization/digests; conditional compilation and pointer-width facts; missing/unsupported reference or import diagnostics. |
| `bd-tvmb.3` | Primitive/object/aggregate slot carrier evidence; Empty/Null/Error/Missing/Nothing/vbNullString propagation; Decimal-as-Variant and declared-Decimal rejection/extension diagnostics. |
| `bd-tvmb.4` | `Call`/no-`Call`/parentheses parse-shape preservation; ByRef alias/temp/no-writeback/writeback behavior; Optional/named/omitted/ParamArray diagnostics and binding. |
| `bd-tvmb.5` | Name/member binding precedence and ambiguity; Let/Set/default-member/property coercion; Null/Empty/Error operator behavior plus non-short-circuit `And`/`Or` and `IIf` side-effect ordering. |
| `bd-tvmb.6` | Option Base/explicit lower bounds/`Array()` lower bounds and `ReDim Preserve`; fixed/variable string and BSTR lifetime; UDT field layout/copy/cleanup and enum coercion. |
| `bd-tvmb.7` | `As New` lazy activation/default instances; `Set`/`Nothing`/object identity and `Implements`; `WithEvents`/event routing plus lifecycle cleanup under error paths. |
| `bd-tvmb.8` | COM named/default/missing arguments and error projection; native Declare scalar/BSTR/Variant/SAFEARRAY/ByRef projection and writeback; exported-callable inbound/outbound cleanup and error policy. |
| `bd-tvmb.9` | `On Error` mode transitions including enabled-vs-active handlers; `Err.Clear`/reset/resume target behavior; cleanup/deopt snapshot state around failing helpers and boundary calls. |
| `bd-tvmb.10` | Descriptor-driven VM execution for selected lanes; raw-bytecode versus package-execution baseline comparison; automated evidence classification for VM/test/interop/oracle gaps. |
| `bd-tvmb.11` | Automated audit that all child beads cite tests; final tracer/governance/dependency/diff checks; stale gap scan across completion map, tracer matrix, semantic package spec, and VM contract. |

## Execution Epics

1. **Registry and package identity**
   - Build canonical descriptor ID/digest infrastructure and central type
     registry.
   - Gate: descriptor IDs/digests are package-owned and VM-visible.
2. **Project/reference/import bundle**
   - Preserve project, module, reference, typelib, native library, source map,
     and host capability facts.
   - Gate: unsupported imports are deterministic package diagnostics.
3. **Type, carrier, and value-state closure**
   - Finish declared types, carriers, Decimal-as-Variant, and special value
     states.
   - Gate: no declared type or carrier fact is recovered from snapshots alone.
4. **Signature/call/diagnostic closure**
   - Finish signatures, call sites, Optional/ParamArray/ByRef/default-member
     behavior, and diagnostic ownership.
   - Gate: call binding behavior is descriptor-backed for selected lanes.
5. **Expression/operator/coercion/property closure**
   - Promote seed semantic tables into canonical package descriptors.
   - Gate: helper choice and property/default-member behavior are package
     facts.
6. **String/array/UDT aggregate closure**
   - Finish BSTR, SAFEARRAY, UDT, enum, layout, bounds, copy, and cleanup
     descriptors.
   - Gate: TB02, TB04, and TB05 have enough VM/package evidence for future JIT
     closure.
7. **Object/event/COM type closure**
   - Finish object/class/interface/event/default-instance/imported-COM
     descriptors.
   - Gate: object identity and member binding are package-visible.
8. **Interop/export closure**
   - Finish late/early COM, native Declare, and exported-callable projection
     descriptors.
   - Gate: TB06 through TB09 package gates are satisfied or explicitly
     unsupported.
9. **Error/cleanup/deopt/host policy closure**
   - Finish error maps, cleanup maps, safepoints/live-carrier maps, and host
     capability diagnostics.
   - Gate: no cleanup/error/deopt obligation is backend-local.
10. **VM consumption and evidence closure**
    - Move selected behavior from runtime-only logic into descriptor-consuming
      VM execution with fixtures and fresh-eyes review.
    - Gate: package execution remains VM-compatible and raw-bytecode baselines
      are intentionally retained only where documented.
11. **Implementation-entry review**
    - Reconcile completion map, tracer matrix, semantic package spec, VM
      contract, and JIT planning workset.
    - Gate: the first JIT implementation workset can consume the package
      without inventing missing semantic facts.

## Bead Rollout

Execution bead parent: `bd-tvmb` (`Typed VM metadata bundle completion`).

Child beads:

1. `bd-tvmb.1` - Registry and package identity.
2. `bd-tvmb.2` - Project/reference/import bundle.
3. `bd-tvmb.3` - Type, carrier, and value-state closure.
4. `bd-tvmb.4` - Signature/call/diagnostic closure.
5. `bd-tvmb.5` - Expression/operator/coercion/property closure.
6. `bd-tvmb.6` - String/array/UDT aggregate closure.
7. `bd-tvmb.7` - Object/event/COM type closure.
8. `bd-tvmb.8` - Interop/export closure.
9. `bd-tvmb.9` - Error/cleanup/deopt/host policy closure.
10. `bd-tvmb.10` - VM consumption and evidence closure.
11. `bd-tvmb.11` - Implementation-entry review and package handoff.

The dependency path starts with `bd-tvmb.1`; later beads open as their
semantic prerequisites close. Every bead closure requires implementation or
evidence updates, impacted tests, governance, `git diff --check`, fresh-eyes
review, and reconciliation of affected docs, completion-map rows, tracer rows,
or blocker state.

## Required Checks

Every substantial change in this workset should run the narrow impacted tests
plus:

```text
./scripts/run-jit-v2-tracer-fixtures.ps1
./scripts/check-governance.ps1
git diff --check
```

When package serialization or VM evidence changes, also run the relevant
compiler, VM, runtime, and host package-identity test lanes.

## Consistency Review

This workset is consistent with the current closed VM-strengthening workset:
that workset delivered a scoped metadata/evidence foundation and selected
descriptor-consumption proof points. This workset owns the remaining closure
needed for a full lossless typed package.

It is also consistent with the JIT v2 planning workset: `ProcLoweringIr` must
consume the executable semantic package and may not reconstruct types, imports,
COM/native ABI, cleanup, or error semantics through a parallel JIT-only path.

Residual gaps listed here intentionally match the completion map and JIT tracer
matrix. Closing this workset requires updating those artifacts so they no
longer carry stale `metadata-missing`, `VM-limitation`, `test-shortcoming`, or
`interop-limitation` rows for the scoped package facts.

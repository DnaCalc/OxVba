# JIT v2 Implementation Design v1

Status: `planning-design`
Date: 2026-05-26
Scope owner: OxVBA JIT/native-readiness
Owning workset:
[`../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md`](../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md)
Detailed IR:
[`JIT_V2_PROC_LOWERING_IR_V1.md`](JIT_V2_PROC_LOWERING_IR_V1.md)
Differential harness:
[`JIT_V2_DIFFERENTIAL_HARNESS_V1.md`](JIT_V2_DIFFERENTIAL_HARNESS_V1.md)
Executable semantic package:
[`EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](EXECUTABLE_SEMANTIC_PACKAGE_V1.md)
Type and expression semantics:
[`VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md),
[`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md)

## Purpose

Define the implementation-ready design for the first Cranelift-based OxVba JIT
v2. This document is functional design, not implementation evidence. It names
the data model, execution ABI, lowering shape, verifier requirements, helper
contract, and first tracer-bullet semantics that implementation worksets must
follow.

The current JIT crate remains disabled until an implementation workset lands
code and evidence.

## Locked Decisions

- First supported JIT target: Windows x64 only.
- First compiled entry shape: one uniform function ABI,
  `extern "C" fn(vmctx: *mut JitVmContext, frame: *mut JitFrame) -> JitStatus`.
- First frame model: `ProcLoweringIr` carries declared slot/carrier kinds.
  Primitive scalars, `BStr`, `ObjectRef`, `SafeArray`, UDT structs, and declared
  `Variant` cells are distinct frame carriers. VM-compatible retained
  `Variant` snapshots are materialized for evidence/deopt; they are not the
  native value model. Declared `Variant` cells must preserve exact COM VARIANT
  layout.
- First expression/call model: coercion, operator, property accessor,
  Optional/ParamArray, and ByRef/ByVal call-site facts come from package
  descriptors. The JIT must not reconstruct those rules from source or ad hoc
  bytecode pattern matching.
- First COM/native strategy: descriptor-backed helper calls are the semantic
  path. Specialized COM/native call lowering may be added later only after it
  proves parity with helper behavior.
- First fallback strategy: no silent VM fallback. Unsupported target,
  unsupported bytecode, helper failure, and deopt are distinct observable
  statuses.
- First debug strategy: JIT is disabled by default in debug sessions until the
  source/bytecode mapping and conservative-debug profile are accepted.

## Public Boundary

Future `oxvba-jit` APIs should grow in this order:

1. **Support query**
   - Input: target triple, host policy, backend feature flags.
   - Output: supported/unavailable with stable diagnostic code.
   - Required diagnostic codes:
     - `JIT-TARGET-UNAVAILABLE`
     - `JIT-BACKEND-DISABLED`
     - `JIT-DEBUG-POLICY-DISABLED`
     - `JIT-UNSUPPORTED-BYTECODE`
2. **Compile plan**
   - Input: executable semantic package, optional procedure symbol,
     `HostPolicy`, helper ABI version, COM/native descriptor digest.
   - Output: `JitCompilePlan` containing support decision, bytecode digest,
     candidate procedures, unsupported PCs, and required helper symbols.
3. **Compile procedure**
   - Input: verified `ProcLoweringIr`.
   - Output: `CompiledProc` with entry pointer, frame layout, source map,
     deopt map, helper ABI version, CLIF diagnostic artifact, and verifier
     result.
4. **Execute procedure**
   - Input: compiled proc, host services, frame initialized from declared
     typed slot values or a VM-compatible materialized snapshot.
   - Output: typed frame result, VM-compatible snapshot evidence, and
     `JitStatus`.

Current APIs may keep their disabled shape until the implementation workset
intentionally expands them. They must not start falling back to VM execution.

## Compilation Pipeline

The pipeline is intentionally staged:

```text
Compiler bytecode + metadata output
  -> ExecutableSemanticPackage
  -> JitCompilePlan
  -> ProcLoweringIr
  -> ProcLoweringIr verifier
  -> CLIF lowering
  -> Cranelift verifier
  -> machine code
  -> differential harness
```

Direct bytecode-to-CLIF lowering is forbidden, and so is a parallel typed JIT
path that reconstructs semantics outside the VM/compiler package boundary. The
executable semantic package is the shared VM/JIT input. `ProcLoweringIr` is the
procedure-lowering contract where package semantics are made explicit and
reviewable for backend codegen.

Compile cache key:

```text
bytecode_digest
package_digest
procedure_symbol_or_entry_pc
target_triple
host_policy_digest
helper_abi_version
runtime_carrier_and_layout_version
com_native_descriptor_digest
debug_policy
optimization_profile
```

Any cache-key mismatch requires recompilation or deterministic unavailability.

## Runtime Entry ABI

### `JitVmContext`

`JitVmContext` is process-local runtime state owned by `oxvba-jit`. It is not a
stable external ABI. It must contain:

- helper table pointer and helper ABI version;
- `Arc<dyn HostServices>` or an opaque host-services handle;
- runtime policy/profile identity;
- COM/native descriptor tables;
- diagnostic sink;
- panic/poison flag for helper failures;
- compile-time support metadata for diagnostics.

No helper may look up ambient process symbols. All runtime helper addresses must
come from the helper table stored in `JitVmContext`.

### `JitFrame`

`JitFrame` is the VM-equivalent procedure frame:

- declared slot array, one entry per `Bytecode::slot_count`;
- per-slot carrier/layout descriptor for primitive scalars, `BStr`,
  `ObjectRef`, `SafeArray`, UDT structs, and declared `Variant` cells;
- user slot count and temporary slot count;
- current bytecode PC/source location;
- current procedure identity;
- error state mirror;
- cleanup stack;
- safepoint live-carrier map;
- byref writeback list;
- VM-snapshot materialization map for evidence/deopt;
- deopt target state;
- optional debug/profiling counters.

Slots hold declared typed carriers. A declared `Variant` slot is represented by
the COM-compatible VARIANT carrier; a primitive or UDT slot must not be boxed as
`Variant` only because the current VM snapshot API reports retained values.
Implementation may use an internal `JitSlot` wrapper, but it must not introduce
JIT-only semantics that cannot materialize the VM-equivalent snapshot.

### `JitStatus`

`JitStatus` must be a compact `repr(C)` status:

- `Ok`
- `Returned`
- `RaisedUnhandled`
- `DeoptRequested`
- `UnsupportedBytecode`
- `HelperFault`
- `HostPolicyDenied`
- `ComFailure`
- `NativeCallFailure`
- `InvariantViolation`

Detailed error information is stored in `JitFrame` or `JitVmContext`, not in the
machine-code return register.

No Rust panic or unwind may cross compiled-code boundaries. Helpers must catch
or prevent panics and return `JitStatus::HelperFault` or a routed runtime error.

## ProcLoweringIr

Detailed type and verifier requirements live in
[`JIT_V2_PROC_LOWERING_IR_V1.md`](JIT_V2_PROC_LOWERING_IR_V1.md). The summary below
is the implementation-design view.

### Procedure Shape

`ProcLoweringIr` should model one compiled procedure or one traceable procedure
slice:

```rust
struct ProcLoweringIr {
    proc_id: ProcLoweringId,
    source_symbol: Option<String>,
    entry_block: BlockId,
    blocks: Vec<ProcBlock>,
    frame_layout: ProcFrameLayout,
    helper_refs: Vec<HelperRef>,
    descriptor_refs: Vec<InteropDescriptorRef>,
    source_map: Vec<ProcSourceMapEntry>,
    deopt_points: Vec<ProcDeoptPoint>,
    cleanup_regions: Vec<CleanupRegion>,
}
```

### Blocks And Terminators

Every block has:

- stable block id;
- bytecode PC range;
- ordered ops;
- optional live-in/live-out slot set;
- terminator.

Terminators:

- `Return`
- `Jump { target }`
- `Branch { cond, if_true, if_false }`
- `CallAndContinue { target_proc, continuation, error_edge }`
- `Deopt { reason, target_pc }`
- `TrapInvariant { diagnostic }`

There are no implicit fallthroughs.

### Values And Slots

Value classes:

- declared slot reference;
- primitive SSA scalar (`I16`, `I32`, `I64`, pointer-sized integer, `F32`,
  `F64`, Boolean, Currency, Date);
- UDT aggregate reference plus field address/value;
- temporary pointer-sized value;
- helper result token;
- descriptor token;
- condition value.

Slot effects must be explicit:

- read slot;
- write slot;
- move/copy declared carrier;
- copy UDT by descriptor;
- load/store UDT field by descriptor offset and field carrier;
- initialize slot;
- clear slot;
- borrow ByRef slot;
- commit ByRef writeback.

Any op that may allocate, call a helper, call host services, call COM/native
code, route an error, or deopt is a safepoint and must carry a live-carrier map.

### Operation Families

Minimum operation families:

- constants and slot copy;
- primitive arithmetic/coercion helper call;
- declared `Variant` arithmetic/coercion helper call;
- UDT field load/store/copy helper call;
- guarded scalar arithmetic;
- string/BSTR helper call;
- SAFEARRAY helper call;
- object lifetime/helper call;
- error state get/set/route;
- branch/jump/call/return;
- late-bound COM helper call;
- early-bound COM helper call;
- native Declare helper call;
- exported callable inbound/outbound projection;
- debug/profiling marker;
- deopt snapshot.

## ProcLoweringIr Verifier

The verifier must run before CLIF lowering and reject:

- missing block terminators;
- invalid block targets;
- use before slot initialization where VM semantics do not allow Empty;
- missing helper ABI declarations;
- helper call without live-carrier map;
- allocation or interop call without cleanup state;
- error-capable op without error edge or explicit unhandled policy;
- branch leaving a cleanup region without cleanup edge;
- ByRef borrow without writeback or cancellation policy;
- COM/native descriptor reference not in descriptor table;
- deopt point missing slot, error, cleanup, byref, and source mapping;
- unsupported target or debug policy not reflected in compile plan.

The verifier output is part of tracer-bullet evidence.

## CLIF Lowering Rules

- Lower only verified `ProcLoweringIr`.
- Use a uniform `vmctx, frame -> status` signature.
- Use `cranelift_frontend` for SSA construction where it reduces boilerplate.
- Use conservative memory flags for frame slots, BSTR cells, SAFEARRAY payloads,
  object pointers, COM memory, native ByRef memory, and host-owned memory.
- Register helper symbols explicitly from `JitVmContext`/module declaration.
- Run Cranelift verifier for every compiled function in tests/debug lanes.
- Save textual CLIF and source/bytecode map artifacts for diagnostics. These
  artifacts are not semantic proof.

Initial lowering may keep most operations as helper calls. Inlining is allowed
only for:

- simple control flow;
- constants;
- slot pointer arithmetic inside verified frame bounds;
- primitive integer/float/Boolean guards that immediately deopt or call helpers
  on mismatch;
- boolean branch lowering after VM truthiness helper or proved scalar Boolean.

## Helper ABI

All helper symbols are versioned. Helper signatures must avoid passing Rust
`Variant` by value through generated-code ABI. Prefer handles/pointers into
`JitFrame`.

Recommended helper shape:

```rust
extern "C" fn(
    vmctx: *mut JitVmContext,
    frame: *mut JitFrame,
    args: *const HelperArg,
    arg_len: usize,
    result: *mut HelperResult,
) -> JitStatus
```

Each helper declaration records:

- symbol;
- helper category;
- ABI version;
- argument slot/value descriptors;
- result descriptor;
- ownership transfer;
- may allocate;
- may mutate slots;
- may set `Err`;
- may route runtime error;
- may call host services;
- may call COM/native code;
- may reenter OxVba;
- cleanup obligations;
- safepoint requirement.

Helper categories:

- primitive arithmetic/coercion;
- declared `Variant` arithmetic/coercion;
- UDT layout/copy/field access;
- string/BSTR;
- SAFEARRAY;
- object lifetime;
- error routing;
- COM late-bound;
- COM early-bound;
- native Declare;
- exported callable projection;
- debug/trace/profiling;
- deopt/snapshot.

## Error Semantics

The JIT mirrors VM error fields:

- `on_error_resume_next`;
- `on_error_goto_label_target`;
- `last_error`;
- `last_error_pc`;
- `last_error_description`;
- `last_error_source`.

Error-capable helpers return status plus error detail. JIT error routing must:

- set `last_error` and description exactly as the VM would;
- under Resume Next, advance to the next bytecode PC and clear pending
  `last_error_pc` as the VM does;
- under `On Error GoTo label`, jump to the target PC;
- under default mode, return `RaisedUnhandled`;
- preserve slot state for failed operations according to VM behavior;
- make `Err.Number`, `Err.Description`, and `ClearErr` observe identical state.

`Resume`, `Resume Next`, and `ResumeLabel` must follow the VM pending-error
rules, including error 20 when there is no pending error.

## Cleanup, Safepoints, And Deopt

Cleanup state is explicit in `JitFrame`. Cleanup entries cover:

- temporary BSTR ownership;
- temporary SAFEARRAY ownership;
- retained ObjectRef addref/release obligations;
- native marshalled buffers;
- COM call frames and EXCEPINFO/BSTR cleanup;
- ByRef writeback or cancellation.

Safepoints are required before and after:

- helper calls;
- allocation;
- BSTR/SAFEARRAY mutation;
- COM/native calls;
- host-service calls;
- deopt exits;
- returns through active cleanup regions.

Every deopt point records:

- procedure id and source/bytecode location;
- live slot map;
- declared carrier ownership and VM-snapshot materialization state;
- cleanup stack;
- error state;
- byref writeback state;
- COM/native boundary state;
- host policy/profile identity.

Deopt resumes through the VM or a later baseline JIT entry only after the frame
has been reconstructed and cleanup ownership is unambiguous.

## COM And Native Interop

COM and native interop use shared ABI descriptors. A descriptor records:

- descriptor id and digest;
- boundary kind: late COM, early COM, native Declare, exported callable;
- symbol/member selector;
- calling convention;
- param count and types;
- ByRef flags and writeback shape;
- BSTR/SAFEARRAY ownership policy;
- HRESULT/EXCEPINFO handling;
- object identity expectations;
- host policy capability required;
- failure-to-Err projection.

Late-bound COM lowering initially calls a helper equivalent to VM host dispatch.
Early-bound COM lowering initially calls a helper with metadata-derived
dispatch/vtable strategy. Native Declare lowering calls a helper using the
existing `ExternalCallDescriptor` direction from bytecode. Exported callable
lowering uses an inbound projection helper to populate `JitFrame` slots and an
outbound projection helper to return values/writebacks.

No COM/native specialization may bypass these descriptors.

## Tracer-Bullet Implementation Order

1. Primitive typed scalar loop.
2. UDT struct field/copy path.
3. Error-routing path.
4. BSTR lifetime path.
5. SAFEARRAY path.
6. Late-bound COM path.
7. Early-bound COM path.
8. Native Declare path.
9. Exported callable path.

Each bullet must land with:

- fixture source;
- VM snapshot evidence;
- JIT snapshot evidence once JIT exists;
- `ProcLoweringIr` verifier evidence;
- Cranelift verifier evidence;
- helper ABI manifest entries;
- CLIF diagnostic artifact;
- cleanup/error assertions where relevant;
- unsupported-target and unsupported-bytecode diagnostics where relevant.

## Implementation Cut Lines

Implementation work should split into these reviewable cuts:

1. JIT support query and disabled diagnostics, preserving current behavior.
2. `ProcLoweringIr` data model and verifier without Cranelift.
3. Helper ABI manifest and no-ambient-symbol registration model.
4. VM/JIT differential harness with JIT still unavailable.
5. Cranelift module integration behind Windows x64 feature gate.
6. Tracer bullets 1-2 execution over primitive and UDT typed carriers.
7. Tracer bullets 3-5 runtime semantics.
8. Tracer bullets 6-9 interop and exported callable semantics.

Each cut must keep unsupported behavior deterministic and must not report JIT
execution evidence until native code actually ran.

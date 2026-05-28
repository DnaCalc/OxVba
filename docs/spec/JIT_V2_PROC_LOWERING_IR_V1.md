# JIT v2 ProcLoweringIr v1

Status: `planning-ir`
Date: 2026-05-26
Owning workset:
[`../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md`](../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md)
Implementation design:
[`JIT_V2_IMPLEMENTATION_DESIGN_V1.md`](JIT_V2_IMPLEMENTATION_DESIGN_V1.md)
Executable semantic package:
[`EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](EXECUTABLE_SEMANTIC_PACKAGE_V1.md)
Expression/call semantics:
[`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md)
Typed metadata package handoff:
[`../validation/TYPED_VM_METADATA_BUNDLE_IMPLEMENTATION_ENTRY_AUDIT_2026-05-28.md`](../validation/TYPED_VM_METADATA_BUNDLE_IMPLEMENTATION_ENTRY_AUDIT_2026-05-28.md)

## Purpose

Specify the concrete procedure-lowering IR that JIT v2 implementation should
build before CLIF lowering. `ProcLoweringIr` is the verified per-procedure
lowering contract between the OxVba executable semantic package and
backend-specific IR such as Cranelift CLIF. It is not the semantic source of
truth, a Cranelift mirror, or a new runtime value model.

The executable semantic package answers what the compiled procedure means.
`ProcLoweringIr` answers how one package procedure is prepared for one
target/profile. Any type, slot, expression, coercion, operator, call-site, UDT,
COM/native, cleanup, error, or source-map fact required by `ProcLoweringIr`
must come from the package or a versioned descriptor referenced by the package.
If the handoff audit or tracer matrix classifies a required fact as missing,
unsupported, interop-limited, oracle-required, or test-blocked,
`ProcLoweringIr` must reject/classify that path rather than deriving the fact
from bytecode shape or backend convenience.

Strict package-only VM work adds `VmPackageSupportReport` as the current shared
support-query surface. `ProcLoweringIr` entry must treat any deferred
VM-consumption row in that report as a blocker until the owning delivery bead
either makes the behavior descriptor-driven or defines a deterministic reject.

## Core Types

Recommended implementation module: `crates/oxvba-jit/src/ir`.

```rust
pub struct ProcLoweringIr {
    pub proc_id: ProcLoweringId,
    pub symbol: Option<String>,
    pub entry_pc: usize,
    pub package_digest: String,
    pub bytecode_digest: String,
    pub support: NativeSupportDecision,
    pub frame: ProcFrameLayout,
    pub blocks: Vec<ProcBlock>,
    pub helpers: Vec<HelperRef>,
    pub descriptors: Vec<InteropDescriptorRef>,
    pub source_map: Vec<ProcSourceMapEntry>,
    pub deopt_points: Vec<ProcDeoptPoint>,
    pub cleanup_regions: Vec<CleanupRegion>,
}

pub struct ProcBlock {
    pub id: BlockId,
    pub pc_start: usize,
    pub pc_end_exclusive: usize,
    pub params: Vec<ProcBlockParam>,
    pub ops: Vec<ProcOp>,
    pub live_in: SlotSet,
    pub live_out: SlotSet,
    pub terminator: ProcTerminator,
}
```

Identifiers are stable inside one IR artifact. Do not use vector indices as
evidence ids once artifacts are serialized.

## Frame Layout

```rust
pub struct ProcFrameLayout {
    pub slot_count: usize,
    pub user_slot_count: usize,
    pub slots: Vec<ProcSlotLayout>,
    pub error_state: ProcErrorStateLayout,
    pub cleanup_stack: ProcCleanupStackLayout,
    pub byref_table: ProcByRefTableLayout,
    pub foreach_table: ProcForEachTableLayout,
}

pub struct ProcSlotLayout {
    pub slot: usize,
    pub role: ProcSlotRole,
    pub initial_state: ProcSlotInitialState,
    pub carrier: ProcCarrierKind,
}
```

Required carrier kinds:

- `I16`
- `I32`
- `I64`
- `PointerSizedInteger`
- `F32`
- `F64`
- `Boolean`
- `Currency`
- `Date`
- `BStr`
- `ObjectRef`
- `SafeArray`
- `UserDefinedStruct { descriptor_id }`
- `VariantComLayout`
- `BindingHandleInternal`
- `PointerSizedTemporary`
- `ScalarTemporary { scalar }`

`VariantComLayout` is required for declared `Variant` slots and COM/native
VARIANT boundary projection. VM-visible snapshots are materialized from the
declared carriers; they must not force primitive or UDT slots to be represented
as `VariantComLayout` inside the JIT frame.

## Slot Effects

```rust
pub enum SlotEffect {
    Read { slot: usize },
    Write { slot: usize, carrier: ProcCarrierKind },
    Move { dst: usize, src: usize },
    Clear { slot: usize },
    BorrowByRef { slot: usize, writeback: ByRefWritebackId },
    CommitByRef { writeback: ByRefWritebackId },
    CancelByRef { writeback: ByRefWritebackId },
}
```

Verifier rules:

- reads and writes target valid slots;
- `BindingHandleInternal` never flows to VM-visible result slots;
- every ByRef borrow resolves to commit or cancel on all exits;
- helper-declared effects match the helper ABI catalog.

## Operations

```rust
pub enum ProcOp {
    LoadConst { dst: usize, value: ProcConst },
    CopySlot { dst: usize, src: usize },
    GuardCarrier { slot: usize, allowed: Vec<ProcCarrierKind>, fail: DeoptPointId },
    GuardVariantTag { slot: usize, allowed: Vec<ProcVariantTag>, fail: DeoptPointId },
    GuardScalarI32 { slot: usize, fail: DeoptPointId },
    ScalarI32BinOp { dst: ProcValueId, op: ScalarBinOp, lhs: ProcValueId, rhs: ProcValueId },
    ScalarF64BinOp { dst: ProcValueId, op: ScalarBinOp, lhs: ProcValueId, rhs: ProcValueId },
    CommitScalarI32 { dst_slot: usize, value: ProcValueId },
    CommitScalarF64 { dst_slot: usize, value: ProcValueId },
    LoadUdtField { dst: ProcValueId, src_slot: usize, field: UdtFieldId },
    StoreUdtField { dst_slot: usize, field: UdtFieldId, value: ProcValueId },
    CopyUdt { dst_slot: usize, src_slot: usize, descriptor: UdtDescriptorId },
    HelperCall { helper: HelperRefId, args: Vec<HelperArgIr>, result: HelperResultIr, safepoint: SafepointId },
    SetErrorMode { mode: ProcErrorMode },
    ClearErr,
    LoadErrField { dst: usize, field: ErrField },
    MarkSource { source_id: ProcSourceMapId },
    Trace { event: ProcTraceEvent },
}
```

Initial implementation may lower most bytecode instructions to `HelperCall`.
Direct primitive ops are accepted for TB01 only when declared carrier and
overflow/coercion behavior are proved or guarded. Direct UDT field ops are
accepted for TB02 only through descriptor-verified offsets and carrier kinds.

## Terminators

```rust
pub enum ProcTerminator {
    Jump { target: BlockId },
    Branch { cond: ProcValueId, if_true: BlockId, if_false: BlockId },
    Return { status: ProcReturnStatus },
    Deopt { point: DeoptPointId },
    TrapInvariant { diagnostic: String },
}
```

There is no implicit fallthrough. `Return` must run or transfer all cleanup
state before returning.

## Safepoints

```rust
pub struct ProcSafepoint {
    pub id: SafepointId,
    pub pc: usize,
    pub source_id: Option<ProcSourceMapId>,
    pub live_slots: SlotSet,
    pub live_temps: Vec<ProcValueId>,
    pub cleanup_stack_depth: usize,
    pub byref_live: Vec<ByRefWritebackId>,
    pub error_state_live: bool,
}
```

Required safepoints:

- before and after helper calls;
- before and after COM/native calls;
- after allocation;
- at error edges;
- at deopt points;
- before returns while cleanup is live.

## Deopt Points

```rust
pub struct ProcDeoptPoint {
    pub id: DeoptPointId,
    pub reason: ProcDeoptReason,
    pub resume_pc: usize,
    pub source_id: Option<ProcSourceMapId>,
    pub live_slots: SlotSet,
    pub slot_materialization: Vec<SlotMaterialization>,
    pub error_state: ErrorStateMaterialization,
    pub cleanup_state: CleanupMaterialization,
    pub byref_state: ByRefMaterialization,
    pub interop_state: Option<InteropBoundaryState>,
}
```

Deopt reasons:

- unsupported carrier tag;
- helper requested baseline;
- debug policy;
- host policy change;
- COM/native descriptor invalidation;
- invariant guard failure.

The first implementation may return `DeoptRequested` without VM resume for
unsupported cases, but any JIT execution claim that depends on deopt resume must
prove reconstructed VM state.

## Error Edges

```rust
pub enum ErrorEdge {
    ResumeNext { next_pc: usize },
    GotoLabel { target_pc: usize },
    Unhandled,
    HelperFault,
}
```

The selected edge is resolved at runtime from `JitFrame` error state, but the IR
must name all legal destinations and the verifier must confirm those
destinations exist.

## COM/Native Descriptor References

```rust
pub enum InteropDescriptorRef {
    LateCom { id: u32, digest: String },
    EarlyCom { id: u32, typelib_identity: String, digest: String },
    NativeDeclare { descriptor_id: u32, digest: String },
    ExportedCallable { export_id: u32, digest: String },
}
```

Descriptor refs are not raw pointers. Descriptor digests are included in cache
keys and differential evidence.

## Verifier Algorithm

The first verifier should run these passes in order:

1. Shape pass: unique ids, entry block exists, all blocks have terminators.
2. Package pass: package digest is present, referenced procedure metadata exists,
   and all slot/type/descriptor references resolve to package facts.
3. CFG pass: all target blocks exist, no implicit fallthrough, reachability
   report generated.
4. Frame pass: slot ids in range and `slot_count` matches package bytecode.
5. Effect pass: reads/writes/borrows match declared slot effects.
6. Helper pass: every helper exists in ABI catalog and effect flags match.
7. Safepoint pass: all helper/allocation/interop/deopt ops have safepoints.
8. Cleanup pass: cleanup stack balanced across branches, returns, and error
   edges.
9. Error pass: error-capable ops have valid edge policy.
10. Deopt pass: all deopt points can materialize slots, error state, cleanup,
   byref, and source location.
11. Interop pass: descriptor refs exist and are included in cache key.
12. Target pass: Windows x64 target/support decision is reflected in IR.

Verifier output should be a structured report containing pass name, status,
diagnostic code, source/bytecode location, and blocking/nonblocking severity.

## Tracer Shapes

### TB01 Primitive Typed Scalar Loop

Expected shape:

```text
entry -> loop_header -> loop_body -> loop_header
                    \-> exit
```

The loop body may use direct `Long` addition and `Double` multiplication when
declared carrier metadata proves the slot types. Every helper/deopt edge must
materialize a VM-compatible snapshot from typed carriers.

### TB02 UDT Struct Field/Copy

The IR must carry a UDT descriptor id, field offsets, field carrier kinds, and a
whole-UDT copy operation. Direct field load/store is allowed only after the
verifier proves descriptor identity and field bounds. Deopt must materialize
all live fields into the VM-compatible snapshot.

### TB03 Error Routing

The declared primitive division helper must be error-capable. Legal error
destinations are resume-next and unhandled. The runtime edge selector follows
frame error state.

### TB04 BSTR Lifetime

String concat is an allocating helper with cleanup state. Then/Else branches
merge with declared destination slot ownership settled and no live temporary
BSTR.

### TB05 SAFEARRAY

Current VM-runnable seed covers array stores, index reads, `For Each`, first
package array-shape descriptors, fixed/static declared bounds, and dynamic
`ReDim` runtime SAFEARRAY bounds. Package execution also covers the selected
rank-1 fixed/static `LBound`/`UBound` descriptor path while raw bytecode keeps
the old unallocated-base-slot runtime error baseline. The IR must model array
helpers as safepoints with element live maps. TB05 cannot close until runtime
bounds-error evidence, multi-rank evidence, and lifecycle ownership evidence
are added.

### TB06-TB09 Interop

COM/native/export bullets initially lower to descriptor-backed helpers. The IR
must carry descriptor refs, safepoints, cleanup, byref, and error policy before
CLIF lowering.

## Serialization

Early implementation should provide:

- pretty text for reviews;
- JSON sidecar for harness assertions;
- CLIF text only after lowering.

IR serialization must avoid raw process addresses.

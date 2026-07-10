# JIT v2 Semantic Contract And Fact Pack v1

> [!CAUTION]
> **Superseded planning contract.** Current semantic facts live in verified OxIR/OxImage and the current JIT architecture.

Status: `planning-contract`
Date: 2026-05-26
Owning workset:
[`../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md`](../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md)
Executable semantic package:
[`EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](EXECUTABLE_SEMANTIC_PACKAGE_V1.md)
Type-system reference:
[`VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md)
Expression/call semantics reference:
[`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md)

## Purpose

Define what JIT v2 must preserve from the current VM. This is the semantic
oracle for the executable semantic package, `ProcLoweringIr`, helpers,
Cranelift lowering, and VM/JIT differential tests.

## Source Truth

Implementation truth currently lives in:

- `crates/oxvba-compiler/src/bytecode.rs`
- `crates/oxvba-vm/src/interpreter.rs`
- `crates/oxvba-vm/src/register_file.rs`
- `crates/oxvba-vm/src/semantics.rs`
- `crates/oxvba-runtime`
- `crates/oxvba-com`
- `crates/oxvba-host/src/native_ready_runner.rs`

Current facts:

- The current durable compiled artifact direction is `OxBundle`: bytecode plus
  procedure metadata and optional project/export/descriptor inventories. JIT v2
  planning treats this as the seed of a fuller executable semantic package.
- `Bytecode` contains `instructions`, `external_call_descriptors`,
  `slot_count`, and `user_slot_count`.
- VM registers store `RuntimeSlot`; the current VM snapshot API reports
  retained `Variant` values for observable slots. This is the oracle evidence
  format, not a requirement that JIT frames box every value as Variant.
- Declared VBA type metadata, including primitive locals and UDT descriptors,
  is semantic input for the executable semantic package and then for
  `ProcLoweringIr`.
- Declared type categories, object/class/interface/COM type descriptors,
  runtime value states, and Decimal-as-Variant-subtype policy are defined by
  `VBA_TYPE_SYSTEM_V1.md`.
- Expression classification, Let/Set coercion, operator behavior, property
  accessor grouping, Optional/ParamArray binding, and ByRef/ByVal call-site
  semantics are defined by `VBA_EXPRESSION_CALL_SEMANTICS_V1.md`.
- `BindingHandle` is a non-VBA binding identity path.
- Error state is currently held in VM fields:
  `on_error_resume_next`, `on_error_goto_label_target`, `last_error`,
  `last_error_pc`, `last_error_description`, and `last_error_source`.
- Windows COM boundary translation belongs to `oxvba-com`.
- Native Declare bytecode already carries `ExternalCallDescriptor` and
  `ExternalCallWriteback`.
- Current JIT rows report disabled/not implemented; they are not execution
  evidence.

## VM-Equivalent Definition

A JIT execution is VM-equivalent for a fixture only when all declared observable
surfaces match the VM for the same executable semantic package, host services,
host policy, descriptors, and initial state:

- VM-compatible slot snapshot for all observed slots, currently materialized as
  retained `Variant` evidence;
- declared carrier/layout evidence for primitive and UDT slots;
- `Err` state fields;
- procedure return/error status;
- host-visible output and diagnostics;
- COM/native boundary observations;
- ByRef writebacks;
- object identity observations;
- cleanup/lifetime counters where relevant;
- unsupported/deopt diagnostics where relevant.

CLIF text, machine-code execution success, or faster timing is not semantic
evidence without VM-equivalence evidence.

## Executable Semantic Package Contract

JIT v2 lowerers consume the same complete executable semantic package that the
VM consumes. Bytecode is the package control stream, but the package also owns
the declared type, slot, UDT, array, object, COM/native, error, source-map,
helper, host-capability, carrier-layout, expression, coercion, operator,
property, and call-site facts needed for execution.

Rules:

- no JIT-only typed reconstruction from source, syntax trees, or ad hoc side
  tables;
- no direct bytecode-to-CLIF lowering that bypasses package metadata;
- any semantic fact needed for `ProcLoweringIr` must be present in the package
  or in a versioned descriptor referenced by package digest;
- VM setup and VM evidence should consume the same package facts for every
  tracer-bullet capability;
- if a tracer bullet cannot run under the VM, classify the reason as a test
  shortcoming, missing package metadata, or real VM/runtime limitation before
  claiming JIT readiness.

## Slot State Contract

- `JitFrame` slot count must equal the package bytecode `slot_count`.
- User-visible snapshot range must match existing VM snapshot behavior.
- Declared type semantics are authoritative for all semantic values.
- `ProcLoweringIr` slots must carry explicit carrier/layout metadata for
  primitive scalars, `BStr`, `ObjectRef`, `SafeArray`, UDT structs, and declared
  `Variant` cells.
- Declared `Variant` slots and COM/native VARIANT boundary projections must use
  the COM-compatible VARIANT layout.
- Decimal is a `Variant` subtype/runtime carrier, not an ordinary declared
  primitive slot type, unless a future non-VBA extension gate explicitly opts
  into declared Decimal storage.
- Temporary primitive SSA values may exist inside verified typed regions and
  must remain materializable at safepoints.
- Every path that leaves a guarded region through helper call, branch merge,
  return, error edge, or deopt must reconcile declared slot state and
  VM-compatible snapshot materialization.
- `Empty`, `Null`, `Error`, Boolean, exact numeric carriers, `BStr`,
  `ObjectRef`, and `SafeArray` retain their VM tags.
- UDT fields retain declared field carrier semantics and whole-UDT copy behavior
  must match the VM.
- `BindingHandle` must not leak into VM-visible snapshots.

## Control-Flow Contract

- Every bytecode PC range lowered into `ProcLoweringIr` maps to one or more IR
  blocks.
- Every block ends in an explicit terminator.
- `Jump`, `JumpIfZero`, `CallProc`, `Return`, `Resume`, `ResumeNext`,
  `ResumeLabel`, and `Halt` must have explicit target/return behavior.
- Calls and returns must preserve the VM call-stack and per-activation error
  frame behavior.
- Debug/source maps must keep enough PC information to reconstruct the VM
  location for diagnostics, deopt, and future debug policy.

## Error-State Contract

JIT error routing mirrors the VM:

- on runtime/helper failure, set `last_error`, `last_error_pc`, description,
  and source according to the VM path;
- under `On Error Resume Next`, continue at `pc + 1`, preserve `last_error`,
  and clear pending `last_error_pc` as the VM does;
- under `On Error GoTo label`, jump to the label target;
- under default mode, return an unhandled error status;
- `Err.Number`, `Err.Description`, `Err.Source`, and `ClearErr` observe and
  mutate the same state as the VM;
- `Resume`, `ResumeNext`, and `ResumeLabel` raise error 20 when no pending
  error exists.

Any helper that can fail must declare whether failure routes through this
contract or reports an invariant/helper fault.

## Lifetime And Cleanup Contract

Cleanup obligations are explicit for:

- UDT fields that own `BStr`, `ObjectRef`, `SafeArray`, or declared `Variant`
  payloads;
- temporary BSTR allocations;
- string concat/intermediate results;
- SAFEARRAY descriptors and payload carriers;
- ObjectRef addref/release paths;
- COM call frames;
- EXCEPINFO and BSTR cleanup at COM boundaries;
- native marshalled buffers;
- ByRef writebacks.

Every return, branch out of a cleanup region, error edge, helper failure, host
failure, COM/native failure, and deopt exit must run or transfer cleanup exactly
once.

## BSTR Contract

- `BStr` remains the semantic string carrier.
- JIT may call helpers for assignment, concat, length, comparison, and coercion.
- JIT may not assume UTF-8/UTF-16 layout beyond documented helper contracts.
- Any direct pointer projection is boundary-specific and must use runtime
  helpers.
- Branch exits and deopt exits must preserve ownership of live strings and
  release dead temporaries.

## Primitive And UDT Contract

- Primitive declared locals and fields (`Boolean`, `Byte`, `Integer`, `Long`,
  `LongLong`, `LongPtr`, `Single`, `Double`, `Currency`, and `Date`) are first
  class JIT carriers, not hidden Variant payloads.
- Arithmetic may use direct Cranelift primitive ops only when declared carrier,
  overflow/coercion, and error behavior are proved or guarded with helper/deopt
  paths.
- UDT slots are descriptor-backed aggregates. Field offsets, field carriers,
  whole-UDT copy semantics, and cleanup obligations must come from the compiler
  and runtime descriptor truth.
- UDT snapshots are VM-compatible projections of the aggregate fields. A UDT
  field is boxed as VARIANT only when the declared field type is `Variant` or a
  boundary contract requires VARIANT projection.

## SAFEARRAY Contract

- `SafeArray` remains the semantic array carrier.
- Array literal, resize, preserve, get/set, For Each, LBound, UBound, and
  COM/native array transport must call runtime helpers until specialization is
  separately proven.
- Element lifetime is part of safepoint live maps.
- Bounds failures route through VM error state when the bytecode reaches runtime
  bounds handling.
- Current VM/package evidence covers VM-runnable store, index, For Each, first
  package array-shape descriptors, fixed/static declared bounds, and dynamic
  `ReDim` runtime SAFEARRAY bounds. Package execution also covers the selected
  rank-1 fixed/static `LBound`/`UBound` descriptor path while raw bytecode
  retains the old fixed-array base-slot runtime error baseline. Runtime
  bounds-error evidence, multi-rank evidence, lifecycle ownership evidence, and
  COM/native SAFEARRAY projection remain required before TB05 closure.

## Object Identity Contract

- VM-visible object identity is `ObjectRef`.
- Raw COM pointers, dispatch ids, native pointers, and integer handles are
  boundary/control-plane data only.
- COM identity observations must preserve `ObjectRef` identity and interface
  descriptor behavior.
- JIT may cache descriptor-derived lookup results only when cache identity is
  included in deopt/invalidator assumptions.

## ByRef Contract

- ByRef borrows must record source slot, projected boundary storage, writeback
  kind, and cancellation policy.
- Writeback commits after successful helper/native/COM return.
- Failure or deopt must either cancel or commit according to the same policy the
  VM boundary uses.
- ByRef state is part of every deopt snapshot.

## COM Contract

Late-bound and early-bound COM support is first-slice design scope:

- `CreateObject` routes through host/COM helpers with host policy checks.
- Late-bound `IDispatch::Invoke` records selector, argument names, default
  member use, HRESULT, EXCEPINFO, ArgErr, and `Err` projection.
- Early-bound calls record typelib identity, member descriptor, dispatch/vtable
  strategy, argument projection, and return projection.
- COM event and callback surfaces are not first executable tracer bullets, but
  the ABI design must not preclude reentry and cleanup.
- COM helper calls are semantic execution paths, not fallback loopholes.

## Native Declare Contract

Native Declare JIT lowering uses the bytecode `ExternalCallDescriptor`
direction:

- declared name, library, alias, ordinal flag, symbol, marshal lane, calling
  convention, selection policy, param types, ByRef flags, return type, and
  writeback descriptors are part of the JIT descriptor digest;
- the current VM/native seed proves scalar calls, BSTR/string pointer access,
  SAFEARRAY byte-buffer pointer access, Variant cell pointer exposure, and
  scalar ByRef writeback;
- general Automation `Variant` and `SAFEARRAY` declared-parameter ABI support is
  not yet present in the VM/native Declare lane and remains a real native ABI
  gap for future TB08 closure;
- marshalled temporaries and writeback buffers are cleanup-stack entries;
- host dynamic-linking capability and policy failures are distinct diagnostics.

## Unsupported Behavior Contract

Unsupported behavior must be observable as unsupported, not hidden by VM
fallback:

- unsupported target -> `JIT-TARGET-UNAVAILABLE`;
- disabled backend -> `JIT-BACKEND-DISABLED`;
- debug policy disabled -> `JIT-DEBUG-POLICY-DISABLED`;
- unsupported bytecode -> `JIT-UNSUPPORTED-BYTECODE`;
- unsupported COM/native descriptor -> descriptor-specific unsupported
  diagnostic.

Unsupported status may be paired with a VM run in a differential harness only
as reference evidence. It is not JIT execution evidence.

## Bytecode Family Map

| Family | Representative instructions | Initial JIT strategy |
|---|---|---|
| Constants/copy | `LoadConst*`, `CopySlot`, `LoadNull`, `LoadEmpty` | Direct slot writes through frame helpers or verified slot stores. |
| Arithmetic/coercion | `AddSlots`, `DivSlots`, `Cmp*`, `Bool*`, conversions | Primitive helper/direct typed fast path for declared primitive lanes; declared `Variant` helper path for Variant/dynamic lanes. |
| UDT | field load/store, whole-UDT copy, UDT locals/params | Descriptor-backed aggregate slots with verifier-checked field carrier/layout metadata. |
| Control flow | `Jump`, `JumpIfZero`, `CallProc`, `Return`, `Halt` | Native block terminators with explicit PC/source maps. |
| Error flow | `SetOnError*`, `Resume*`, `RaiseError`, `ClearErr`, `LoadErr*` | Frame error-state helpers and explicit error edges. |
| String/BSTR | `LoadConstString`, `ConcatSlots`, `IntrinsicLenDigits`, string intrinsics | Helper-first with cleanup maps. |
| SAFEARRAY | `IntrinsicArray*`, `IntrinsicForEach*`, `LBound`, `UBound` | Helper-first with live-carrier maps. |
| Host services | file/console/time/UI/process intrinsics | Helper-first, host policy checked, may be unsupported. |
| COM | `IntrinsicCreateObjectHost`, `IntrinsicDispatchInvokeHost`, COM event helpers | Descriptor-backed helpers, Windows x64 first. |
| Native Declare | `IntrinsicInvokeSymbolHost` | Shared ABI descriptor helper, writeback/cleanup explicit. |
| Object/events | WithEvents and object helper instructions | Not first executable tracer bullets; design must preserve `ObjectRef`. |

## Completion Criteria For This Contract

This contract is ready for first implementation only when:

- every first tracer bullet maps to bytecode family rows above;
- every first tracer bullet names the executable semantic package facts it
  requires and whether those facts already exist in `OxBundle`/metadata or
  still need package work;
- helper ABI catalog entries exist for every helper-first row used by tracer
  bullets;
- `ProcLoweringIr` verifier requirements cover every contract surface above;
- VM/JIT differential harness can capture the named observable surfaces.

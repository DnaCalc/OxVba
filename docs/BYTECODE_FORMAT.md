# Bytecode Format

## Status

Implementation-linked working document.

Authoritative current implementation:
- `crates/oxvba-compiler/src/bytecode.rs`
- `crates/oxvba-compiler/src/emit.rs`
- `crates/oxvba-vm/src/interpreter.rs`

This document describes the current in-memory bytecode model. It is not a frozen binary serialization spec.
It is also not a native object format or a direct PE/ELF emission contract.
For the next execution-layer evolution, bytecode is the control stream inside
the complete executable semantic package described in
[`spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md).

## Current Representation

`oxvba-compiler::Bytecode` currently carries:
- `instructions: Vec<Instruction>`
- `external_call_descriptors: Vec<ExternalCallDescriptor>`
- `slot_count: usize`
- `user_slot_count: usize`

`ExternalCallDescriptor` currently carries:
- `descriptor_id`
- `declared_name`
- `library`
- `alias`
- `ordinal_alias`
- `symbol`
- `marshal_lane`
- `calling_convention`
- `selection_policy`

## Instruction Surface

The current `Instruction` enum is the source of truth.

Implemented instruction families include:
- scalar register load/copy/arithmetic
- comparisons and boolean ops
- jumps, calls, returns, halt
- error-state operations
- host intrinsics:
  - UI
  - `DoEvents`
  - filesystem/process/time
  - COM create/invoke/subscribe/unsubscribe/callback legacy intrinsics
  - dynamic-link invoke
- collection/runtime helper intrinsics
- array/tag/type introspection intrinsics
- `WithEvents` runtime binding/owner-iteration intrinsics

Representative examples:
- `LoadConstI32`
- `AddConstI32`
- `JumpIfZero`
- `CallProc`
- `IntrinsicDispatchInvokeHost`
- `IntrinsicCreateObjectHost`
- `IntrinsicInvokeSymbolHost`
- `IntrinsicWithEventsSet`
- `ClearErr`
- `RaiseError`
- `ResumeNext`

## Execution Model Notes

Current bytecode is:
- register-slot based
- interpreted by `oxvba-vm`
- the control-stream baseline future VM/JIT/native lanes must match as part of
  the complete executable semantic package

Current slots are not a stable ABI. They are compiler/runtime implementation details.

For native-readiness planning, bytecode plus VM behavior is the current
executable model and the baseline that future direct native lanes must match.
The next target is not a parallel typed JIT path; it is a complete executable
semantic package that carries bytecode together with the declared type,
descriptor, error, source-map, host, and interop metadata required by both VM
and JIT. A direct native compiler may lower from that package through a future
real procedure-lowering IR, but the current bytecode document does not claim a
direct AOT/native-image pipeline.

## Stability Boundary

Current stable expectations:
- enum variants and fields are authoritative for current source
- emitted bytecode is deterministic for identical source/config inputs
- `user_slot_count` is the host-visible snapshot boundary

Not yet stable:
- binary on-disk encoding
- opcode numbering
- compact operand encoding
- mmap/zero-copy persistence contract

## Near-Term Direction

Planned evolution remains:
- clearer bytecode authority boundary versus future serialized form
- tighter documentation of instruction families and invariants
- growth of `OxBundle` or its successor into the complete executable semantic
  package shared by VM, JIT, wrappers, and future native lanes
- eventual stable serialization only after the instruction surface and calling conventions settle
- shared runner/correctness schema before bytecode behavior is compared with
  future native artifacts

Until then, code is authoritative and this document should stay synchronized to that code rather than pretending a frozen format already exists.

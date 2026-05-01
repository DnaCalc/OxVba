# Bytecode Format

## Status

Implementation-linked working document.

Authoritative current implementation:
- `crates/oxvba-compiler/src/bytecode.rs`
- `crates/oxvba-compiler/src/emit.rs`
- `crates/oxvba-vm/src/interpreter.rs`

This document describes the current in-memory bytecode model. It is not a frozen binary serialization spec.
It is also not a native object format or a direct PE/ELF emission contract.

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
- optionally mirrored through `oxvba-jit` for supported subsets

Current slots are not a stable ABI. They are compiler/runtime implementation details.

For native-readiness planning, bytecode is the authoritative executable model
and the baseline that future direct native lanes must match. A direct native
compiler may lower from bytecode or from a future real native-facing procedure
IR, but the current bytecode document does not claim a direct AOT/native-image
pipeline.

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
- eventual stable serialization only after the instruction surface and calling conventions settle
- shared runner/correctness schema before bytecode behavior is compared with
  future native artifacts

Until then, code is authoritative and this document should stay synchronized to that code rather than pretending a frozen format already exists.

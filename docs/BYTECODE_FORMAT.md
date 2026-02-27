# BYTECODE_FORMAT.md

## Status
Draft v0 (scaffold).

## Representation
`oxvba-compiler::Bytecode` currently stores:
- `instructions: Vec<Instruction>`
- `slot_count: usize`

Current `Instruction` variants:
- `LoadConstI32 { slot, value }`
- `AddConstI32 { slot, value }`
- `SubConstI32 { slot, value }`
- `CopySlot { dst, src }`
- `CmpEqSlots { dst, lhs, rhs }`
- `CmpLeSlots { dst, lhs, rhs }`
- `JumpIfZero { cond_slot, target_pc }`
- `Jump { target_pc }`
- `IncSlot { slot }`
- `Halt`

`Bytecode` now tracks:
- `slot_count`: total runtime slots (declared + compiler temporaries).
- `user_slot_count`: declared/user-visible slots for snapshots and conformance output.

This is an MVP representation to support early vertical-slice execution and tests.

## Planned evolution
- Explicit opcode enum and operand encoding.
- Register-window aware calling convention.
- rkyv-serializable stable binary layout for mmap/zero-copy loading.

## Feature flags
- `mach_zero_copy_bytecode` (crate: `oxvba-compiler`): enables zero-copy bytecode loading optimization path when implemented.

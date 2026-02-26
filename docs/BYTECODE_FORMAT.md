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
- `Halt`

This is an MVP representation to support early vertical-slice execution and tests.

## Planned evolution
- Explicit opcode enum and operand encoding.
- Register-window aware calling convention.
- rkyv-serializable stable binary layout for mmap/zero-copy loading.

## Feature flags
- `mach_zero_copy_bytecode` (crate: `oxvba-compiler`): enables zero-copy bytecode loading optimization path when implemented.

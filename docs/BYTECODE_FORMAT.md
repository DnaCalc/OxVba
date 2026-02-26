# BYTECODE_FORMAT.md

## Status
Draft v0 (scaffold).

## Representation
`oxvba-compiler::Bytecode` currently stores:
- `instructions: Vec<String>`

This is a temporary representation to support early vertical-slice execution and tests.

## Planned evolution
- Explicit opcode enum and operand encoding.
- Register-window aware calling convention.
- rkyv-serializable stable binary layout for mmap/zero-copy loading.

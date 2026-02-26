# ARCHITECTURE.md

## Current implementation snapshot

Workspace crates:
- `oxvba-syntax`: tokenization + syntax tree scaffold.
- `oxvba-runtime`: 16-byte Variant container, basic coercion/arithmetic.
- `oxvba-ir`: HIR/MIR/CFG structures and lowering scaffolds.
- `oxvba-compiler`: resolve/typecheck/lower/emit scaffold with rkyv bytecode object.
- `oxvba-vm`: register-file VM scaffold for instruction execution.
- `oxvba-jit`: JIT interface placeholder.
- `oxvba-com`: COM abstraction scaffolding.
- `oxvba-host`: engine orchestration and root-object registration.
- `oxvba-cli`: `run` entry point.

## Intended evolution
The implementation follows `MACH1000_PLAN.md` sequencing. Early milestones prioritize correctness and compatibility evidence, with performance features promoted only after parity gates are green.

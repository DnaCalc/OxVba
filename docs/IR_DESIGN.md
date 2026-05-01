# IR Design

## Status

Implementation-linked current-truth document.

The active OxVba compiler path does not currently use a real HIR/MIR/CFG
pipeline. Source and project analysis in `oxvba-compiler` emits
`oxvba-compiler::Bytecode` directly, and executable behavior is defined by the
VM/JIT/runtime lanes documented in [`BYTECODE_FORMAT.md`](BYTECODE_FORMAT.md)
and [`ARCHITECTURE.md`](ARCHITECTURE.md).

## Removed Historical Scaffold

The previous `oxvba-ir` crate names:
- `VbaHir`
- `VbaMir`
- `CfgIr`

These were historical scaffolds. They preserved simple instruction sequences and
had scaffold-level tests, but did not model semantic control flow, typed values,
slot effects, helper calls, diagnostics, or source/bytecode mapping. They were
removed from active code during the native-ready rebase and must not be used as
evidence that OxVba has an active multi-level optimization pipeline.

Current error handling behavior is represented in bytecode/runtime instructions
such as `SetOnErrorResumeNext`, `SetOnErrorGoto0`, `SetOnErrorGotoLabel`,
`Resume`, `ResumeNext`, and `ResumeLabel`, not by a semantic CFG layer.

## Native-Ready Decision

The native-ready rebase workset tracks the fake IR removal:
[`WORKSET_2026-04-30_RUNTIMEVALUE_IR_STUB_CLEANOUT.md`](worksets/WORKSET_2026-04-30_RUNTIMEVALUE_IR_STUB_CLEANOUT.md).

There are two acceptable future paths:
- replace it with a real native-facing procedure IR.

A future `NativeProcIr` should only be introduced with an actual contract:
- basic blocks and explicit terminators;
- typed value/slot effects;
- structured runtime/helper calls;
- error-state and exceptional-control semantics;
- source and bytecode mapping;
- diagnostics suitable for compiler and runner reporting;
- tests that prove it preserves VM-visible behavior.

Until then, bytecode remains the implementation truth and optimization/no-op IR
language should not be described as active architecture.

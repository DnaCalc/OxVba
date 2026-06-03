# IR Design

## Status

Implementation-linked current-truth document.

This document concerns the **mid-level / procedure-lowering optimization IR**
(the `VbaHir`/`VbaMir`/`CfgIr` family). There is no active optimization IR of
that kind: source and project analysis in `oxvba-compiler` emits
`oxvba-compiler::Bytecode` plus runtime/project metadata, and executable
behavior is defined by the VM/JIT/runtime lanes documented in
[`BYTECODE_FORMAT.md`](BYTECODE_FORMAT.md),
[`ARCHITECTURE.md`](ARCHITECTURE.md), and the executable semantic package draft
[`spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md).

Distinct from this document: a separate **front-end bound HIR** is being built in
`oxvba-compiler/src/frontend_hir*.rs` as the replacement for the legacy
string-rewriting front-end (`source → oxvba-syntax CST → binder → bound HIR →
lowering → bytecode`), tracked under `bd-aprs` and described in
[`ARCHITECTURE.md`](ARCHITECTURE.md) ("End-State Destination") and
[`FRONTEND_STATE_REPORT_2026-06-03.md`](FRONTEND_STATE_REPORT_2026-06-03.md). That
front-end HIR is a source-level resolved tree, not the removed `oxvba-ir`
optimization IR, and it does not change the conclusion here that no mid-level
optimization IR is active.

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

The accepted future layering is:
- grow `OxBundle` or its successor into a complete executable semantic package
  shared by the VM, JIT, wrappers, and future native lanes;
- lower package procedures into a real procedure-lowering IR only after package
  facts are complete enough for the scoped tracer.

A future `ProcLoweringIr` should only be introduced with an actual lowering
contract:
- basic blocks and explicit terminators;
- typed value/slot effects;
- structured runtime/helper calls;
- error-state and exceptional-control semantics;
- source and bytecode mapping;
- diagnostics suitable for compiler and runner reporting;
- tests that prove it preserves VM-visible behavior.

Until then, bytecode plus current VM behavior remains the implementation truth
and optimization/no-op IR language should not be described as active
architecture.

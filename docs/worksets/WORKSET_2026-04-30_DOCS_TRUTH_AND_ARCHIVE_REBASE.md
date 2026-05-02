# Docs Truth And Archive Rebase Workset

Status: `complete` (reconfirmed in recovery audit)
Date: 2026-04-30; recovery update 2026-05-02
Parent: `WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md`

## Purpose

Separate historical planning from current implementation truth. The immediate
problem is that early MACH-1000 architecture material still reads as current
authority in places where the repo now follows a different, more concrete path:
bytecode -> retained-`Variant` VM/JIT -> wrapper packaging, with direct native
compilation still future work.

## Scope

In scope:

- Add supersession or historical-context headers to old plans and reviews.
- Update `docs/README.md` to identify current authoritative docs.
- Rewrite `docs/ARCHITECTURE.md`, `docs/IR_DESIGN.md`, and
  `docs/BYTECODE_FORMAT.md` around checked-in implementation truth.
- Update `docs/archive/README.md` so archived and historical plans no longer
  look like active execution guidance.
- Add links to native-ready rebase worksets and specs.

Out of scope:

- Removing code.
- Rewriting all old profile ladder docs.
- Implementing direct native compiler/linker work.

## Execution Epics

1. **Authority Map Refresh**
   - Close condition: `docs/README.md` names current-truth docs and demotes
     historical synthesis plans.
2. **Historical Header Pass**
   - Close condition: `MACH1000_PLAN.md` and archive index identify historical
     status and current successors.
3. **Current Truth Rewrite**
   - Close condition: architecture/IR/bytecode docs match current code paths and
     planned cleanup work.
4. **Doc Drift Search Gate**
   - Close condition: active docs no longer claim active HIR/MIR/CFG or direct
     native AOT beyond wrapper/JIT truth.

## First Beads

- `docs-truth-001`: refresh `docs/README.md` authority table.
- `docs-truth-002`: add historical-context header to `MACH1000_PLAN.md`.
- `docs-truth-003`: rewrite `docs/ARCHITECTURE.md`.
- `docs-truth-004`: rewrite `docs/IR_DESIGN.md` and refresh
  `docs/BYTECODE_FORMAT.md`.
- `docs-truth-005`: run search gate and record residuals.

## Terminal Gate

This workset is complete when active docs clearly say:

- `MACH1000_PLAN.md` is historical synthesis/vision, not the live execution
  plan.
- the former `oxvba-ir` scaffold was removed from active code and any future
  IR must be a real native-readiness IR.
- Direct native PE/ELF is future work, while wrapper artifacts and Cranelift JIT
  subset support are current truth.
- `RuntimeValue` is no longer present in active Rust source; retained `Variant`
  and SAFEARRAY `Variant` carriers are the active value substrate.

## Completion State

Completed in the native-ready modernization cleanup:

- top-level guidance now points to `docs/ARCHITECTURE.md`, active worksets,
  status files, and evidence artifacts for current execution truth;
- `MACH1000_PLAN.md` is labeled historical synthesis and vision context;
- `docs/ARCHITECTURE.md`, `docs/IR_DESIGN.md`, and `docs/BYTECODE_FORMAT.md`
  describe bytecode, retained `Variant`, VM/JIT, wrapper, and future native
  boundaries without claiming an active HIR/MIR/CFG pipeline;
- `docs/README.md`, `docs/archive/README.md`, and `docs/spec/README.md` point
  at the native-ready rebase worksets and specs;
- the 2026-05-02 recovery audit supersedes older RuntimeValue residual language:
  active Rust source is clean, while child worksets 3-5 remain under recovery
  re-audit for executable value/corpus/runner evidence.

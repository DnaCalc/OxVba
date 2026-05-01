# Native-Ready Rebase Master Workset

Status: `in-progress`
Date: 2026-04-30
Scope owner: OxVBA architecture/runtime/native-readiness

## Purpose

Coordinate the next multi-workset rebase that prepares OxVBA for direct native
compilation without pretending the current runtime is already native-ready.

This is an umbrella workset. Its terminal gate is not direct native PE/ELF
output. Its terminal gate is a cleaned, evidence-backed baseline where docs,
runtime value carriers, type semantics, correctness stress coverage, and runner
telemetry are coherent enough for the later native compiler/linker workset.

## Current Truth Baseline

- `oxvba-compiler` emits `Bytecode` directly.
- `oxvba-vm` is the reference execution engine over retained `Variant` slots.
- `oxvba-jit` is a Cranelift-backed subset accelerator with explicit VM
  fallback in current product lanes.
- `oxvba-build` emits wrapper artifacts over canonical `.oxb` bundles.
- The former `oxvba-ir` HIR/MIR/CFG scaffold and compiler `lower_to_hir` no-op
  lowering have been removed from active code.
- `RuntimeValue` remains in active code today as a compatibility/projection
  carrier and must be removed or explicitly quarantined before native-facing
  APIs are considered clean.
- UDT support is a bounded flattened-field semantic subset, not native struct
  layout or unconstrained UDT-byref ABI parity.

## Current Execution State

The umbrella is active. The docs-truth and IR-scaffold-removal slices have
landed. Phase 2 has an active `RuntimeValue` family inventory in
[`../evidence/native_ready/RUNTIMEVALUE_ACTIVE_USE_INVENTORY_2026-05-01.md`](../evidence/native_ready/RUNTIMEVALUE_ACTIVE_USE_INVENTORY_2026-05-01.md). The VM/host
snapshot/invoke/observation surface migration has landed with evidence in
[`../evidence/native_ready/RUNTIMEVALUE_VM_HOST_SURFACE_MIGRATION_2026-05-01.md`](../evidence/native_ready/RUNTIMEVALUE_VM_HOST_SURFACE_MIGRATION_2026-05-01.md).
The JIT, HAL/COM/runtime, and remaining presentation delivery migrations remain
open. The value/numeric/UDT cleanup, stress corpus, and runner/performance
scaffold remain open follow-on work.

## Child Worksets

| Order | Workset | Purpose | Terminal gate |
|---|---|---|---|
| 1 | `WORKSET_2026-04-30_DOCS_TRUTH_AND_ARCHIVE_REBASE.md` | Demote historical plans and make active docs implementation-accurate. | No authoritative doc claims active HIR/MIR/CFG or direct native AOT beyond current wrapper/JIT truth. |
| 2 | `WORKSET_2026-04-30_RUNTIMEVALUE_IR_STUB_CLEANOUT.md` | Remove `RuntimeValue` and fake IR scaffold from active code/API surfaces. | Fake IR code is removed; `RuntimeValue` migration remains inventoried until search gates are clean outside archived docs or approved blocker notes. |
| 3 | `WORKSET_2026-04-30_VALUE_SUBSTRATE_NUMERIC_UDT_CLEANUP.md` | Make value/type semantics native-ready. | Native-facing code can rely on one retained-`Variant` value substrate and explicit UDT boundary model. |
| 4 | `WORKSET_2026-04-30_CORRECTNESS_CORPUS_AND_ORACLE_STRESS.md` | Build stress tests likely to expose hidden numeric/type bugs. | Stress corpus is runnable across VM/JIT/reference lanes with oracle/spec classification. |
| 5 | `WORKSET_2026-04-30_REFERENCE_RUNNERS_AND_PERF_SCAFFOLD.md` | Standardize correctness/perf evidence for VM/JIT/wrappers/future native. | Shared runner schema emits comparable result and performance artifacts. |

## Required Specs

- `docs/spec/NATIVE_READY_VALUE_SUBSTRATE_V1.md`
- `docs/spec/NATIVE_READY_RUNNER_AND_BENCHMARK_SCHEMA_V1.md`

## Execution Policy

1. Do not begin direct native compiler/linker implementation under this
   umbrella.
2. Prefer deleting or quarantining stale surfaces over preserving compatibility
   names that obscure current truth.
3. Preserve historical documents for provenance, but remove their authority over
   current execution planning.
4. Any remaining `RuntimeValue`, fake IR, or UDT/native-layout residual must be
   named as a blocker or explicit compatibility exception.
5. Every child workset must update both documentation and verification evidence
   before being marked complete.

## Umbrella Terminal Gate

The umbrella is complete when all child worksets are complete and the following
search/verification gates are green:

- `rg "\bRuntimeValue\b" crates docs` returns only archived docs or approved
  residual notes.
- `rg "CfgIr|VbaHir|VbaMir" crates docs` returns only archived docs or approved
  residual notes.
- `docs/ARCHITECTURE.md`, `docs/IR_DESIGN.md`, `docs/BYTECODE_FORMAT.md`, and
  `docs/README.md` describe current implementation truth.
- VM/JIT/wrapper reference runners use the shared result schema.
- Numeric/UDT stress corpus runs in the expected backend matrix.
- Native compiler/linker work has a clean prerequisite checklist.

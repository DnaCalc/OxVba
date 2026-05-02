# Native-Ready Rebase Master Workset

Status: `complete` (recovered 2026-05-02; wrapper library follow-up non-blocking)
Date: 2026-04-30; recovery update 2026-05-02
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
- `RuntimeValue` has been removed from active Rust source by `bd-0w46`
  (`8d5fdfc0`); retained `Variant` and SAFEARRAY `Variant` carriers are now the
  active runtime/API value surface.
- UDT support is a bounded flattened-field semantic subset, not native struct
  layout or unconstrained UDT-byref ABI parity.

## Current Execution State

The umbrella is active again for recovery tracking. Child workset 1 remains
materially complete for docs truth/direct-native non-claims. Child workset 2 is
now recovered complete after `bd-0w46`; active Rust source has zero
`RuntimeValue|runtime_value` matches and fake IR crate APIs remain absent.

The previous value/numeric/UDT and correctness corpus claims have now been
recovered with nonzero executable tests. The runner/performance lane has an
active VM/JIT schema producer and CLI path plus wrapper EXE row production;
wrapper library row production remains an explicit non-blocking follow-up
(`bd-9xmu.5.9`).

Recovery audit 2026-05-02 supersedes the earlier umbrella completion claim for
current planning truth:
[`NATIVE_READY_RECOVERY_AUDIT_2026-05-02.md`](../evidence/native_ready/NATIVE_READY_RECOVERY_AUDIT_2026-05-02.md).
The earlier terminal audit remains historical evidence. Child worksets 3 and 4
were recovered on 2026-05-02; child workset 5 is recovered under a reduced
claim with VM/JIT and wrapper EXE producer evidence, while wrapper library
artifact producer work is left as non-blocking successor bead `bd-9xmu.5.9`.

## Child Worksets

| Order | Workset | Purpose | Terminal gate |
|---|---|---|---|
| 1 | `WORKSET_2026-04-30_DOCS_TRUTH_AND_ARCHIVE_REBASE.md` | Demote historical plans and make active docs implementation-accurate. | No authoritative doc claims active HIR/MIR/CFG or direct native AOT beyond current wrapper/JIT truth. |
| 2 | `WORKSET_2026-04-30_RUNTIMEVALUE_IR_STUB_CLEANOUT.md` | Remove `RuntimeValue` and fake IR scaffold from active code/API surfaces. | **Recovered complete:** fake IR code is removed and active Rust source has zero `RuntimeValue|runtime_value` matches. |
| 3 | `WORKSET_2026-04-30_VALUE_SUBSTRATE_NUMERIC_UDT_CLEANUP.md` | Make value/type semantics native-ready. | **Recovered complete:** post-`RuntimeValue` Variant-native numeric/coercion/UDT gates have executing tests. |
| 4 | `WORKSET_2026-04-30_CORRECTNESS_CORPUS_AND_ORACLE_STRESS.md` | Build stress tests likely to expose hidden numeric/type bugs. | **Recovered complete:** `NR-NUM-001/002`, `NR-COERCE-001`, and `NR-UDT-001` run nonzero tests. |
| 5 | `WORKSET_2026-04-30_REFERENCE_RUNNERS_AND_PERF_SCAFFOLD.md` | Standardize correctness/perf evidence for VM/JIT/wrappers/future native. | **Recovered complete:** VM/JIT and wrapper EXE schema producers recovered; wrapper library producer deferred to non-blocking `bd-9xmu.5.9`. |

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
4. Any reintroduced `RuntimeValue`, fake IR, or UDT/native-layout residual must
   be named as a blocker or explicit compatibility exception.
5. Every child workset must update both documentation and verification evidence
   before being marked complete.

## Umbrella Terminal Gate

The umbrella returns to complete only when all child worksets are complete under
the recovery audit and the following search/verification gates are green:

- `rg -n "RuntimeValue|runtime_value" crates --glob '*.rs'` returns zero active
  Rust source matches.
- `rg "CfgIr|VbaHir|VbaMir" crates docs` returns zero active crate matches and
  only current explanatory or residual-note docs.
- `docs/ARCHITECTURE.md`, `docs/IR_DESIGN.md`, `docs/BYTECODE_FORMAT.md`, and
  `docs/README.md` describe current implementation truth.
- VM/JIT and wrapper EXE reference runners use the shared result schema;
  wrapper library rows remain sample-only under non-blocking successor
  `bd-9xmu.5.9`.
- Numeric/UDT stress corpus runs in the expected backend matrix.
- Native compiler/linker work has a clean prerequisite checklist.

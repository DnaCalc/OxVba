# Reference Runners And Performance Scaffold Workset

Status: `complete`
Date: 2026-04-30
Parent: `WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md`

## Purpose

Standardize correctness and performance evidence for VM, JIT, wrapper EXE,
wrapper library, and future direct native artifacts.

## Scope

In scope:

- Shared runner result schema.
- VM/JIT/wrapper runner normalization.
- JIT fallback reporting.
- Artifact-size and elapsed-time capture.
- Benchmark workload catalog aligned with correctness stress corpus.

Out of scope:

- Direct native compiler/linker implementation.
- Product claims of speed superiority.
- Cross-machine absolute performance claims.

## Evidence

- Runner schema lock:
  [`../evidence/native_ready/RUNNER_SCHEMA_LOCK_2026-05-01.md`](../evidence/native_ready/RUNNER_SCHEMA_LOCK_2026-05-01.md)
- VM/JIT runner row normalization:
  [`../evidence/native_ready/VM_JIT_RUNNER_ROWS_2026-05-01.md`](../evidence/native_ready/VM_JIT_RUNNER_ROWS_2026-05-01.md)
- Wrapper runner smoke rows:
  [`../evidence/native_ready/WRAPPER_RUNNER_ROWS_2026-05-01.md`](../evidence/native_ready/WRAPPER_RUNNER_ROWS_2026-05-01.md)
- Runner artifact size and timing fields:
  [`../evidence/native_ready/RUNNER_SIZE_TIMING_FIELDS_2026-05-01.md`](../evidence/native_ready/RUNNER_SIZE_TIMING_FIELDS_2026-05-01.md)
- Benchmark corpus under shared schema:
  [`../evidence/native_ready/BENCHMARK_CORPUS_SHARED_SCHEMA_2026-05-01.md`](../evidence/native_ready/BENCHMARK_CORPUS_SHARED_SCHEMA_2026-05-01.md)

## Execution Epics

1. **Schema Lock**
   - Close condition: `NATIVE_READY_RUNNER_AND_BENCHMARK_SCHEMA_V1.md` is the
     shared result contract.
2. **VM/JIT Runner Normalization**
   - Close condition: VM and JIT produce comparable rows, including fallback
     status.
3. **Wrapper Runner Normalization**
   - Close condition: wrapper EXE/library rows can be compared with VM/JIT
     results and artifact size.
4. **Benchmark Corpus Integration**
   - Close condition: correctness stress rows can be reused as perf workloads
     where meaningful.
5. **Native Placeholder Contract**
   - Close condition: future PE/ELF native rows have a reserved backend shape
     but no false implementation claim.

## First Beads

Rolled out on 2026-05-01 under bead epic `bd-9xmu.5`:

- `bd-9xmu.5.1` / `runner-000`: roll out this executable bead path.
- `bd-9xmu.5.2` / `runner-001`: lock shared runner schema writer path. Done
  2026-05-01; the schema is locked, canonical header sample exists, and writer
  validation rules are recorded.
- `bd-9xmu.5.3` / `runner-002`: normalize VM/JIT rows with fallback status.
  Done 2026-05-01; sample rows cover VM, JIT without fallback, and JIT with VM
  fallback under the shared schema.
- `bd-9xmu.5.4` / `runner-003`: add wrapper EXE/library smoke rows. Done
  2026-05-01; sample rows cover `wrapper-exe` and `wrapper-library` artifact
  identities under the shared schema.
- `bd-9xmu.5.5` / `runner-004`: add artifact size and timing fields. Done
  2026-05-01; sample benchmark rows populate byte sizes, iterations, warmups,
  elapsed milliseconds, result digests, and trend-only claim boundaries.
- `bd-9xmu.5.6` / `runner-005`: publish first benchmark corpus under shared
  schema. Done 2026-05-01; benchmark seed rows reuse `NR-NUM-002`,
  `NR-COERCE-001`, and `NR-UDT-001` with backend, status, timing/size,
  fallback classification, digest, and claim-boundary fields.

## Terminal Gate

This workset is complete when direct native work can use existing reference
runners for both correctness and performance comparison without inventing a new
evidence format.


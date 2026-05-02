# Reference Runners And Performance Scaffold Workset

Status: `in-progress` (VM/JIT producer recovered; wrapper producer follow-up open)
Date: 2026-04-30; recovery update 2026-05-02
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
- Active VM/JIT producer recovery:
  [`../evidence/native_ready/RUNNER_PRODUCER_RECOVERY_2026-05-02.md`](../evidence/native_ready/RUNNER_PRODUCER_RECOVERY_2026-05-02.md)

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
- `bd-9xmu.5.2` / `runner-001`: lock shared runner schema writer path.
  Recovered for VM/JIT in `bd-9xmu.5.7` via active Rust producer and CLI path.
- `bd-9xmu.5.3` / `runner-002`: normalize VM/JIT rows with fallback status.
  Recovered 2026-05-02; `native_ready_runner_rows` emits VM/JIT rows and marks
  project JIT fallback explicitly.
- `bd-9xmu.5.4` / `runner-003`: add wrapper EXE/library smoke rows. Still
  sample-only for real wrapper artifact execution; follow-up delivery bead
  `bd-9xmu.5.8` owns the real wrapper producer.
- `bd-9xmu.5.5` / `runner-004`: add artifact size and timing fields. Recovered
  for VM/JIT rows; wrapper artifact size/timing remains part of `bd-9xmu.5.8`.
- `bd-9xmu.5.6` / `runner-005`: publish first benchmark corpus under shared
  schema. Stress workload references are recovered by `bd-9xmu.4.7`; active
  VM/JIT producer exists for new rows.
- `bd-9xmu.5.7` / recovery: decide and implement the runner evidence producer
  gate after discovering current evidence is schema/sample-only. Recovered
  2026-05-02 for VM/JIT producer; wrapper producer explicitly deferred to
  `bd-9xmu.5.8`.
- `bd-9xmu.5.8` / follow-up: implement wrapper artifact schema producer.
  Open; required before claiming real wrapper EXE/library row production.

## Terminal Gate

This workset returns to complete when direct native work can use existing
reference runners for both correctness and performance comparison without
inventing a new evidence format.

Recovery decision: `bd-9xmu.5.7` restored the active VM/JIT schema producer and
CLI path, but explicitly reduced the wrapper claim to sample/schema scaffolding
until `bd-9xmu.5.8` implements a real wrapper EXE/library artifact runner.
Current evidence:
[`../evidence/native_ready/RUNNER_PRODUCER_RECOVERY_2026-05-02.md`](../evidence/native_ready/RUNNER_PRODUCER_RECOVERY_2026-05-02.md).


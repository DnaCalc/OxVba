# Reference Runners And Performance Scaffold Workset

Status: `complete` (recovered 2026-05-02; wrapper library producer delivered 2026-05-07)
Date: 2026-04-30; recovery update 2026-05-02; wrapper-library update 2026-05-07
Parent: `WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md`

## Purpose

Standardize correctness and performance evidence for VM, JIT, wrapper EXE,
wrapper library, and future direct native artifacts.

## Scope

In scope:

- Shared runner result schema.
- VM/wrapper runner normalization plus disabled JIT placeholder reporting.
- JIT v1 purge status reporting.
- Artifact-size and elapsed-time capture.
- Benchmark workload catalog aligned with correctness stress corpus.

Out of scope:

- Direct native compiler/linker implementation.
- Product claims of speed superiority.
- Cross-machine absolute performance claims.

## Evidence

- Runner schema lock:
  [`../evidence/native_ready/RUNNER_SCHEMA_LOCK_2026-05-01.md`](../evidence/native_ready/RUNNER_SCHEMA_LOCK_2026-05-01.md)
- VM runner row normalization with disabled JIT placeholder:
  [`../evidence/native_ready/VM_JIT_RUNNER_ROWS_2026-05-01.md`](../evidence/native_ready/VM_JIT_RUNNER_ROWS_2026-05-01.md)
- Wrapper runner smoke rows:
  [`../evidence/native_ready/WRAPPER_RUNNER_ROWS_2026-05-01.md`](../evidence/native_ready/WRAPPER_RUNNER_ROWS_2026-05-01.md)
- Runner artifact size and timing fields:
  [`../evidence/native_ready/RUNNER_SIZE_TIMING_FIELDS_2026-05-01.md`](../evidence/native_ready/RUNNER_SIZE_TIMING_FIELDS_2026-05-01.md)
- Benchmark corpus under shared schema:
  [`../evidence/native_ready/BENCHMARK_CORPUS_SHARED_SCHEMA_2026-05-01.md`](../evidence/native_ready/BENCHMARK_CORPUS_SHARED_SCHEMA_2026-05-01.md)
- Active VM/wrapper producer recovery:
  [`../evidence/native_ready/RUNNER_PRODUCER_RECOVERY_2026-05-02.md`](../evidence/native_ready/RUNNER_PRODUCER_RECOVERY_2026-05-02.md)
- Wrapper library producer follow-up:
  [`../evidence/native_ready/WRAPPER_LIBRARY_RUNNER_PRODUCER_2026-05-07.md`](../evidence/native_ready/WRAPPER_LIBRARY_RUNNER_PRODUCER_2026-05-07.md)

## Execution Epics

1. **Schema Lock**
   - Close condition: `NATIVE_READY_RUNNER_AND_BENCHMARK_SCHEMA_V1.md` is the
     shared result contract.
2. **VM Runner Normalization**
   - Close condition: VM produces executable rows and JIT rows, if emitted, are
     explicit disabled placeholders rather than fallback execution claims.
3. **Wrapper Runner Normalization**
   - Close condition: wrapper EXE/library rows can be compared with VM
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
  Recovered for VM in `bd-9xmu.5.7` via active Rust producer and CLI path.
- `bd-9xmu.5.3` / `runner-002`: normalize VM rows and JIT placeholder status.
  Updated 2026-05-25 after the JIT v1 purge; `native_ready_runner_rows` emits
  executable VM rows and marks JIT as `JIT-NOT-IMPLEMENTED`.
- `bd-9xmu.5.4` / `runner-003`: add wrapper EXE/library smoke rows. Recovered
  for wrapper EXE real artifact execution by `bd-9xmu.5.8`; wrapper library
  real artifact/exported-call row production was delivered by `bd-9xmu.5.9`.
- `bd-9xmu.5.5` / `runner-004`: add artifact size and timing fields. Recovered
  for VM, disabled JIT placeholder, wrapper EXE, and wrapper library rows.
- `bd-9xmu.5.6` / `runner-005`: publish first benchmark corpus under shared
  schema. Stress workload references are recovered by `bd-9xmu.4.7`; active
  VM producer exists for new rows.
- `bd-9xmu.5.7` / recovery: decide and implement the runner evidence producer
  gate after discovering current evidence is schema/sample-only. Recovered
  2026-05-02 for VM producer; wrapper EXE producer split to
  `bd-9xmu.5.8`.
- `bd-9xmu.5.8` / follow-up: implement wrapper EXE artifact schema producer.
  Recovered 2026-05-02; `oxvba native-ready-runner --wrapper-exe` builds and
  executes a wrapper EXE and emits a `wrapper-exe` row.
- `bd-9xmu.5.9` / follow-up: implement wrapper library artifact schema
  producer. Delivered 2026-05-07; `oxvba native-ready-runner --wrapper-library`
  builds a wrapper library, invokes a supported exported `NativeExport`, and
  emits a `wrapper-library` row.

## Terminal Gate

This workset is complete under the recovery decision: direct-native follow-on
work can use the shared schema plus executable VM, disabled JIT placeholder,
wrapper EXE, and wrapper library reference rows without inventing a new
evidence format.

Current evidence:
- [`../evidence/native_ready/RUNNER_PRODUCER_RECOVERY_2026-05-02.md`](../evidence/native_ready/RUNNER_PRODUCER_RECOVERY_2026-05-02.md)
- [`../evidence/native_ready/WRAPPER_LIBRARY_RUNNER_PRODUCER_2026-05-07.md`](../evidence/native_ready/WRAPPER_LIBRARY_RUNNER_PRODUCER_2026-05-07.md)

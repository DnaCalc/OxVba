# Reference Runners And Performance Scaffold Workset

Status: `in-progress` (recovery audit reopened)
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
  Reopened for recovery audit; current evidence proves schema/sample CSV shape,
  but no active Rust schema writer/producer was found.
- `bd-9xmu.5.3` / `runner-002`: normalize VM/JIT rows with fallback status.
  Reopened for recovery audit; current evidence is sample rows only unless an
  executable producer is implemented or the terminal gate is explicitly reduced.
- `bd-9xmu.5.4` / `runner-003`: add wrapper EXE/library smoke rows. Reopened
  for recovery audit; current evidence is sample rows only.
- `bd-9xmu.5.5` / `runner-004`: add artifact size and timing fields. Reopened
  for recovery audit; current evidence is sample rows only.
- `bd-9xmu.5.6` / `runner-005`: publish first benchmark corpus under shared
  schema. Reopened for recovery audit; benchmark seed rows refer to stress tests
  that currently filter to zero tests.
- `bd-9xmu.5.7` / recovery: decide and implement the runner evidence producer
  gate after discovering current evidence is schema/sample-only. Open
  2026-05-02; this is now the active terminal recovery bead for phase 5.

## Terminal Gate

This workset returns to complete when direct native work can use existing
reference runners for both correctness and performance comparison without
inventing a new evidence format. The recovery decision must choose one of two
truthful gates:

1. implement an executable producer that emits the shared schema for VM/JIT and
   wrapper lanes; or
2. explicitly reduce the workset claim to schema/sample scaffolding and create a
   follow-up delivery bead for real producer implementation.

Recovery blocker: the 2026-05-02 audit found docs/spec/sample CSV evidence but
no active Rust Native-Ready runner schema producer. Evidence:
[`../evidence/native_ready/NATIVE_READY_RECOVERY_AUDIT_2026-05-02.md`](../evidence/native_ready/NATIVE_READY_RECOVERY_AUDIT_2026-05-02.md).


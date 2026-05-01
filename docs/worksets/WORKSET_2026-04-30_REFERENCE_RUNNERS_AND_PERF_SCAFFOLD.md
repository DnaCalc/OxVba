# Reference Runners And Performance Scaffold Workset

Status: `in-progress`
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
- `bd-9xmu.5.3` / `runner-002`: normalize VM/JIT rows with fallback status.
- `bd-9xmu.5.4` / `runner-003`: add wrapper EXE/library smoke rows.
- `bd-9xmu.5.5` / `runner-004`: add artifact size and timing fields.
- `bd-9xmu.5.6` / `runner-005`: publish first benchmark corpus under shared schema.

## Terminal Gate

This workset is complete when direct native work can use existing reference
runners for both correctness and performance comparison without inventing a new
evidence format.


# Native-Ready Runner And Benchmark Schema v1

Status: `locked-baseline`
Date: 2026-04-30
Scope owner: OxVBA validation/performance/native-readiness

## Purpose

Define a common result schema for VM, JIT, wrapper, and future direct native
correctness/performance runs.

## CSV Columns

Required columns (canonical header also stored at
`docs/evidence/native_ready/runner_samples/native_ready_runner_schema_header_v1.csv`):

```text
run_id,timestamp_utc,host_os,target_arch,workload_id,workload_name,source_path,backend,artifact_kind,artifact_path,artifact_size_bytes,mode,iterations,warmup_iterations,mean_ms,min_ms,max_ms,exit_status,diagnostic_code,fallback_used,fallback_reason,result_kind,result_digest,claim_boundary
```

Allowed `backend` values:

- `vm`
- `jit`
- `wrapper-exe`
- `wrapper-library`
- `native-pe-x64`
- `native-elf-x64`

Allowed `artifact_kind` values:

- `none`
- `oxb`
- `wrapper-exe`
- `wrapper-library`
- `native-exe`
- `native-library`

## Correctness Result Policy

`result_digest` must be derived from a deterministic observable result:

- retained `Variant` snapshot,
- stdout/stderr capture,
- exit status,
- exported function return/argument writeback,
- diagnostic payload.

Runners may emit a richer sidecar JSON artifact, but the CSV row must retain the
digest and claim boundary.

## JIT Fallback Policy

JIT rows must report whether Cranelift executed or the VM fallback path was
used. A JIT row with fallback is valid reference evidence, but it is not native
execution evidence.

## Performance Policy

Performance rows are trend evidence only unless a claim explicitly names:

- workload,
- host class,
- backend pair,
- iteration policy,
- threshold,
- artifact set.

Skipped oracle/native rows are valid boundary evidence, not parity or speed
evidence.

## Native Placeholder Policy

`native-pe-x64` and `native-elf-x64` backend values are reserved for future
direct native work. They must not be emitted by current runners until real PE/ELF
native artifacts execute.


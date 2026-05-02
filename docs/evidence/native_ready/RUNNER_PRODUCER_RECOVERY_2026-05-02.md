# Runner Producer Recovery

Date: 2026-05-02
Bead: `bd-9xmu.5.7`
Status: VM/JIT schema producer restored; wrapper artifact producer deferred to `bd-9xmu.5.8`

## Scope

The recovery audit found that phase-5 runner evidence was schema/sample CSV only.
This pass adds an active Rust producer for VM/JIT rows under
`NATIVE_READY_RUNNER_AND_BENCHMARK_SCHEMA_V1` and a CLI entry point that emits
that schema for loaded projects.

## Active producer paths

- `crates/oxvba-host/src/native_ready_runner.rs`
  - `NATIVE_READY_RUNNER_SCHEMA_HEADER`
  - `NativeReadyRunnerConfig`
  - `produce_native_ready_vm_jit_rows`
  - `emit_native_ready_vm_jit_csv`
- `crates/oxvba-cli/src/main.rs`
  - `oxvba native-ready-runner ...`
- `crates/oxvba-host/tests/native_ready_runner_rows.rs`
  - executable tests for VM/JIT row emission and locked-header CSV output.

## Validation

```text
cargo test -p oxvba-host --test native_ready_runner_rows
  running 2 tests
  test native_ready_runner_csv_uses_locked_header_and_data_rows ... ok
  test native_ready_vm_jit_runner_produces_schema_rows ... ok
```

CLI smoke command executed against `target/native_ready_runner_smoke/Main.bas`:

```text
cargo run -p oxvba-cli --quiet -- native-ready-runner target/native_ready_runner_smoke \
  --run-id-prefix nr-cli-smoke-001 \
  --timestamp-utc 2026-05-02T00:00:00Z \
  --workload-id NR-RUNNER-CLI-001 \
  --workload-name "Native-ready CLI runner smoke" \
  --source-path target/native_ready_runner_smoke/Main.bas \
  --iterations 1
```

Output shape:

```text
run_id,timestamp_utc,host_os,target_arch,workload_id,workload_name,source_path,backend,artifact_kind,artifact_path,artifact_size_bytes,mode,iterations,warmup_iterations,mean_ms,min_ms,max_ms,exit_status,diagnostic_code,fallback_used,fallback_reason,result_kind,result_digest,claim_boundary
nr-cli-smoke-001-vm,...,vm,none,,0,correctness,1,0,...,0,,false,not-applicable,variant-snapshot,fnv1a64:...,Reference runner row produced by active Rust schema producer; not direct native PE/ELF evidence
nr-cli-smoke-001-jit,...,jit,none,,0,correctness,1,0,...,0,,true,project-visible-snapshot-vm-fallback,variant-snapshot,fnv1a64:...,JIT row includes active JIT preflight then uses VM project-visible snapshot fallback; reference evidence only
```

## Claim boundary

- Covered: executable VM/JIT row production with shared header, artifact fields,
  elapsed timing fields, fallback flag/reason, result kind, and deterministic
  snapshot digest.
- JIT boundary: project execution currently performs JIT preflight and then uses
  VM fallback for project-visible snapshot filtering; rows mark
  `fallback_used=true` and are reference evidence, not direct JIT/native
  execution evidence.
- Deferred: wrapper EXE/library rows remain schema/sample-backed until
  `bd-9xmu.5.8` implements a real wrapper artifact runner with artifact path,
  artifact size, execution result, and digest capture.

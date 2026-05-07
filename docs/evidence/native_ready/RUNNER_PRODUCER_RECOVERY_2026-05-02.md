# Runner Producer Recovery

Date: 2026-05-02
Bead: `bd-9xmu.5.7`
Status: VM/JIT and wrapper EXE schema producers restored; wrapper library producer later delivered by `bd-9xmu.5.9`

## Scope

The recovery audit found that phase-5 runner evidence was schema/sample CSV only.
This pass adds an active Rust producer for VM/JIT rows under
`NATIVE_READY_RUNNER_AND_BENCHMARK_SCHEMA_V1` and a CLI entry point that emits
that schema for loaded projects. Follow-up passes added wrapper EXE artifact
build/execute rows and wrapper library exported-call rows through the same CLI
command.

## Active producer paths

- `crates/oxvba-host/src/native_ready_runner.rs`
  - `NATIVE_READY_RUNNER_SCHEMA_HEADER`
  - `NativeReadyRunnerConfig`
  - `produce_native_ready_vm_jit_rows`
  - `emit_native_ready_vm_jit_csv`
- `crates/oxvba-cli/src/main.rs`
  - `oxvba native-ready-runner ...`
  - `--wrapper-exe` / `--wrapper-out <exe>` builds a wrapper EXE artifact,
    executes it, and appends a `backend=wrapper-exe` row with artifact size,
    elapsed timing, exit status, and stdout/stderr digest.
  - `--wrapper-library` / `--wrapper-library-out <dll|so|dylib>` builds a
    wrapper library artifact, invokes a supported exported `NativeExport`, and
    appends a `backend=wrapper-library` row with artifact size, elapsed timing,
    and exported-call digest.
- `crates/oxvba-build/src/exe.rs`
  - generated EXE shims now call `execute_bundle_with_variant_snapshot`.
- `crates/oxvba-host/tests/native_ready_runner_rows.rs`
  - executable tests for VM/JIT row emission and locked-header CSV output.

## Validation

```text
cargo test -p oxvba-host --test native_ready_runner_rows
  running 2 tests
  test native_ready_runner_csv_uses_locked_header_and_data_rows ... ok
  test native_ready_vm_jit_runner_produces_schema_rows ... ok
```

VM/JIT CLI smoke command executed against `target/native_ready_runner_smoke/Main.bas`:

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

Wrapper EXE CLI smoke command executed against the same source:

```text
cargo run -p oxvba-cli --quiet -- native-ready-runner target/native_ready_runner_smoke \
  --run-id-prefix nr-cli-wrapper-smoke-001 \
  --timestamp-utc 2026-05-02T00:00:00Z \
  --workload-id NR-RUNNER-WRAPPER-001 \
  --workload-name "Native-ready wrapper runner smoke" \
  --source-path target/native_ready_runner_smoke/Main.bas \
  --iterations 1 \
  --wrapper-exe \
  --wrapper-out target/native-ready/wrapper/nr-cli-wrapper-smoke-001.exe
```

Output includes a real wrapper row:

```text
nr-cli-wrapper-smoke-001-wrapper-exe,...,wrapper-exe,wrapper-exe,target/native-ready/wrapper/nr-cli-wrapper-smoke-001.exe,1993728,correctness,1,0,...,0,,false,not-applicable,stdout-stderr-exit,fnv1a64:...,Wrapper EXE artifact built and executed by native-ready runner; wrapper host over OXB, not direct native PE/ELF evidence
```

Wrapper library follow-up evidence:

- `docs/evidence/native_ready/WRAPPER_LIBRARY_RUNNER_PRODUCER_2026-05-07.md`

Additional validation:

```text
cargo test -p oxvba-cli parse_native_ready_runner_args_supports_wrapper_exe
cargo test -p oxvba-cli parse_native_ready_runner_args_supports_wrapper_library
cargo test -p oxvba-build exe_shim_contains_project_name
cargo check -p oxvba-cli
cargo check -p oxvba-build
```

## Claim boundary

- Covered: executable VM/JIT row production with shared header, artifact fields,
  elapsed timing fields, fallback flag/reason, result kind, and deterministic
  snapshot digest.
- Covered: executable wrapper EXE row production with real wrapper artifact path,
  artifact size, elapsed timing, exit status, stdout/stderr/exit digest, and
  wrapper-specific claim boundary.
- Covered: executable wrapper library row production with real wrapper artifact
  path, artifact size, elapsed timing, exported `NativeExport` call digest, and
  wrapper-library-specific claim boundary.
- JIT boundary: project execution currently performs JIT preflight and then uses
  VM fallback for project-visible snapshot filtering; rows mark
  `fallback_used=true` and are reference evidence, not direct JIT/native
  execution evidence.
- Wrapper boundary: wrapper EXE and wrapper library rows prove generated wrapper
  execution/invocation over an embedded OXB bundle; they are not direct native
  PE/ELF codegen evidence.

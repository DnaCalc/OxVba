# Runner Schema Lock Evidence

> Recovery note 2026-05-02: the schema remains a useful contract, but the
> recovery audit found sample CSV evidence rather than an active Rust
> Native-Ready runner producer. Reopened recovery bead: `bd-9xmu.5.7`.

Date: 2026-05-01
Bead: `bd-9xmu.5.2` / `runner-001`
Workset: `WORKSET_2026-04-30_REFERENCE_RUNNERS_AND_PERF_SCAFFOLD.md`

## Outcome

`docs/spec/NATIVE_READY_RUNNER_AND_BENCHMARK_SCHEMA_V1.md` is the shared runner
row contract for current VM/JIT/wrapper evidence and future native placeholders.

The schema writer path is:

1. Producers emit CSV rows with the exact header in
   `runner_samples/native_ready_runner_schema_header_v1.csv`.
2. Correctness producers populate `result_kind` and `result_digest` from a
   deterministic retained-Variant snapshot, stdout/stderr, exit status, return
   value/writeback, or diagnostic payload.
3. JIT producers always populate `fallback_used` and `fallback_reason`.
4. Wrapper producers populate `artifact_kind`, `artifact_path`, and
   `artifact_size_bytes` when artifacts exist.
5. Timing fields use milliseconds and remain trend evidence only unless a future
   claim supplies host/workload/backend/iteration/threshold context.
6. `native-pe-x64` / `native-elf-x64` remain reserved placeholders and must not
   be emitted by current runners.

## Validation path

Minimum validation for a producer artifact:

- header exactly equals `native_ready_runner_schema_header_v1.csv`;
- every `backend` and `artifact_kind` value is from the spec enum;
- `fallback_used` is populated for `jit` rows;
- size/timing fields are numeric or empty only when not applicable;
- `claim_boundary` states what the row proves and what it does not prove.

## Verification

Documentation/support-only bead. Validation command:

```text
cargo check --workspace
```

Result: passed.

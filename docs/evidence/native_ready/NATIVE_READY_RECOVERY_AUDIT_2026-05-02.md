# Native-Ready Recovery Audit (2026-05-02)

Status: recovery truth audit after `bd-0w46` / commit `8d5fdfc0`; updated with recovery execution evidence later on 2026-05-02.

## Outcome

The earlier Native-Ready umbrella terminal audit is superseded as current truth.
It closed with `RuntimeValue` residual compatibility blockers still active. Those
blockers were later removed by `bd-0w46`, and the active Rust source now has no
`RuntimeValue` / `runtime_value` references.

Recovered current state:

- child workset 1 (`WORKSET_2026-04-30_DOCS_TRUTH_AND_ARCHIVE_REBASE.md`) is
  materially complete for the original docs-truth/fake-IR/direct-native claim
  cleanup, with this audit documenting the post-`bd-0w46` RuntimeValue truth
  refresh;
- child workset 2 (`WORKSET_2026-04-30_RUNTIMEVALUE_IR_STUB_CLEANOUT.md`) is
  now actually complete for active Rust source: fake IR crate APIs are absent and
  `RuntimeValue` is removed from active Rust source;
- child worksets 3, 4, and 5 are not accepted as complete until recovery beads
  restore or re-prove their terminal gates against the post-`RuntimeValue`
  codebase.

## Hard Gates Rechecked

Commands run from repo root:

```bash
rg -n "RuntimeValue|runtime_value" crates --glob '*.rs'; test $? -eq 1
rg -n "CfgIr|VbaHir|VbaMir|oxvba[_-]ir|lower_to_hir" crates Cargo.toml Cargo.lock | wc -l
cargo test -p oxvba-vm numeric_stress_rounding_overflow_truncation_edges
cargo test -p oxvba-vm coercion_error_stress_rows_cover_empty_null_cverr_and_assignment_timing
cargo test -p oxvba-vm mixed_numeric_matrix_current_variant_results
cargo test -p oxvba-host nested_udt
cargo test -p oxvba-vm -- --list | rg -n "numeric|coerc|mixed|udt|variant|carrier"
cargo test -p oxvba-host -- --list | rg -n "nested_udt|udt|numeric|coerc|variant|carrier"
rg -n "RunnerRow|NativeReady|native_ready_runner|fallback_used|run_id" crates --glob '*.rs'
```

Results:

- Active Rust `RuntimeValue|runtime_value` gate: passed, zero matches.
- Fake IR crate/Cargo gate: passed, zero matches.
- Previously cited phase-3/phase-4 targeted tests now filter to zero tests:
  - `numeric_stress_rounding_overflow_truncation_edges`: 0 tests run;
  - `coercion_error_stress_rows_cover_empty_null_cverr_and_assignment_timing`: 0
    tests run;
  - `mixed_numeric_matrix_current_variant_results`: 0 tests run;
  - `nested_udt`: 0 tests run.
- `cargo test -p oxvba-vm -- --list` currently shows only
  `tests::snapshot_api_returns_variant_snapshot_results` matching the recovery
  query; the documented numeric/coercion/UDT stress tests are absent.
- `cargo test -p oxvba-host -- --list` shows many Variant/COM/pointer tests but
  no `nested_udt` recovery target.
- Runner/perf scaffold appears to be docs/spec/sample CSV only; no active Rust
  producer for the Native-Ready runner schema was found.

## Recovery Decisions

1. Mark the old umbrella completion audit as superseded, not deleted.
2. Update workset docs so child worksets 1 and 2 are the only recovered-complete
   child worksets.
3. Reopen the umbrella and child epics for phases 3, 4, and 5.
4. Reopen or create recovery beads for:
   - phase 3: post-`RuntimeValue` Variant-native value/numeric/UDT gate audit;
   - phase 4: correctness corpus tests that actually execute and fail if broken;
   - phase 5: real runner producer or an explicitly reduced schema/sample-only
     terminal gate.

## Recovery Update

Later on 2026-05-02 the recovery beads restored the missing executable evidence:

- `bd-9xmu.3.9`: phase-3 value/numeric/UDT gates now have nonzero tests; see
  `VALUE_NUMERIC_UDT_RECOVERY_EXECUTABLE_TESTS_2026-05-02.md`.
- `bd-9xmu.4.7`: `NR-NUM-001/002`, `NR-COERCE-001`, and `NR-UDT-001` now have
  nonzero tests; see
  `CORRECTNESS_CORPUS_RECOVERY_EXECUTABLE_STRESS_2026-05-02.md`.
- `bd-9xmu.5.7`: VM/JIT runner row production now has an active Rust producer
  and CLI path; see `RUNNER_PRODUCER_RECOVERY_2026-05-02.md`.
- `bd-9xmu.5.8`: wrapper EXE row production now builds and executes a real
  wrapper artifact through the CLI path; see
  `RUNNER_PRODUCER_RECOVERY_2026-05-02.md`.

## Non-Claims

This audit does not claim direct native PE/ELF compilation exists. Wrapper
library schema production remains deferred to `bd-9xmu.5.9` unless and until
that bead adds a real wrapper library artifact runner.

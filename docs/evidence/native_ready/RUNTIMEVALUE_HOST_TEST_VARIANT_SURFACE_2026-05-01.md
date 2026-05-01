# RuntimeValue host test Variant surface migration — 2026-05-01

## Scope

Migrated small/medium host integration tests from legacy `RuntimeValue` compatibility snapshots/invocation helpers to retained `Variant` surfaces.

## Changed surfaces

- Replaced `RuntimeValueCompatEngineExt` snapshot calls with direct Variant APIs:
  - `execute_source_with_variant_snapshot(_phased)`
  - `execute_project_with_variant_snapshot_phased`
- Replaced compatibility procedure/member invocation with direct Variant APIs:
  - `invoke_procedure_with_variants`
  - `invoke_member_on_object_with_variants`
- Replaced debugger compatibility observation with Variant debugger APIs:
  - `start_variants`, `step_into_variants`, `step_out_variants`
  - `evaluate_variant`, `current_variant_pause_state`
- Rewrote expected values and observation renderers to inspect `Variant` carriers directly.

## Files migrated

- `crates/oxvba-host/tests/console_stdio_end_to_end.rs`
- `crates/oxvba-host/tests/debug_session_host_harness.rs`
- `crates/oxvba-host/tests/end_to_end_mix.rs`
- `crates/oxvba-host/tests/file_io_host_backed_end_to_end.rs`
- `crates/oxvba-host/tests/host_sensitive_oracle_lane.rs`
- `crates/oxvba-host/tests/imported_collection_newenum_regression.rs`
- `crates/oxvba-host/tests/invoke_procedure_tests.rs`
- `crates/oxvba-host/tests/loaded_project_session_duplication_regression.rs`
- `crates/oxvba-host/tests/project_entry_point_end_to_end.rs`
- `crates/oxvba-host/tests/sqliteforexcel_declare_integration.rs`
- `crates/oxvba-host/tests/startup_entry_end_to_end.rs`
- `crates/oxvba-host/tests/vba_attribute_oracle_lane.rs`
- `crates/oxvba-host/tests/xll_application_binding.rs`

## Residual host-test RuntimeValue lanes

Remaining host test files are the larger COM and native/pointer suites and are intentionally left for subsequent coherent slices:

```text
crates/oxvba-host/tests/com_client_end_to_end.rs
crates/oxvba-host/tests/com_client_registered_lane.rs
crates/oxvba-host/tests/com_early_project_end_to_end.rs
crates/oxvba-host/tests/native_declare_string_marshalling_end_to_end.rs
crates/oxvba-host/tests/pointer_helpers_end_to_end.rs
```

## Validation

- `cargo fmt --all`
- `cargo check --workspace --all-targets`
- `rg -l "RuntimeValue|runtime_value" crates/oxvba-host/tests --glob '*.rs'`

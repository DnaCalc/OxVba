# RuntimeValue host native/pointer test Variant migration — 2026-05-01

## Scope

Migrated the Windows native declare and pointer helper host test suites from legacy `RuntimeValue` compatibility snapshots to retained `Variant` snapshots.

## Files migrated

- `crates/oxvba-host/tests/native_declare_string_marshalling_end_to_end.rs`
- `crates/oxvba-host/tests/pointer_helpers_end_to_end.rs`

## Migration details

- Replaced `RuntimeValueCompatEngineExt` imports with direct `Engine` usage.
- Replaced `execute_source_with_value_snapshot` with `execute_source_with_variant_snapshot`.
- Replaced `RuntimeValue` pattern assertions with direct `Variant` inspections:
  - `as_i32`, `as_i64`
  - `as_f32`, `as_f64`, `as_date_f64`
  - `as_currency_scaled_i64`
  - `as_bstr`, `as_bool`
  - `Variant::from_i32`, `from_i64`, `from_bool`, `from_decimal96`

## Residual host-test RuntimeValue lanes

After this slice, host test `RuntimeValue` mentions are confined to the COM suites:

```text
crates/oxvba-host/tests/com_client_end_to_end.rs
crates/oxvba-host/tests/com_client_registered_lane.rs
crates/oxvba-host/tests/com_early_project_end_to_end.rs
```

## Validation

- `cargo fmt --all`
- `cargo check --workspace --all-targets`
- `rg -l "RuntimeValue|runtime_value" crates/oxvba-host/tests --glob '*.rs'`

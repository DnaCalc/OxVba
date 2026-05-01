# RuntimeValue host COM test Variant migration — 2026-05-01

## Scope

Completed host test migration by moving the remaining COM host suites from `RuntimeValue` compatibility observations to retained `Variant` snapshots.

## Files migrated

- `crates/oxvba-host/tests/com_client_end_to_end.rs`
- `crates/oxvba-host/tests/com_early_project_end_to_end.rs`

## Changes

- Removed `RuntimeValueCompatEngineExt` imports from the COM host suites.
- Replaced project/source execution with direct Variant snapshot APIs:
  - `execute_source_with_variant_snapshot_phased`
  - `execute_project_with_variant_snapshot_phased`
- Rewrote canonical object/SAFEARRAY snapshot normalization to operate on `Variant` directly.
- Replaced scalar/string/error/date/currency/decimal/object/SAFEARRAY expectations with `Variant` constructors and accessors.
- Replaced SAFEARRAY compatibility helpers with retained helpers:
  - `from_typed_variants`
  - `replace_variant_elements`
  - `variant_elements`

## Result

`crates/oxvba-host/tests` is clean for `RuntimeValue|runtime_value`.

## Validation

- `cargo fmt --all`
- `cargo check --workspace --all-targets`
- `rg -l "RuntimeValue|runtime_value" crates/oxvba-host/tests --glob '*.rs'` returned no files.
- `rg -l "RuntimeValue|runtime_value" crates --glob '*.rs'` now reports only host source compatibility/test modules, JIT, runtime, and VM internals.

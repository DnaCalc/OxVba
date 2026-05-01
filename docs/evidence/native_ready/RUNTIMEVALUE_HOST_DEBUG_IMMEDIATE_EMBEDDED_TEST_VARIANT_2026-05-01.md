# RuntimeValue host debugger/immediate/embedded test Variant migration — 2026-05-01

## Scope

Migrated host module tests for debugger, immediate, and embedded run-session paths away from `RuntimeValue` compatibility projections.

## Files migrated

- `crates/oxvba-host/src/debugger.rs`
- `crates/oxvba-host/src/immediate.rs`
- `crates/oxvba-host/src/embedded.rs`

## Changes

- Debugger tests now use `start_variants`, `step_into_variants`, `evaluate_variant`, and `current_variant_pause_state`.
- Immediate tests now use `evaluate_variant`, `ImmediateVariantEvaluationOutput`, and `snapshot_variants`.
- Embedded tests now use `EmbeddedInvokeProcedureVariantRequest`, `invoke_procedure_variant`, and `invoke_entry_point_variant`.
- Removed compatibility request/result projection assertions from these module tests.

## Validation

- `cargo fmt --all`
- `cargo check --workspace --all-targets`
- `rg -l "RuntimeValue|runtime_value" crates/oxvba-host/src/debugger.rs crates/oxvba-host/src/immediate.rs crates/oxvba-host/src/embedded.rs` returned no files.

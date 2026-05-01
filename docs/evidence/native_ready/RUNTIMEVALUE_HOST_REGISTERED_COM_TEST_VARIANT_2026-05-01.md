# RuntimeValue registered COM host test Variant migration — 2026-05-01

## Scope

Migrated `crates/oxvba-host/tests/com_client_registered_lane.rs` from legacy `RuntimeValue` compatibility surfaces to retained `Variant` observations.

## Changes

- Replaced `RuntimeValueCompatEngineExt` import with direct host APIs.
- Replaced source execution with `execute_source_with_variant_snapshot_phased`.
- Replaced `poll_com_event_callback` with `poll_com_event_callback_variants`.
- Rewrote callback, object, scalar, string, and SAFEARRAY assertions to use `Variant` APIs:
  - `as_object_ref`, `as_i32`, `as_safearray`, `variant_elements`
  - `Variant::from_i32`, `from_bool`, `from_string`

## Residual host-test RuntimeValue lanes

Host test residuals are now confined to the broad COM client and early-project COM suites:

```text
crates/oxvba-host/tests/com_client_end_to_end.rs
crates/oxvba-host/tests/com_early_project_end_to_end.rs
```

## Validation

- `cargo fmt --all`
- `cargo check --workspace --all-targets`
- `rg -l "RuntimeValue|runtime_value" crates/oxvba-host/tests --glob '*.rs'`

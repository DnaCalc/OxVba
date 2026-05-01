# RuntimeValue COM Dynamic/Model Clean — 2026-05-01

Bead: `bd-0w46` / remove RuntimeValue type and bridges

## Scope

This checkpoint removes direct `RuntimeValue` compatibility methods from COM dynamic/model carrier types where direct `Variant` APIs already exist.

## Changes

- Removed `DynamicValue::from_runtime_value` and `DynamicValue::to_runtime_value`; callers now use `DynamicValue::from_variant` / `variant`.
- Removed `ComValue::from_runtime_value` and `ComValue::to_runtime_value`; tests now assert `ComValue::from_variant` / `to_variant` behavior.
- Rewrote VM tests that populated COM dynamic values through `RuntimeValue` to use `Variant::from_i32`.

## Validation

Commands run from repository root:

```text
cargo fmt --all
cargo check --workspace
cargo check --workspace --all-targets
rg -n "RuntimeValue|runtime_value" crates/oxvba-com/src/dynamic_object.rs crates/oxvba-com/src/model.rs
```

Results:

- `cargo fmt --all`: passed.
- `cargo check --workspace`: passed.
- `cargo check --workspace --all-targets`: passed.
- Dynamic/model carrier search: no matches.

## Residuals

COM crate residuals now concentrate in explicit compatibility modules and Windows invoke/VARIANT internals (`compat.rs`, `platform/portable.rs`, `windows_invoke.rs`, `windows_variant.rs`) plus lower-level helper names still containing `runtime_value`.

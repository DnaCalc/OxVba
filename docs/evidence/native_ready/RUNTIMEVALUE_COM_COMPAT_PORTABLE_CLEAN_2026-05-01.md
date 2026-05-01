# RuntimeValue COM Compat/Portable Clean — 2026-05-01

Bead: `bd-0w46` / remove RuntimeValue type and bridges

## Scope

This checkpoint removes unused `RuntimeValue` compatibility projections from portable COM and narrows `oxvba-com::compat` to scalar token helpers only.

## Changes

- Deleted `platform::portable::compat::RuntimeValueCompatPortableDispatch`; no repository callers remained.
- Removed `RuntimeValue` imports/comments from `platform/portable.rs`.
- Removed `com_value_from_runtime_value`, `com_value_to_runtime_value`, and `variant_to_runtime_value` from `oxvba-com::compat`.
- `ComValue` continues to expose direct `Variant` conversion plus scalar token helpers.

## Validation

Commands run from repository root:

```text
cargo fmt --all
cargo check --workspace
cargo check --workspace --all-targets
rg -n "RuntimeValue|runtime_value" crates/oxvba-com/src/compat.rs crates/oxvba-com/src/platform/portable.rs
rg -l "RuntimeValue" crates/oxvba-com/src --glob '*.rs'
```

Results:

- `cargo fmt --all`: passed.
- `cargo check --workspace`: passed.
- `cargo check --workspace --all-targets`: passed.
- Compat/portable search: no matches.
- COM crate `RuntimeValue` type-name residual files are now limited to `windows_invoke.rs` and `windows_variant.rs`.

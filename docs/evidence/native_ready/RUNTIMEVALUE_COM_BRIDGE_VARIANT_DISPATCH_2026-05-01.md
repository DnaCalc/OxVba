# RuntimeValue COM Bridge Variant Dispatch — 2026-05-01

Bead: `bd-0w46` / remove RuntimeValue type and bridges

## Scope

This checkpoint removes the `RuntimeValue` return surface from `WindowsComBridge` dispatch and event-callback accessors that already had retained `Variant` callers.

## Changes

- Removed `WindowsComBridge::event_callback_arg`; retained `event_callback_variant` remains.
- Rewrote `WindowsComBridge::dispatch_invoke_variant` as the direct bridge dispatch API instead of a wrapper over `dispatch_invoke_runtime_value`.
- Rewrote `WindowsComBridge::dispatch_invoke_dynamic_variant` as the direct dynamic dispatch API instead of a wrapper over `dispatch_invoke_dynamic_runtime_value`.
- Removed the explicit `RuntimeValue` import from `windows_bridge.rs`.

## Validation

Commands run from repository root:

```text
cargo fmt --all
cargo check --workspace --all-targets
rg -n "RuntimeValue" crates/oxvba-com/src/windows_bridge.rs
```

Results:

- `cargo fmt --all`: passed.
- `cargo check --workspace --all-targets`: passed.
- `RuntimeValue` type-name search in `windows_bridge.rs`: no matches.

## Residuals

`windows_bridge.rs` still calls lower-level helper functions whose names include `runtime_value`; those helpers live in `windows_invoke.rs` and remain a later COM-internals migration target.

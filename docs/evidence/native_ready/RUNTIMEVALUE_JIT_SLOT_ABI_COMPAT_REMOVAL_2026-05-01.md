# RuntimeValue JIT Slot ABI Compat Removal — 2026-05-01

Bead: `bd-0w46` / remove RuntimeValue type and bridges

## Scope

This checkpoint removes the unused `RuntimeValue` compatibility adapter from the JIT slot ABI. `RtSlot` now exposes only retained `Variant` construction/access.

## Changes

- Deleted `slot_abi::compat::RuntimeValueCompatRtSlotExt` and `rtslot_from_runtime_value`.
- Rewrote slot ABI unit tests to construct `RtSlot` from `Variant` values directly.
- Malformed pointer-carrier tests now assert retained `Variant` shape rather than projection into a compatibility error string.

## Validation

Commands run from repository root:

```text
cargo fmt --all
cargo check --workspace --all-targets
rg -n "RuntimeValue|runtime_value" crates/oxvba-jit/src/slot_abi.rs
```

Results:

- `cargo fmt --all`: passed.
- `cargo check --workspace --all-targets`: passed.
- JIT slot ABI search: no matches.

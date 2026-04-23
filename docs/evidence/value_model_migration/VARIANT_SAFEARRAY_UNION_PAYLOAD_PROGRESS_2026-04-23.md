# Variant SAFEARRAY Union Payload Progress (2026-04-23)

Status: `vmm-e6` progress evidence, not closure.

Implemented:

1. `Variant` now supports `VT_ARRAY | VT_VARIANT` as `VarType::ArrayVariant`.
2. The `Variant` union payload stores a real raw `SAFEARRAY*` pointer.
3. `Variant` clone/drop/wire-byte paths now clone and release the owned
   `SAFEARRAY*` payload through `SafeArray`.
4. `RuntimeValue::ArrayIntent` now bridges through `Variant` instead of being
   rejected by `Variant::try_from_runtime_value`.

Validation:

1. `cargo test -p oxvba-runtime --lib`

Remaining blocker:

1. This does not close `vmm-e6` because `RuntimeValue` is still a separate
   semantic enum used as the VM/runtime late-bound value surface. The completion
   target still requires the internal late-bound/general `Dim x` value carrier
   itself to be exactly Windows/COM `VARIANT`, with non-VBA internal tokens such
   as `BindingHandle` explicitly outside that carrier.

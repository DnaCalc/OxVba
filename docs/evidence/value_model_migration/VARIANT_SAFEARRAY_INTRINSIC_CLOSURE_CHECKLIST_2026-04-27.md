# Variant/SAFEARRAY Intrinsic Closure Checklist (2026-04-27)

Status: current `vmm-e7` closure checklist.

Scope:

1. `vmm-e6` exact internal `VARIANT` / SAFEARRAY delivery evidence.
2. Remaining `RuntimeValue` projection disposition.
3. Current implemented/projected/bounded state for Variant-family carriers.

## Checks Run

1. `cargo fmt -p oxvba-runtime --check`
2. `cargo check -p oxvba-runtime`
3. `cargo test -p oxvba-runtime safe_array --lib`
4. `cargo fmt -p oxvba-jit -p oxvba-vm --check`
5. `cargo check -p oxvba-jit -p oxvba-vm`
6. `cargo test -p oxvba-jit runtime_array_resize_paths_preserve_variant_slot_carriers --lib`
7. `cargo test -p oxvba-vm intrinsic_array_resize_1d_materializes_zeroed_byte_payload --lib`
8. `cargo test -p oxvba-vm runtime_redim_preserve_1d_retains_overlapping_byte_values --lib`
9. `cargo fmt -p oxvba-com --check`
10. `cargo check -p oxvba-com`
11. `cargo test -p oxvba-com windows_variant --lib`
12. `cargo fmt -p oxvba-com -p oxvba-hal --check`
13. `cargo check -p oxvba-com -p oxvba-hal`
14. `cargo test -p oxvba-com com_value_preserves_safe_array_payload_shape --lib`
15. `cargo test -p oxvba-hal com_safe_array_variant_roundtrips_through_adapter_helpers --lib`
16. `./scripts/check-governance.ps1`

## Implemented

1. VM registers and JIT runtime slots retain canonical runtime `Variant`
   carriers for normal VBA values.
2. Runtime `SafeArray` owns a raw SAFEARRAY-style descriptor with contiguous
   typed payload storage and retained `Variant` element APIs.
3. Runtime `SafeArray` descriptors advertise Automation element metadata
   through `fFeatures` and validate an OxVba owner-prefix marker before raw
   descriptor adoption/cloning.
4. Windows COM scalar, typed SAFEARRAY, and `IEnumVARIANT` result paths
   materialize retained `Variant` carriers before legacy compatibility
   projection.
5. HAL `_variant` companions are the production retained host-service entry
   points used by VM/JIT paths.
6. Host/project/immediate/debugger/CLI/JIT/VM public retained snapshot and
   result surfaces expose `Variant` carriers before legacy projections.
7. Retained-path VM/JIT/runtime/COM/HAL tests construct SAFEARRAY payloads
   through `SafeArray::from_variants`, `from_typed_variants`, or
   `from_typed_variants_nd`.

## Projected

1. `RuntimeValue::{to_variant,from_variant}` and
   `Variant::{try_from_runtime_value,to_runtime_value}` remain explicit
   compatibility bridge APIs.
2. `SafeArray::{from_values,from_values_nd,from_typed_values*,
   from_shape_and_values,elements,replace_elements}` remain public legacy
   compatibility projections beside retained `Variant` APIs.
3. VM/JIT/host `RuntimeValue` snapshot/result methods remain compatibility
   aliases over retained `*_variants` surfaces.
4. HAL legacy `RuntimeValue` methods remain compatibility wrappers over
   retained `_variant` companions.
5. COM model, dynamic COM, Windows COM, immediate/debugger display, and
   pointer-helper legacy `RuntimeValue` methods remain compatibility
   projections around retained carriers.

## Bounded

1. `BindingHandle` remains outside the VBA/COM value model as an explicit
   control-plane token.
2. External COM allocator identity is bounded to the Windows bridge. The
   runtime raw SAFEARRAY adoption API only accepts OxVba-owned descriptors with
   the local owner-prefix marker.
3. Public compatibility APIs are intentionally retained for downstream callers;
   they do not own internal production value storage.

Decision:

1. `vmm-e6` delivery is complete.
2. `vmm-e7` closure checklist is complete for the Variant/SAFEARRAY intrinsic
   family.
3. Follow-on value-model work must not reopen this lane unless a production
   retained path lacks a Variant/SAFEARRAY companion or stores general values
   as `RuntimeValue`.

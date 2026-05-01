# RuntimeValue COM Crate Clean — 2026-05-01

Bead: `bd-0w46` / remove RuntimeValue type and bridges

## Scope

This checkpoint completes the COM crate internal rewrite away from `RuntimeValue` names/types and retains `Variant` as the COM invocation/result carrier.

## Changes

- Replaced Windows invoke helper families with Variant-named/Variant-returning functions:
  - `invoke_dispatch_variant`
  - `invoke_member_spec_variant`
  - `invoke_direct_dispid_variant`
  - `invoke_bound_dispatch_variant_with_shared_state`
  - `execute_bound_variant` / `execute_bound_variant_with_shared_state`
- Replaced Windows VARIANT result projection with `take_variant_result_variant` and direct `variant_to_variant_value` use.
- Updated COM bridge/lib exports to the Variant helper names.
- Rewrote Windows VARIANT tests to assert `Variant`/`SafeArray::from_variants` outputs.

## Validation

Commands run from repository root:

```text
cargo fmt --all
cargo check --workspace
cargo check --workspace --all-targets
rg -n "RuntimeValue|runtime_value" crates/oxvba-com/src --glob '*.rs'
```

Results:

- `cargo fmt --all`: passed.
- `cargo check --workspace`: passed.
- `cargo check --workspace --all-targets`: passed.
- COM source search: no matches.

## Residuals

`RuntimeValue` remains outside `oxvba-com`; subsequent checkpoints should continue with JIT/VM/host/runtime surfaces.

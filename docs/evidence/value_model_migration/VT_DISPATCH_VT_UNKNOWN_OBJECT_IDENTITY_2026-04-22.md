# VT_DISPATCH / VT_UNKNOWN Object Identity Rebinding

Date: 2026-04-22
Bead: `bd-t8rr.6.4` / `vmm-f3`
Status: active evidence

## Purpose

Record the post-`ObjectRef` correctness pass for the object-valued COM result
lanes that are owned by `vmm-f3`:

1. `VT_DISPATCH` results must still rebind into invokable OxVba runtime objects.
2. `VT_UNKNOWN` results that expose `IDispatch` must still rebind into
   invokable OxVba runtime objects.
3. repeated rebinding of the same native object must preserve canonical
   retained `ObjectRef` identity instead of fabricating a fresh runtime object
   with the same compat id.
4. nondispatch `VT_UNKNOWN` lanes must remain bounded failures.

## Implementation landing

The retained Windows COM binding layer now returns the retained runtime object
stored in `ComBinding.runtime_object` rather than reconstructing a new
`ObjectRef` from the compat id every time a dispatch-capable result is rebound.

Primary file:

1. [windows_runtime_state.rs](/C:/Work/DnaCalc/OxVba/crates/oxvba-com/src/windows_runtime_state.rs)

Concretely:

1. `bind_native_dispatch_result(...)` now reuses the retained runtime object
   when the same native `IUnknown` identity is observed again.
2. `insert_bound_object_binding(...)` now returns the retained runtime object
   created for the binding instead of a second fresh `ObjectRef`.
3. `insert_bound_object_binding_at_handle(...)` now preserves the caller's
   retained object identity for non-zero object bindings instead of rebuilding
   by compat id.

## Verification commands

The following focused checks were re-run after the landing:

1. `cargo test -p oxvba-host --test com_client_end_to_end dispatchinvoke_reuses_retained_object_identity_for_dispatch_and_unknown_results -- --test-threads=1 --nocapture`
2. `cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_reuses_imported_object_identity_for_dispatch_and_unknown_results -- --test-threads=1 --nocapture`
3. `cargo test -p oxvba-host --test com_client_end_to_end dispatchinvoke_accepts_object_variant_results -- --test-threads=1 --nocapture`
4. `cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_executes_imported_object_result_member_calls -- --test-threads=1 --nocapture`
5. `cargo test -p oxvba-host --test com_client_end_to_end plain_unknown -- --test-threads=1 --nocapture`
6. `./scripts/check-governance.ps1`

## Findings

1. Late-bound repeated `ReturnSelfDispatch` / `ReturnSelfUnknown` results now
   preserve identical retained `ObjectRef` identity across repeated rebinds.
2. Imported-reference repeated `ReturnSelfDispatch` / `ReturnSelfUnknown`
   results now preserve identical retained `ObjectRef` identity across repeated
   rebinds.
3. The existing invokable-object result lanes still pass after the identity
   change.
4. Plain nondispatch `VT_UNKNOWN` scalar, typed-array, and variant-array lanes
   still fail with the bounded `IUnknown::QueryInterface(IDispatch)` diagnostic.

## Interpretation

`vmm-f3` is no longer relying on compat-id equality as a proxy for object
identity in the core retained COM result path. The repeated object-result lanes
now satisfy the intended canonical-runtime rule:

1. dispatch-capable COM results rebind to stable retained runtime objects, and
2. nondispatch `VT_UNKNOWN` results remain explicit bounded failures.

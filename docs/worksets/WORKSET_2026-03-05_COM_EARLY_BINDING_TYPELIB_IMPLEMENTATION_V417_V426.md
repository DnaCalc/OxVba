# WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_IMPLEMENTATION_V417_V426

## Scope

Execute the first implementation tranche after planning closure:
- PMR type-library identity schema + resolver updates,
- HAL type-library resolver/cache substrate,
- compiler binder rewrite bridge for deterministic early-bound subset,
- runtime lane reuse via `CreateObject`/`DispatchInvoke` lowering,
- design-to-code gate closure with docs and verification.

Profiles covered: `v417..v426`

## Deliverables

1. PMR references support `libid/version/lcid` hints in addition to `importlib`, with deterministic bind statuses and codes.
2. PMR resolver path supports deterministic matching for `libid` first, with importlib fallback and stable ambiguous/unresolved diagnostics.
3. HAL adds a `TypeLibraryHal` contract (`resolve/load/invalidate`) and exposes it through `HostServices`.
4. Windows standard adapter implements deterministic known-identity type-library resolve/load/cache/invalidate behavior; non-Windows adapters return deterministic unsupported errors.
5. Compiler project-lowering pass accepts constrained external declarations (`Dim x As Lib.Type`, `Dim x As New Lib.Type`) and rewrites to existing deterministic COM late-bound intrinsics.
6. Compiler rewrite pass lowers constrained early-bound member calls (`x.Count()`, `x.Exists(v)`) to `DispatchInvoke` token lanes with explicit diagnostics for unsupported members/arity.
7. Module-aware lowering and rewrite-bridge lowering remain parity-checked on early-bound fixtures.
8. Conformance probe touches the new HAL typedef lane.
9. Profile status/evidence/docs are published for `v417..v426` and gate-sync targets advance to `v426`.

## Verification Commands

- `cargo test -p oxvba-host type_library_resolution_binds_unique_libid_identity -- --nocapture`
- `cargo test -p oxvba-host type_library_resolution_reports_ambiguous_libid_identity -- --nocapture`
- `cargo test -p oxvba-hal conformance_l0_passes_for_all_profiles_in_runtime_mode -- --nocapture`
- `cargo test -p oxvba-compiler compile_project_rewrites_as_new_external_type_to_createobject_selector -- --nocapture`
- `cargo test -p oxvba-compiler compile_project_rewrites_early_bound_member_call_to_dispatchinvoke_subset -- --nocapture`
- `cargo test -p oxvba-compiler compile_project_module_aware_matches_rewrite_bridge_for_early_bound_fixture -- --nocapture`
- `cargo test -p oxvba-hal -p oxvba-host -p oxvba-compiler`
- `./scripts/meta-check.ps1 -Fast`

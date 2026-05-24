# Wrapped Native Library Profile

Date: 2026-05-24
Bead: `bd-hjys.11`

## Implementation summary

Refactored the wrapped native-library profile onto wrapper plan abstractions in `crates/oxvba-build/src/dll.rs`:

- `WrappedNativeLibraryProfile`
- `NativeThunkPlan`
- `build_wrapped_native_library_profile(...)`
- `generate_dll_shim_from_wrapper_profile(...)`

The profile consumes neutral `ProjectReflection` plus explicit `NativeExportDescriptor` selections and produces `WrapperGenerationPlan` with `CallableSelectionPlan::ExplicitCallableIds`.

## Acceptance coverage

| Acceptance criterion | Evidence |
| --- | --- |
| Explicit export selection uses descriptor identity. | `dll::wrapper_plan_tests::wrapped_native_profile_selects_explicit_descriptor_identity_only` asserts selected callable IDs in the wrapper plan match thunk callable IDs. |
| Generated native thunks invoke neutral callable runtime path. | `dll::wrapper_plan_tests::wrapped_native_generated_thunk_uses_neutral_callable_typed_path` asserts generated source uses `invoke_callable_typed` and embeds the descriptor callable ID. |
| Non-selected public Functions are not exported. | Same selection test includes another public function and asserts it is not present in thunk plans. |
| Unsupported signatures produce build-time wrapper diagnostics. | `dll::wrapper_plan_tests::wrapped_native_profile_reports_unsupported_signatures` asserts `WRAPPED-NATIVE-UNSUPPORTED-SIGNATURE`. |
| Evidence artifact required. | This file. |

## Checks run

```text
cargo fmt
cargo test -p oxvba-build dll::wrapper_plan_tests -- --nocapture
cargo check --workspace --all-targets
```

Results: all passed.

## Fresh-eyes review notes

- This profile is native-export wrapper policy over neutral descriptors; it does not use UDF admission.
- The existing legacy `generate_dll_shim` remains for older callers until downstream profile migration beads consume the new API.

# Bundle Descriptor Truth

Date: 2026-05-24
Bead: `bd-hjys.4`
Workset: `docs/worksets/WORKSET_2026-05-24_HOST_PROJECT_CALLABLE_REFLECTION_AND_WRAPPER_GENERATION_REWORK.md`
Contract source: `docs/evidence/host_callable/NEUTRAL_DESCRIPTOR_MODEL.md`

## Implementation summary

The bundle descriptor inventory now persists neutral callable descriptors instead
of compiler-invented host/UDF policy.

Changed surfaces:

- `DescriptorInventory.host_calls` replaced with `DescriptorInventory.callables`.
- `BundleHostCallDescriptor` replaced with `BundleCallableDescriptor`.
- Added bundle signature records:
  - `BundleProcedureSignature`
  - `BundleProcedureParameterDescriptor`
  - `BundleVbaTypeDescriptor`
  - `BundleProcedureAnnotation`
- Added explicit unavailable result:
  - `BundleDescriptorInventoryError::Unavailable`
  - `OxBundle::callable_descriptors()` returns `Result<&[BundleCallableDescriptor], BundleDescriptorInventoryError>`.
- Added `OxBundle::project_reflection()` so bundle-loaded projects can consume
  packaged descriptor truth without reparsing source.
- `CompiledProject` now carries `project_reflection` populated at compile time,
  with runtime routes attached from procedure metadata.
- `ProjectRuntimeSession::project_reflection()` exposes the prepared session
  reflection; `compile_and_prepare_session_from_bundle()` uses
  `bundle.project_reflection()` when descriptor inventory is present.

Removed from bundle callable truth:

- `selection_policy`
- `category` / `description` / synthesized `argument_descriptions`
- `volatile`
- `dependency_policy`
- `side_effect_policy`
- `thread_safety_policy`
- `allowed_contexts`
- worksheet/formula context names

## Acceptance coverage

| Acceptance criterion | Evidence |
| --- | --- |
| Source reflection and bundle reflection match for identity/signature facts. | `bundle::tests::source_reflection_and_bundle_callable_inventory_match` compares compiled `ProjectReflection` procedures with `OxBundle::callable_descriptors()` for callable ID, module/procedure names, descriptor fingerprint, parameter count, and return type. |
| Bundle-prepared sessions consume descriptor inventory when present. | `invoke_procedure_tests::bundle_prepared_session_consumes_callable_descriptor_inventory` prepares a host session from an `OxBundle` and verifies `ProjectRuntimeSession::project_reflection()` contains the bundle-provided `Add` descriptor with runtime route slots. |
| Legacy/no-inventory bundles report explicit unavailable or compatibility state. | `bundle::tests::legacy_bundle_reports_callable_inventory_unavailable` asserts `OxBundle::callable_descriptors()` returns `BundleDescriptorInventoryError::Unavailable` for a bundle without descriptor inventory. |
| Descriptor fingerprint change behavior is covered. | `bundle::tests::descriptor_fingerprint_changes_when_signature_changes` compiles two signatures for the same procedure and asserts bundle descriptor fingerprints differ. |
| Evidence artifact required. | This file: `docs/evidence/host_callable/BUNDLE_DESCRIPTOR_TRUTH.md`. |

## Checks run

```text
cargo fmt
cargo test -p oxvba-compiler bundle -- --nocapture
cargo test -p oxvba-host --test invoke_procedure_tests bundle_prepared_session_consumes_callable_descriptor_inventory -- --nocapture
cargo check --workspace --all-targets
```

Results: all passed.

## Fresh-eyes review notes

- The new bundle callable descriptor is intentionally neutral and does not retain
  old policy fields under new names.
- Bundle reflection reconstruction is descriptor-driven. It does not reparse source.
- Legacy/no-inventory behavior is explicit via `BundleDescriptorInventoryError::Unavailable`.
- Runtime route attachment is compile-time descriptor enrichment, not UDF admission.

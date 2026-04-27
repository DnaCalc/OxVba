# V0.2 IDispatch Metadata-Backed Behavior

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.3.3`
Status: complete

## Scope

This bead reconciles the in-scope late-bound `IDispatch` behavior from the
V0.2 supported matrix where authoritative metadata exists:

- `LBD-003`: name-backed member resolution maps through authoritative
  metadata before COM lowering.
- `LBD-006`: named arguments are accepted when metadata supplies parameter
  identities/DISPIDs.
- `LBD-007`: default-member dispatch uses imported/typelib default-member
  identity when available.
- `LBD-009`: unsupported, ambiguous, or metadata-missing shapes keep
  deterministic diagnostics instead of broad parity claims.

## Implementation Evidence

- `crates/oxvba-com/src/invoke_policy.rs` validates named-argument ordering,
  canonicalizes metadata-known named arguments with omitted placeholders, and
  plans default-member dispatch through `ComBinding::default_member_token` when
  a metadata-backed member spec exists.
- `crates/oxvba-hal/src/adapters/standard/com.rs` resolves
  `DynamicMemberSelector::Name` through object metadata before lowering to
  `ComInvokeRequest`; unresolved names produce
  `COM-E-DYNAMIC-NAME-UNRESOLVED`.
- `crates/oxvba-hal/src/adapters/standard/mod.rs` covers controlled native
  COM named-method, named-property, named-default-member, dictionary runtime
  passthrough, and missing-argument diagnostics.
- `crates/oxvba-compiler/src/project.rs` and
  `crates/oxvba-host/tests/com_early_project_end_to_end.rs` cover imported
  metadata default-member rewrites plus wrong-arity, missing-default-member, and
  ambiguous-default-member diagnostics.

## Migration Fix

While running the host evidence lane, `com_early_project_end_to_end.rs` still
used the pre-migration `SafeArray::elements` field form. The helper now uses
the migrated `elements()`/`replace_elements()` API, matching the adjacent COM
end-to-end suite and preserving SAFEARRAY shape during snapshot
canonicalization.

## Verification

Passed:

- `cargo test -p oxvba-com invoke_policy --lib`
- `cargo test -p oxvba-compiler late_bound_named_argument_call_preserves_dispatch_lowering --lib`
- `cargo test -p oxvba-hal windows_native_controlled_test_dispatch_supports_named_default_member_args_runtime_value_v2 --lib`
- `cargo test -p oxvba-hal windows_native_dictionary_named_default_member_passes_through_for_runtime_resolution_v2 --lib`
- `cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_executes_imported_named_argument_calls`
- `cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_reports_compile_error_for_missing_default_member`
- `cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_reports_compile_error_for_ambiguous_default_member`

## Residual Boundary

This does not claim arbitrary Office-wide `IDispatch` parity. Unsupported rows
remain governed by
`docs/evidence/v0_2/V02_IDISPATCH_SUPPORTED_MATRIX_2026-04-27.md`, especially
metadata-missing natural default-member syntax and server-specific
optional-argument synthesis.

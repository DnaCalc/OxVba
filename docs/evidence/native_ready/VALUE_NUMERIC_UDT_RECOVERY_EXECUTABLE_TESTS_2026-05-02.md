# Value/Numeric/UDT Recovery Executable Tests

Date: 2026-05-02
Bead: `bd-9xmu.3.9`
Status: recovered evidence for phase-3 gates after `RuntimeValue` removal

## Scope

This recovery pass replaces the stale phase-3 proof that cited filters which
ran zero tests after the RuntimeValue compatibility deletion. The active proof is
now executable Rust coverage over retained `Variant` carriers.

## Code paths recovered

- `crates/oxvba-vm/src/semantics.rs`
  - `mixed_numeric_matrix_current_variant_results`
  - `exact_scalar_carrier_expectations_preserve_variant_tags`
  - `numeric_stress_rounding_overflow_truncation_edges`
  - `coercion_error_stress_rows_cover_empty_null_cverr_and_assignment_timing`
- `crates/oxvba-host/tests/end_to_end_mix.rs`
  - `nested_udt_field_access_integration`
  - `nested_udt_whole_assignment_copies_declared_field_slots`
  - `nested_udt_cross_type_rejection`
- `crates/oxvba-compiler/src/resolve.rs`
  - UDT declarations now retain a hidden type marker in resolver metadata so
    same-shape cross-type whole-UDT assignment rejects instead of silently
    flatten-copying between distinct declared UDT types.
  - `resolve_udt_cross_type_whole_assignment_is_unsupported` locks the resolver
    diagnostic path.

## Validation

```text
cargo test -p oxvba-vm mixed_numeric_matrix_current_variant_results
  running 1 test
  test semantics::tests::mixed_numeric_matrix_current_variant_results ... ok

cargo test -p oxvba-vm exact_scalar_carrier_expectations_preserve_variant_tags
  running 1 test
  test semantics::tests::exact_scalar_carrier_expectations_preserve_variant_tags ... ok

cargo test -p oxvba-compiler resolve_udt_cross_type_whole_assignment_is_unsupported
  running 1 test
  test resolve::tests::resolve_udt_cross_type_whole_assignment_is_unsupported ... ok

cargo test -p oxvba-host nested_udt
  running 3 tests
  test nested_udt_cross_type_rejection ... ok
  test nested_udt_whole_assignment_copies_declared_field_slots ... ok
  test nested_udt_field_access_integration ... ok
```

## Claim boundary

- Covered: retained `Variant` numeric carriers, mixed arithmetic tags, exact
  `Currency`/`Decimal`/`Date`/Boolean carrier preservation, flattened nested UDT
  field access, whole-UDT copy for the same declared type, and same-shape
  cross-type UDT rejection.
- Not claimed: native struct layout, arbitrary UDT ByRef ABI parity, or direct
  native PE/ELF execution.

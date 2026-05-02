# Correctness Corpus Recovery Executable Stress Tests

Date: 2026-05-02
Bead: `bd-9xmu.4.7`
Status: recovered executable stress coverage after stale zero-test filters

## Scope

This pass restores executable tripwires for the Native-Ready correctness corpus
rows that were invalidated by RuntimeValue compatibility test deletion.

## Recovered rows

| Row | Active test | Evidence |
|---|---|---|
| `NR-NUM-001` / mixed numeric matrix | `cargo test -p oxvba-vm mixed_numeric_matrix_current_variant_results` | nonzero: 1 passed |
| `NR-NUM-002` / numeric rounding-overflow-truncation edges | `cargo test -p oxvba-vm numeric_stress_rounding_overflow_truncation_edges` | nonzero: 1 passed |
| `NR-COERCE-001` / Empty/Null/CVErr/coercion timing | `cargo test -p oxvba-vm coercion_error_stress_rows_cover_empty_null_cverr_and_assignment_timing` | nonzero: 1 passed |
| `NR-UDT-001` / nested UDT semantic subset | `cargo test -p oxvba-host nested_udt` | nonzero: 3 passed |

## Validation transcript

```text
cargo test -p oxvba-vm numeric_stress_rounding_overflow_truncation_edges
  running 1 test
  test semantics::tests::numeric_stress_rounding_overflow_truncation_edges ... ok

cargo test -p oxvba-vm coercion_error_stress_rows_cover_empty_null_cverr_and_assignment_timing
  running 1 test
  test semantics::tests::coercion_error_stress_rows_cover_empty_null_cverr_and_assignment_timing ... ok

cargo test -p oxvba-host nested_udt
  running 3 tests
  test nested_udt_cross_type_rejection ... ok
  test nested_udt_whole_assignment_copies_declared_field_slots ... ok
  test nested_udt_field_access_integration ... ok
```

## Claim boundary

The corpus is again an executable native-readiness tripwire for the scoped
numeric/coercion/UDT semantic rows. Office/VBA oracle packets remain captured or
skipped under their documented headless-CI rationale; this evidence does not
claim direct native execution or broad Office object model parity.

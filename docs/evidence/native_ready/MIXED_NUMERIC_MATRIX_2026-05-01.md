# Mixed Numeric Matrix Evidence

> Recovery note 2026-05-02: this matrix is current again after
> `bd-9xmu.3.9`/`bd-9xmu.4.7`; the cited filter now runs one executable test
> and passes. See `VALUE_NUMERIC_UDT_RECOVERY_EXECUTABLE_TESTS_2026-05-02.md`.

Date: 2026-05-01
Bead: `bd-9xmu.3.5` / `value-clean-004`
Workset: `WORKSET_2026-04-30_VALUE_SUBSTRATE_NUMERIC_UDT_CLEANUP.md`

## Matrix slice

This slice records the current retained-`Variant` mixed numeric behavior covered
by regression test `semantics::tests::mixed_numeric_matrix_current_variant_results`.

| Operation | Left tag | Right tag | Result / behavior |
|---|---|---|---|
| `+` | `Integer` | `Long` | `Long` result (`i32`) |
| `+` | `Long` | `Double` | `Double` result |
| `+` | `Currency` | `Long` | `Double` result using scaled currency value |
| `+` | `Decimal` | `Integer` | `Double` result using decimal numeric value |
| `+` | `Date` | `Long` | `Double` result; date subtype is not preserved by arithmetic helper |
| `+` | `Boolean(True)` | `Long` | `Long`; VBA truth carrier contributes `-1` |
| `+` | numeric `String` | `Long` | `Long` after numeric text compatibility coercion |
| `+` | `Null` | numeric | `Null` propagates |
| `+` | `Error/CVErr` | numeric | type-mismatch error from helper |
| `/` | `Long` | `Long` | `Double` quotient |
| `/` | numeric | zero numeric | runtime error code `11` classification |

## Current constraints

- The matrix intentionally records current implementation behavior; it is not a
  full Excel/VBA oracle parity table.
- Currency/Decimal exactness and date/boolean carrier expectations are expanded
  in `bd-9xmu.3.6`.
- The result matrix is a prerequisite for future native specialization: native
  code may inline only rows with equivalent retained-Variant helper behavior.

## Verification

Passed:

```text
cargo test -p oxvba-vm mixed_numeric_matrix_current_variant_results
cargo test -p oxvba-vm semantics::tests
cargo check --workspace
```

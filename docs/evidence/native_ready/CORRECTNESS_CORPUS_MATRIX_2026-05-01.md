# Correctness Corpus Matrix And Fixture Naming

Date: 2026-05-01
Bead: `bd-9xmu.4.2` / `stress-001`
Workset: `WORKSET_2026-04-30_CORRECTNESS_CORPUS_AND_ORACLE_STRESS.md`

## Matrix schema

Each corpus row must record:

| Field | Meaning |
|---|---|
| `case_id` | Stable ID: `NR-{area}-{nnn}`. |
| `area` | `numeric`, `coercion-error`, `udt-layout`, or `oracle-foldback`. |
| `fixture_or_test` | Test function, fixture path, or oracle packet path. |
| `source_shape` | Short description of code/value shape under test. |
| `expected_result` | Expected Variant/result/error state. |
| `classification` | `oracle`, `spec`, `implementation-defined`, `accepted-subset`, `deferred`, or `blocked`. |
| `backends` | Required lanes such as `vm`, `jit`, `runtime`, `host`, `office-vba`. |
| `claim_boundary` | What the row proves and what it explicitly does not claim. |
| `owner_bead` | Delivery bead that owns the row. |

## Fixture naming convention

- Rust test functions: `{area}_{topic}_{shape}_{expectation}`.
- Source fixtures: `nr_{area}_{nnn}_{topic}.bas` or
  `nr_{area}_{nnn}_{topic}.basproj`.
- Oracle packets: `oracle_{area}_{nnn}_{topic}.{bas,ps1,json}`.
- Evidence rows should keep `case_id` stable even if the underlying fixture path
  changes.

## Initial matrix rows

| case_id | area | fixture_or_test | source_shape | expected_result | classification | backends | claim_boundary | owner_bead |
|---|---|---|---|---|---|---|---|---|
| `NR-NUM-001` | numeric | `mixed_numeric_matrix_current_variant_results` | Integer/Long, Currency/Decimal/Date/Boolean mixed arithmetic | Retained Variant result tags per matrix | implementation-defined | runtime, vm | Current retained-Variant helper behavior, not full Excel parity | `bd-9xmu.4.3` |
| `NR-NUM-002` | numeric | `numeric_stress_rounding_overflow_truncation_edges` | Round tens, Long multiplication overflow, integer division truncation, Mod, exponentiation, Null negation, Single + const | Current retained-Variant helper results, including implementation-defined Long overflow wrap tripwire | implementation-defined | runtime, vm | Numeric tripwire before native specialization; not full Excel overflow parity | `bd-9xmu.4.3` |
| `NR-COERCE-001` | coercion-error | `coercion_error_stress_rows_cover_empty_null_cverr_and_assignment_timing` | blank numeric string, Empty, Null division, CVErr arithmetic, Null comparison, Let/Object assignment timing | Current retained-Variant helper results/errors | implementation-defined/spec | runtime, vm | Coercion/error-state tripwire before native specialization | `bd-9xmu.4.4` |
| `NR-UDT-001` | udt-layout | `nested_udt_field_access_integration`, `nested_udt_cross_type_rejection`, formal whole-copy fixtures | nested fields, whole copy, cross-type rejection | Bounded semantic subset passes; native layout not claimed | accepted-subset | vm, jit, host | Internal UDT semantics only, not native ABI layout | `bd-9xmu.4.5` |
| `NR-ORACLE-001` | oracle-foldback | TBD oracle packet | selected numeric/coercion rows under Office/VBA | Captured or skipped with rationale | oracle/deferred | office-vba | External oracle packet, not required in headless CI | `bd-9xmu.4.6` |

## Verification

Documentation/support-only bead. Validation command:

```text
cargo check --workspace
```

Result: passed.

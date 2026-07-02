# Conformance Oracle Run

- Timestamp (UTC): 2026-07-02T22:35:24Z
- Excel version: 16.0
- Excel process id: 20500
- Dialog guardian enabled: True
- Dialog guardian log: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_w10_typed_pending_20260702\excel_dialog_guardian.log
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_w10_typed_pending_20260702\results.csv

## Summary

| Metric | Count |
|--------|-------|
| Total tests | 14 |
| Oracle OK | 12 |
| Oracle error | 2 |
| Skipped | 0 |
| **Match** | **0** |
| **Mismatch** | **0** |

## All Results

| File | Oracle Status | Oracle Values | Golden Status | Golden Values | Match |
|------|-------------|-------------|--------------|-------------|-------|
| financial_algorithm_npv_irr_mirr_subset.bas | error | `` |  | `` |  |
| financial_algorithm_rate_nper_subset.bas | ok | `f64:0.029228540769134|f64:10` |  | `` |  |
| jit_intrinsic_math_subset.bas | ok | `i16:1` |  | `` |  |
| object_identity_is_nothing.bas | ok | `empty|empty|bool:true|bool:true` |  | `` |  |
| object_identity_is_same_and_different.bas | ok | `empty|empty|bool:true|bool:false` |  | `` |  |
| stdlib_array_introspection_bounds.bas | ok | `empty|i32:0|i32:2` |  | `` |  |
| stdlib_array_introspection_types.bas | ok | `empty|i32:8204|string:"Variant()"` |  | `` |  |
| stdlib_date_serial_value.bas | ok | `f64:46081|f64:46081` |  | `` |  |
| stdlib_financial_zero_rate.bas | ok | `f64:-11|f64:-11|f64:-3` |  | `` |  |
| stdlib_math_transcendental_identity.bas | ok | `f64:0|f64:1|f64:0|f64:1` |  | `` |  |
| stdlib_random_financial_expansion.bas | error | `` |  | `` |  |
| stdlib_rnd_isolated.bas | ok | `f64:76|f64:10` |  | `` |  |
| stdlib_time_serial_value.bas | ok | `f64:4.30902777777778E-02|f64:4.30902777777778E-02` |  | `` |  |
| string_join_array_tag_count.bas | ok | `empty|string:"10203"` |  | `` |  |

# Conformance Oracle Run

- Timestamp (UTC): 2026-07-03T00:43:08Z
- Excel version: 16.0
- Excel process id: 21168
- Dialog guardian enabled: True
- Dialog guardian log: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_variant_integer_probe2_20260703\excel_dialog_guardian.log
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_variant_integer_probe2_20260703\results.csv

## Summary

| Metric | Count |
|--------|-------|
| Total tests | 9 |
| Oracle OK | 9 |
| Oracle error | 0 |
| Skipped | 0 |
| **Match** | **0** |
| **Mismatch** | **0** |

## All Results

| File | Oracle Status | Oracle Values | Golden Status | Golden Values | Match |
|------|-------------|-------------|--------------|-------------|-------|
| zz_probe_variant_add_i16_boundary.bas | ok | `i16:32767|i16:1|i32:32768` |  | `` |  |
| zz_probe_variant_add_small.bas | ok | `i16:0|i16:1|i16:1` |  | `` |  |
| zz_probe_variant_bool_add.bas | ok | `bool:true|i16:1|i16:0` |  | `` |  |
| zz_probe_variant_byte_add.bas | ok | `u8:1|u8:2|u8:3` |  | `` |  |
| zz_probe_variant_long_add.bas | ok | `i32:32768|i16:1|i32:32769` |  | `` |  |
| zz_probe_variant_mul_i16_boundary.bas | ok | `i16:200|i16:200|i32:40000` |  | `` |  |
| zz_probe_variant_neg_boundary.bas | ok | `i32:-32768|i32:32768` |  | `` |  |
| zz_probe_variant_neg_small.bas | ok | `i16:1|i16:-1` |  | `` |  |
| zz_probe_variant_sub_small.bas | ok | `i16:0|i16:1|i16:-1` |  | `` |  |

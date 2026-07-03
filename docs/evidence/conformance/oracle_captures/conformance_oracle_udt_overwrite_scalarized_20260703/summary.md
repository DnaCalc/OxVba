# Conformance Oracle Run

- Timestamp (UTC): 2026-07-03T01:14:50Z
- Excel version: 16.0
- Excel process id: 37304
- Dialog guardian enabled: True
- Dialog guardian log: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_udt_overwrite_scalarized_20260703\excel_dialog_guardian.log
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_udt_overwrite_scalarized_20260703\results.csv

## Summary

| Metric | Count |
|--------|-------|
| Total tests | 1 |
| Oracle OK | 1 |
| Oracle error | 0 |
| Skipped | 0 |
| **Match** | **0** |
| **Mismatch** | **1** |

## Mismatches

| File | Oracle | Golden | Notes |
|------|--------|--------|-------|
| udt_whole_assignment_overwrite.bas | ok: `i16:7|i16:6|i16:7|i16:6` | ok: `empty|i32:7|i32:6|empty|i32:7|i32:6` | oracle=ok/i16:7|i16:6|i16:7|i16:6 golden=ok/empty|i32:7|i32:6|empty|i32:7|i32:6 |

## All Results

| File | Oracle Status | Oracle Values | Golden Status | Golden Values | Match |
|------|-------------|-------------|--------------|-------------|-------|
| udt_whole_assignment_overwrite.bas | ok | `i16:7|i16:6|i16:7|i16:6` | ok | `empty|i32:7|i32:6|empty|i32:7|i32:6` | false |

# VM3 Record Array Field Excel Oracle

- Run ID: vm3_record_array_field_oracle_20260701T2135Z
- Captured: 2026-07-01T21:33:51Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-record-array-field-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| UDT-FIXED-ARRAY-FIELD | ok |  |  | ok | 7 |
| UDT-SCALAR-FIELD-INDEX-GET | compile-error | Compile error: /  / Expected array | RunProbe = s.Value(0) | not-run |  |
| UDT-SCALAR-FIELD-INDEX-SET | compile-error | Compile error: /  / Expected array | s.Value(0) = 7 | not-run |  |

Raw JSON: results.json

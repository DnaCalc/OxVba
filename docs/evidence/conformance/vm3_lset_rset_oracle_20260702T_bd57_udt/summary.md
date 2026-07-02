# VM3 LSet/RSet Excel Oracle

- Run ID: vm3_lset_rset_oracle_20260702T_bd57_udt
- Captured: 2026-07-02T09:50:54Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-lset-rset-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| LSET-UDT-COPY | ok |  |  | ok | \|xy\| |
| LSET-UDT-SAME-LAYOUT-SCALAR | ok |  |  | ok | \|xy\|:513 |
| LSET-UDT-DIFFERENT-SAME-SIZE | ok |  |  | ok | 513:3:4 |
| LSET-UDT-SOURCE-SHORTER | ok |  |  | ok | 120,121,122,122 |
| LSET-UDT-SOURCE-LONGER | ok |  |  | ok | 119,120 |
| LSET-UDT-FIXED-ARRAY | ok |  |  | ok | 1:2:3:4 |
| LSET-UDT-RHS-NONRECORD | compile-error | Compile error: /  / Type mismatch / Gauge | LSet a = "xy" | not-run |  |
| RSET-UDT-TARGET | compile-error | Compile error: /  / RSet allowed only on strings / Gauge | RSet a = b | not-run |  |
| LSET-UDT-VARIABLE-STRING | compile-error | Compile error: /  / Type mismatch / Gauge | LSet a = b | not-run |  |

Raw JSON: results.json

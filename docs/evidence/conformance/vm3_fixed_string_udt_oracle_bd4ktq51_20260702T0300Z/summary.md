# VM3 Fixed-String UDT Excel Oracle

- Run ID: vm3_fixed_string_udt_oracle_bd4ktq51_20260702T0300Z
- Captured: 2026-07-02T02:32:04Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-fixed-string-udt-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value | Error |
|---|---|---|---|---|---|---|
| UDT-FIXED-DEFAULT | ok |  |  | ok | 5:\|\0\0\0\0\0\|:0 |  |
| UDT-FIXED-SHORT | ok |  |  | ok | 5:\|ab   \|:32 |  |
| UDT-FIXED-LONG | ok |  |  | ok | 5:\|abcde\| |  |
| UDT-FIXED-ARRAY-ELEMENT | ok |  |  | ok | 5:\|\0\0\0\0\0\|;5:\|xy   \| |  |
| UDT-FIXED-WHOLE-COPY | ok |  |  | ok | 5:\|ab   \| |  |
| UDT-FIXED-NULL | ok |  |  | ok | err:94:Invalid use of Null:\|\0\0\0\0\0\| |  |
| UDT-FIXED-LEN-SIZE | ok |  |  | ok | 8:14 |  |

Raw JSON: results.json

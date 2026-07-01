# VM3 LSet/RSet Excel Oracle

- Run ID: vm3_lset_rset_oracle_20260701T215755Z
- Captured: 2026-07-01T22:02:58Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-lset-rset-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| LSET-FIXED-SHORT | ok |  |  | ok | 5:\|ab   \| |
| RSET-FIXED-SHORT | ok |  |  | ok | 5:\|   ab\| |
| LSET-FIXED-LONG | ok |  |  | ok | 5:\|abcde\| |
| RSET-FIXED-LONG | ok |  |  | ok | 5:\|abcde\| |
| LSET-VARIABLE-PRESEEDED | ok |  |  | ok | 5:\|ab   \| |
| RSET-VARIABLE-PRESEEDED | ok |  |  | ok | 5:\|   ab\| |
| LSET-VARIABLE-EMPTY | ok |  |  | ok | 0:\|\| |
| RSET-VARIABLE-EMPTY | ok |  |  | ok | 0:\|\| |
| LSET-VARIABLE-LONG | ok |  |  | ok | 3:\|abc\| |
| RSET-VARIABLE-LONG | ok |  |  | ok | 3:\|abc\| |
| RSET-FIXED-NUMERIC | ok |  |  | ok | 5:\|   42\| |
| LSET-FIXED-NULL | ok |  |  | ok | err:94:Invalid use of Null |
| RSET-FIXED-NULL | ok |  |  | ok | err:94:Invalid use of Null |
| LSET-LONG-TARGET | compile-error | Compile error: /  / LSet allowed only on strings and user-defined types / Gauge | LSet n = "12" | not-run |  |
| RSET-LONG-TARGET | compile-error | Compile error: /  / RSet allowed only on strings / Gauge | RSet n = "12" | not-run |  |
| LSET-UDT-COPY | ok |  |  | ok | \|xy\| |

Raw JSON: results.json

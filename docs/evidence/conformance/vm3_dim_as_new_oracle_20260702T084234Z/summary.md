# VM3 Dim As New Excel Oracle

- Run ID: vm3_dim_as_new_oracle_20260702T084234Z
- Captured: 2026-07-02T08:44:22Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-dim-as-new-oracle.ps1
- Class setup: imported .cls file with Attribute VB_PredeclaredId = False, so only Dim As New can create instances.
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value | Error |
|---|---|---|---|---|---|---|
| LOCAL-DIM-ONLY | ok |  |  | ok | 0\| |  |
| LOCAL-FIRST-MEMBER | ok |  |  | ok | 11\|I; |  |
| LOCAL-IS-NOTHING | ok |  |  | ok | False\|I; |  |
| LOCAL-SET-NOTHING-BEFORE-ACCESS | ok |  |  | ok | 0\| |  |
| LOCAL-SET-NOTHING-RESURRECT | ok |  |  | ok | 10\|I;T11;I; |  |
| GLOBAL-DIM-ONLY | ok |  |  | ok | 0\| |  |
| GLOBAL-SET-NOTHING-RESURRECT | ok |  |  | ok | 10\|I;T11;I; |  |

Raw JSON: results.json

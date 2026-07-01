# VM3 On n GoTo/GoSub Selector Excel Oracle

- Run ID: vm3_on_computed_branch_oracle_20260701T2321Z
- Captured: 2026-07-01T23:19:06Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-on-computed-branch-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| GOTO-IN-RANGE-1 | ok |  |  | ok | L1:0 |
| GOTO-ZERO | ok |  |  | ok | fallthrough:0 |
| GOTO-NEGATIVE | ok |  |  | ok | err:5 |
| GOTO-OUT-OF-RANGE | ok |  |  | ok | fallthrough:0 |
| GOTO-FRACTION-1-5 | ok |  |  | ok | L2:0 |
| GOTO-FRACTION-2-5 | ok |  |  | ok | L2:0 |
| GOTO-STRING-NONNUMERIC | ok |  |  | ok | err:13 |
| GOTO-NULL | ok |  |  | ok | err:94 |
| GOSUB-IN-RANGE-2 | ok |  |  | ok | before:S2:after:0 |
| GOSUB-ZERO | ok |  |  | ok | before:after:0 |
| GOSUB-NEGATIVE | ok |  |  | ok | err:5:before |
| GOSUB-OUT-OF-RANGE | ok |  |  | ok | before:after:0 |
| GOSUB-FRACTION-1-5 | ok |  |  | ok | before:S2:after:0 |

Raw JSON: results.json

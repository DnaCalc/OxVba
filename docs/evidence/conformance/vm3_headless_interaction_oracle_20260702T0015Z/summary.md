# VM3 Headless Interaction Excel Oracle

- Run ID: vm3_headless_interaction_oracle_20260702T0015Z
- Captured: 2026-07-02T00:08:46Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-headless-interaction-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.
- Debug.Assert note: false assertions are an IDE debugger break-state boundary in Excel/VBE, not a headless runtime modal; this oracle intentionally captures Shell runtime timing only.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| SHELL-RETURNS-BEFORE-EXIT | ok |  |  | ok | 5\|True\|0.016 |

Raw JSON: results.json

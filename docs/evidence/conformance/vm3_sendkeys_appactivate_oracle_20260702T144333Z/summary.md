# VM3 SendKeys/AppActivate Excel Oracle

- Run ID: vm3_sendkeys_appactivate_oracle_20260702T144333Z
- Captured: 2026-07-02T14:45:14Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-sendkeys-appactivate-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.
- Probe safety: SendKeys uses an empty key string; no real keystrokes are injected.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| SENDKEYS-EMPTY-STATEMENT | ok |  |  | ok | 0: |
| APPACTIVATE-MISSING-WINDOW | ok |  |  | ok | 5:Invalid procedure call or argument |
| SENDKEYS-EXPRESSION | compile-error | Compile error: /  / Expected Function or variable / Gauge | RunProbe = SendKeys("") | not-run |  |
| APPACTIVATE-EXPRESSION | compile-error | Compile error: /  / Expected Function or variable / Gauge | RunProbe = AppActivate("__OXVBA_NO_SUCH_WINDOW_20260702__") | not-run |  |

Raw JSON: results.json

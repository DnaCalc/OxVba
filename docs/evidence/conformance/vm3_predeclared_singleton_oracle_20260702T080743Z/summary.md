# VM3 Predeclared Singleton Excel Oracle

- Run ID: vm3_predeclared_singleton_oracle_20260702T080743Z
- Captured: 2026-07-02T08:09:48Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-predeclared-singleton-oracle.ps1
- Class setup: imported .cls file with Attribute VB_PredeclaredId = True, not CodeModule text injection.
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value | Error |
|---|---|---|---|---|---|---|
| PERSISTENCE | ok |  |  | ok | 12\|I; |  |
| LOCAL-REF-SET-NOTHING | ok |  |  | ok | 11\|I; |  |
| SET-PREDECLARED-NOTHING | ok |  |  | ok | 11:10\|I;T11;I; |  |
| SET-PREDECLARED-NOTHING-IDENTITY | ok |  |  | ok | True:10\|I;T11;I; |  |
| SET-PREDECLARED-NEW | ok |  |  | ok | 10\|I;I;T11; |  |
| HELD-OLD-REF-THEN-RESET | ok |  |  | ok | 11:10\|I;I; |  |

Raw JSON: results.json

# VM3 Option Explicit Ordinary Undeclared Name Excel Oracle

- Run ID: vm3_option_explicit_oracle_20260702T140228Z
- Captured: 2026-07-02T14:05:54Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-option-explicit-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| NOEXPLICIT-UNDECLARED-READ-EMPTY | ok |  |  | ok | 0:Empty |
| NOEXPLICIT-UNDECLARED-ASSIGN-READ | ok |  |  | ok | 7:2 |
| NOEXPLICIT-UNDECLARED-BYREF | ok |  |  | ok | 12:2 |
| NOEXPLICIT-MISSING-CALL | compile-error | Compile error: /  / Sub or Function not defined | MissingProc | not-run |  |
| NOEXPLICIT-MISSING-FUNCTION-CALL | compile-error | Compile error: /  / Sub or Function not defined | x = MissingProc(1) | not-run |  |
| EXPLICIT-UNDECLARED-READ | compile-error | Compile error: /  / Variable not defined | RunProbe = x | not-run |  |
| EXPLICIT-UNDECLARED-ASSIGN | compile-error | Compile error: /  / Variable not defined | x = 7 | not-run |  |
| EXPLICIT-UNDECLARED-BYREF | compile-error | Compile error: /  / Variable not defined | Inc x | not-run |  |

Raw JSON: results.json

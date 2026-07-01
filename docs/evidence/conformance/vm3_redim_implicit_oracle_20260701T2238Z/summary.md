# VM3 ReDim Implicit Declaration Excel Oracle

- Run ID: vm3_redim_implicit_oracle_20260701T2238Z
- Captured: 2026-07-01T22:37:01Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-redim-implicit-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| NOEXPLICIT-REDIM-UNDECLARED | ok |  |  | ok | 0:1:8204:2:7 |
| EXPLICIT-REDIM-UNDECLARED | ok |  |  | ok | 0:1:8204:2:7 |
| EXPLICIT-UNDECLARED-READ-CONTROL | compile-error | Compile error: /  / Variable not defined | RunProbe = missingName | not-run |  |
| EXPLICIT-REDIM-THEN-UNDECLARED-READ | compile-error | Compile error: /  / Variable not defined | RunProbe = missingName | not-run |  |
| EXPLICIT-DECLARED-VARIANT-REDIM | ok |  |  | ok | 0:1:8204:2:7 |
| EXPLICIT-DECLARED-DYNAMIC-LONG-REDIM | ok |  |  | ok | 0:1:8195:3:7 |
| EXPLICIT-SCALAR-LONG-REDIM | compile-error | Compile error: /  / Expected array | ReDim a(1) | not-run |  |
| EXPLICIT-REDIM-PRESERVE-UNDECLARED | compile-error | Compile error: /  / Variable not defined | ReDim Preserve a(1) | not-run |  |
| NOEXPLICIT-REDIM-PRESERVE-UNDECLARED | compile-error | Compile error: /  / Variable not defined | ReDim Preserve a(1) | not-run |  |

Raw JSON: results.json

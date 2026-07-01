# VM3 Scoping Visibility Excel Oracle

- Run ID: vm3_scoping_visibility_oracle_20260701T0945Z
- Captured: 2026-07-01T09:54:39Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-scoping-visibility-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, then PID-scoped process cleanup.

| Case | Compile | Dialog | Run | Value |
|---|---|---|---|---|
| SCOPING-SAME-MODULE-PRIVATE | ok |  | ok | 7 |
| SCOPING-CROSS-UNQUAL-PRIVATE | compile-error | Compile error: /  / Sub or Function not defined | not-run |  |
| SCOPING-CROSS-QUAL-PRIVATE | compile-error | Compile error: /  / Method or data member not found | not-run |  |
| SCOPING-DUP-PUBLIC | compile-error | Compile error: /  / Ambiguous name detected: Dup | not-run |  |
| SCOPING-MODULE-MEMBER-COLLISION | compile-error | Compile error: /  / Expected variable or procedure, not module | not-run |  |
| SCOPING-VALID-PROJECT-QUALIFIER | ok |  | ok | 13 |
| SCOPING-WRONG-PROJECT-QUALIFIER | compile-error | Compile error: /  / Variable not defined | not-run |  |
| SCOPING-FRIEND-STANDARD-MODULE | compile-error | Compile error: /  / Only valid in object module | not-run |  |
| SCOPING-FRIEND-CLASS-MODULE | ok |  | ok | 19 |

Raw JSON: results.json

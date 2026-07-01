# VM3 Err HelpFile/HelpContext Excel Oracle

- Run ID: vm3_err_help_oracle_20260701T2332Z
- Captured: 2026-07-01T23:31:06Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-err-help-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| ERR-INITIAL-FIELDS | ok |  |  | ok | 0\|\|\|\|0\|0 |
| ERR-HELP-WRITE-READ | ok |  |  | ok | help.chm\|42 |
| ERR-CLEAR-RESETS-HELP | ok |  |  | ok | 0\|\|\|\|0\|0 |
| ERROR-STATEMENT-HELP-DEFAULTS | ok |  |  | ok | 9\|Subscript out of range\|VBAProject\|C:\Program Files\Common Files\Microsoft Shared\VBA\VBA7.1\1033\VbLR6.chm\|1000009\|0 |
| ERR-RAISE-EXPLICIT-HELP | ok |  |  | ok | 77\|desc\|src\|help.chm\|42 |
| ERR-RAISE-NAMED-HELP | ok |  |  | ok | 78\|desc2\|src2\|named.hlp\|43 |
| ERR-RAISE-OMITTED-INHERITS-HELP | ok |  |  | ok | 79\|Application-defined or object-defined error\|VBAProject\|C:\Program Files\Common Files\Microsoft Shared\VBA\VBA7.1\1033\VbLR6.chm\|1000095 |
| ERR-RAISE-AFTER-CLEAR-DEFAULTS-HELP | ok |  |  | ok | 80\|Application-defined or object-defined error\|VBAProject\|C:\Program Files\Common Files\Microsoft Shared\VBA\VBA7.1\1033\VbLR6.chm\|1000095 |
| ERR-RAISE-PARTIAL-HELP-INHERIT | ok |  |  | ok | 81\|Application-defined or object-defined error\|VBAProject\|explicit.hlp\|1000095 |
| ERR-RAISE-RESUME-NEXT-INHERITS-ACTUAL | ok |  |  | ok | 79\|prevdesc\|prevsrc\|prev.hlp\|9 |
| ERR-RAISE-RESUME-NEXT-DIRECT-WRITES | ok |  |  | ok | 79\|prevdesc\|prevsrc\|prev.hlp\|9 |
| ERR-RAISE-PARTIAL-HELP-AFTER-ACTUAL | ok |  |  | ok | 81\|prevdesc\|prevsrc\|explicit.hlp\|9 |

Raw JSON: results.json

# VM3 Erl And Line-Numbered Error Flow Excel Oracle

- Run ID: vm3_erl_line_oracle_20260701T2318Z
- Captured: 2026-07-01T22:51:35Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-erl-line-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| ERL-INITIAL | ok |  |  | ok | 0:3 |
| ERL-NUMERIC-LINE-NO-ERROR | ok |  |  | ok | 0:3:1 |
| ERL-RESUME-NEXT-SAME-LINE | ok |  |  | ok | 11:10 |
| ERL-GOTO-HANDLER-SAME-LINE | ok |  |  | ok | 5:10 |
| ERL-PRIOR-NUMERIC-LINE-UNNUMBERED-FAULT | ok |  |  | ok | 11:10 |
| ERL-COLON-LINE-THEN-FAULT | ok |  |  | ok | 11:10 |
| ERL-CALLEE-LINE-CAUGHT-BY-CALLER | ok |  |  | ok | 7:0 |
| COMPILE-ONERROR-UNDEFINED-LABEL | compile-error | Compile error: /  / Label not defined / Gauge | On Error GoTo MissingHandler | not-run |  |

Raw JSON: results.json

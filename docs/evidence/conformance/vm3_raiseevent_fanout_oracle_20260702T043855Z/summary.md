# VM3 RaiseEvent Fan-Out Excel Oracle

- Run ID: vm3_raiseevent_fanout_oracle_20260702T043855Z
- Captured: 2026-07-02T04:41:01Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-raiseevent-fanout-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value | Error |
|---|---|---|---|---|---|---|
| SAME-SINK-FIELDS-WIRE-FIRST-SECOND | ok |  |  | ok | first1;second11;\|112 |  |
| SAME-SINK-FIELDS-WIRE-SECOND-FIRST | ok |  |  | ok | second1;first12;\|121 |  |
| TWO-SINKS-CREATE-A-B-WIRE-B-A | ok |  |  | ok | B1;A12;\|121 |  |
| TWO-SINKS-CREATE-B-A-WIRE-A-B | ok |  |  | ok | A1;B11;\|112 |  |
| REBIND-SAME-FIELD-MOVES-OR-PRESERVES | ok |  |  | ok | B1;A12;\|121 |  |
| CLEAR-THEN-REWIRE-MOVES | ok |  |  | ok | B1;A12;\|121 |  |
| REASSIGN-OLD-SOURCE-DETACHED | ok |  |  | ok | 1;\|1\|17 |  |

Raw JSON: results.json

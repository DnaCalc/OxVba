# VM3 Scoping Follow-up Excel Oracle

- Run ID: vm3_scoping_followup_oracle_20260701T1655Z
- Captured: 2026-07-01T16:52:50Z
- Harness: C:\Work\DnaCalc\OxVba\scripts\run-vm3-scoping-followup-oracle.ps1
- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE document when exposed, owned-dialog dismissal, then PID-scoped process cleanup.

| Case | Compile | Dialog | Selected | Run | Value |
|---|---|---|---|---|---|
| SCOPING-XREF-BASELINE | ok |  |  | ok | 42 |
| SCOPING-XREF-MODULE-QUALIFIED | ok |  |  | ok | 42 |
| SCOPING-XREF-PROJECT-QUALIFIED | ok |  |  | ok | 30 |
| SCOPING-CONST-VAR-COLLISION | compile-error | Compile error: /  / Ambiguous name detected: SharedName / Gauge | SharedName | not-run |  |
| SCOPING-UDT-ENUM-COLLISION | compile-error | Compile error: /  / Ambiguous name detected: Payload / Gauge | Value As Payload | not-run |  |
| SCOPING-OPTION-PRIVATE-XREF | compile-error | Compile error: /  / Sub or Function not defined / Gauge | HiddenValue | not-run |  |
| SCOPING-XREF-PRECEDENCE | ok |  |  | ok | 102 |
| SCOPING-WITHEVENTS-ACTIVE | ok |  |  | ok | 23 |

Raw JSON: results.json

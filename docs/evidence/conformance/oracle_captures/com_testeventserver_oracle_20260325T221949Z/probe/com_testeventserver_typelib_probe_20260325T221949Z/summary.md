# COM TestEventServer Typelib Probe

- Run ID: 20260325T221949Z
- Generated UTC: 2026-03-25T22:19:54Z
- Registration path: HKCU current-user reg import
- Typelib export path: TlbExp.exe
- TypeLib file: C:\Work\DnaCalc\OxVba\tools\OxVba.TestEventServer\bin\Debug\net48\OxVba.TestEventServer.tlb
- Reference name: OxVba_TestEventServer
- Reference GUID: {E2A30001-0001-0001-0001-000000000001}
- Reference version: 1.0
- Reference broken: False
- Results CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_oracle_20260325T221949Z\probe\com_testeventserver_typelib_probe_20260325T221949Z\results.csv

## Cases
| Case | Scenario | Status | Observed |
|---|---|---|---|
| CCT-027-TES-001 | AddFromFile + New TestEventServer + Ping() | ok | 42 |
| CCT-027-TES-002 | AddFromFile + WithEvents TestEventServer source interface | ok | 7 |
| CCT-048-TES-001 | Saved workbook reopened after referenced .tlb file is removed | ok | reference missing from reopened VBProject.References set |

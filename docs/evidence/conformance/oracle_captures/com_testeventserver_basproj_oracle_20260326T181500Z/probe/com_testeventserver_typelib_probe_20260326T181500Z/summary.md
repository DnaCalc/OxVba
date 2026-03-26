# COM TestEventServer Typelib Probe

- Run ID: 20260326T181500Z
- Generated UTC: 2026-03-26T16:10:37Z
- Registration path: HKCU current-user reg import
- Typelib export path: TlbExp.exe
- TypeLib file: C:\Work\DnaCalc\OxVba\tools\OxVba.TestEventServer\bin\Debug\net48\OxVba.TestEventServer.tlb
- Reference name: OxVba_TestEventServer
- Reference GUID: {E2A30001-0001-0001-0001-000000000001}
- Reference version: 1.0
- Reference broken: False
- Results CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_basproj_oracle_20260326T181500Z\probe\com_testeventserver_typelib_probe_20260326T181500Z\results.csv

## Cases
| Case | Scenario | Status | Observed |
|---|---|---|---|
| CCT-027-TES-001 | AddFromFile + New TestEventServer + Ping() | ok | 42 |
| CCT-027-TES-002 | AddFromFile + WithEvents TestEventServer source interface | ok | 7 |
| CCT-048-TES-001 | Saved workbook reopened after referenced .tlb file is removed | ok | reference missing from reopened VBProject.References set |

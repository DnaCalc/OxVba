# COM TestEventServer Versioned Typelib Probe

- Run ID: 20260325T222709Z
- Generated UTC: 2026-03-25T22:32:00Z
- v1 TypeLib: C:\Work\DnaCalc\OxVba\tools\OxVba.TestEventServer\bin\Debug\net48\OxVba.TestEventServer.tlb
- v2 TypeLib: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_versioned_typelib_probe_20260325T222709Z\variant_v2\bin\Debug\net48\OxVba.TestEventServer.tlb
- Results CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_versioned_typelib_probe_20260325T222709Z\results.csv

## Cases
| Case | Scenario | Status | Observed |
|---|---|---|---|
| CCT-048-TES-002 | AddFromFile v1 typelib + New TestEventServer + Ping() | ok | name=OxVba_TestEventServer;version=1.0;result=42 |
| CCT-048-TES-003 | AddFromFile v2 typelib + New TestEventServer + Ping() | ok | name=OxVba_TestEventServer;version=2.0;result=42 |
| CCT-048-TES-004 | Saved workbook reopened after referenced typelib path is replaced with v2 | ok | name=OxVba_TestEventServer;version=1.0;broken=False;result=42 |
| CCT-048-TES-005 | Saved workbook reopened after referenced typelib file is removed | ok | name=;version=1.0;broken=True |
| CCT-048-TES-006 | Saved workbook reopened after missing typelib file is restored | ok | name=OxVba_TestEventServer;version=1.0;broken=False;result=42 |

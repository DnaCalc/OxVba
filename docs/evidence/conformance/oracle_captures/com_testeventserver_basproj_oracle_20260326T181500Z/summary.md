# COM TestEventServer BasProj Oracle Run

- Run ID: 20260326T181500Z
- Generated UTC: 2026-03-26T16:11:37Z
- Probe CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_basproj_oracle_20260326T181500Z\probe\com_testeventserver_typelib_probe_20260326T181500Z\results.csv
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_basproj_oracle_20260326T181500Z\results.csv
- Total cases: 1
- Match count: 1
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-043 | CCT-043-TES-001 | ok: 42 | ok: 42 | true | Excel baseline reuses file-backed .tlb AddFromFile probe CCT-027-TES-001; OxVba anchor: com_early_project_end_to_end::early_bound_loaded_basproj_executes_registered_testeventserver_ping; command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_executes_registered_testeventserver_ping -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_basproj_oracle_20260326T181500Z\CCT-043-TES-001.log.txt |

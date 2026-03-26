# COM TestEventServer Reference Order Oracle Run

- Run ID: 20260326T171500Z
- Generated UTC: 2026-03-26T16:51:03Z
- Base TypeLib: C:\Work\DnaCalc\OxVba\tools\OxVba.TestEventServer\bin\Debug\net48\OxVba.TestEventServer.tlb
- Alt TypeLib: C:\Work\DnaCalc\OxVba\temp\generated\com_testeventserver_reference_order\20260326T171500Z\OxVba.TestEventServerAlt\bin\Debug\net48\OxVba.TestEventServerAlt.tlb
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_reference_order_oracle_20260326T171500Z\results.csv
- Total cases: 2
- Match count: 2
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-043 | CCT-043-TES-ORDER-001 | ok: 42 | ok: 42 | true | Excel reference-order=OxVba_TestEventServer:{E2A30001-0001-0001-0001-000000000001};OxVba_TestEventServerAlt:{E2A30001-0001-0001-0001-000000000101}; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_prefers_first_typelib_reference_for_unqualified_testeventserver -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_reference_order_oracle_20260326T171500Z\CCT-043-TES-ORDER-001.log.txt |
| CCT-043 | CCT-043-TES-ORDER-002 | ok: 84 | ok: 84 | true | Excel reference-order=OxVba_TestEventServerAlt:{E2A30001-0001-0001-0001-000000000101};OxVba_TestEventServer:{E2A30001-0001-0001-0001-000000000001}; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_prefers_reversed_first_typelib_reference_for_unqualified_testeventserver -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_reference_order_oracle_20260326T171500Z\CCT-043-TES-ORDER-002.log.txt |

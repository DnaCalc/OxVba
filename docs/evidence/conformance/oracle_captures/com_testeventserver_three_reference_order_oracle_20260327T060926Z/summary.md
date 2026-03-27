# COM TestEventServer Three-Reference Order Oracle Run

- Run ID: 20260327T060926Z
- Generated UTC: 2026-03-27T06:10:40Z
- Base TypeLib: C:\Work\DnaCalc\OxVba\tools\OxVba.TestEventServer\bin\Debug\net48\OxVba.TestEventServer.tlb
- Alt TypeLib: C:\Work\DnaCalc\OxVba\temp\generated\com_testeventserver_three_reference_order\20260327T060926Z\OxVba.TestEventServerAlt\bin\Debug\net48\OxVba.TestEventServerAlt.tlb
- Alt2 TypeLib: C:\Work\DnaCalc\OxVba\temp\generated\com_testeventserver_three_reference_order\20260327T060926Z\OxVba.TestEventServerAlt2\bin\Debug\net48\OxVba.TestEventServerAlt2.tlb
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_order_oracle_20260327T060926Z\results.csv
- Total cases: 3
- Match count: 3
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-043 | CCT-043-TES-ORDER3-001 | ok: 42 | ok: compile-selected-progid=OxVba.TestEventServer | true | Excel reference-order=OxVba_TestEventServer:{E2A30001-0001-0001-0001-000000000001};OxVba_TestEventServerAlt:{E2A30001-0001-0001-0001-000000000101};OxVba_TestEventServerAlt2:{E2A30001-0001-0001-0001-000000000201}; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_prefers_first_of_three_typelib_references_for_unqualified_testeventserver -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_order_oracle_20260327T060926Z\CCT-043-TES-ORDER3-001.log.txt |
| CCT-043 | CCT-043-TES-ORDER3-002 | ok: 84 | ok: compile-selected-progid=OxVba.TestEventServerAlt | true | Excel reference-order=OxVba_TestEventServerAlt:{E2A30001-0001-0001-0001-000000000101};OxVba_TestEventServer:{E2A30001-0001-0001-0001-000000000001};OxVba_TestEventServerAlt2:{E2A30001-0001-0001-0001-000000000201}; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_prefers_middle_first_of_three_typelib_references_for_unqualified_testeventserver -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_order_oracle_20260327T060926Z\CCT-043-TES-ORDER3-002.log.txt |
| CCT-043 | CCT-043-TES-ORDER3-003 | ok: 126 | ok: compile-selected-progid=OxVba.TestEventServerAlt2 | true | Excel reference-order=OxVba_TestEventServerAlt2:{E2A30001-0001-0001-0001-000000000201};OxVba_TestEventServer:{E2A30001-0001-0001-0001-000000000001};OxVba_TestEventServerAlt:{E2A30001-0001-0001-0001-000000000101}; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_prefers_third_variant_when_first_of_three_typelib_references_for_unqualified_testeventserver -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_order_oracle_20260327T060926Z\CCT-043-TES-ORDER3-003.log.txt |

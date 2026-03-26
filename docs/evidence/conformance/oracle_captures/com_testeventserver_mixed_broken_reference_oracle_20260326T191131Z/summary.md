# COM TestEventServer Mixed Broken Reference Oracle Run

- Run ID: 20260326T191131Z
- Generated UTC: 2026-03-26T19:17:29Z
- Base TypeLib: C:\Work\DnaCalc\OxVba\tools\OxVba.TestEventServer\bin\Debug\net48\OxVba.TestEventServer.tlb
- Alt TypeLib: C:\Work\DnaCalc\OxVba\temp\generated\com_testeventserver_reference_order\20260326T171500Z\OxVba.TestEventServerAlt\bin\Debug\net48\OxVba.TestEventServerAlt.tlb
- Probe timeout seconds: 15
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_mixed_broken_reference_oracle_20260326T191131Z\results.csv
- Modal inspection note: timeout after successful reopen is treated as likely blocked/modal Excel behavior; the runner records the last captured stage and reference state before forcing cleanup.

- Total cases: 2
- Match count: 0
- Mismatch count: 2

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-043 | CCT-043-TES-MIXED-001 | timeout: execution-did-not-return-within-15s | ok: 84 | false | Excel stage=reopened; refs=name=;guid={E2A30001-0001-0001-0001-000000000001};broken=True|name=OxVba_TestEventServerAlt;guid={E2A30001-0001-0001-0001-000000000101};broken=False; modal_observed=possible; probe_exit_code=; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_mixed_broken_base_then_valid_alt_executes_alt -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_mixed_broken_reference_oracle_20260326T191131Z\CCT-043-TES-MIXED-001.log.txt |
| CCT-043 | CCT-043-TES-MIXED-002 | timeout: execution-did-not-return-within-15s | ok: 42 | false | Excel stage=reopened; refs=name=;guid={E2A30001-0001-0001-0001-000000000101};broken=True|name=OxVba_TestEventServer;guid={E2A30001-0001-0001-0001-000000000001};broken=False; modal_observed=possible; probe_exit_code=; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_mixed_broken_alt_then_valid_base_executes_base -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_mixed_broken_reference_oracle_20260326T191131Z\CCT-043-TES-MIXED-002.log.txt |

# COM TestEventServer Three-Reference Unqualified Broken-Later Oracle Run

- Run ID: 20260327T063542Z
- Generated UTC: 2026-03-27T06:37:19Z
- Base TypeLib: C:\Work\DnaCalc\OxVba\tools\OxVba.TestEventServer\bin\Debug\net48\OxVba.TestEventServer.tlb
- Alt TypeLib: C:\Work\DnaCalc\OxVba\temp\generated\com_testeventserver_three_reference_unqualified_broken_later\20260327T063542Z\OxVba.TestEventServerAlt\bin\Debug\net48\OxVba.TestEventServerAlt.tlb
- Alt2 TypeLib: C:\Work\DnaCalc\OxVba\temp\generated\com_testeventserver_three_reference_unqualified_broken_later\20260327T063542Z\OxVba.TestEventServerAlt2\bin\Debug\net48\OxVba.TestEventServerAlt2.tlb
- Probe timeout seconds: 15
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_unqualified_broken_later_oracle_20260327T063542Z\results.csv
- Excel popup handling note: this runner uses a harness-side VBE dialog helper to keep hidden Excel automation bounded. Popup handling is treated as automation hygiene and coarse failure classification, not user-facing parity.

- Total cases: 2
- Match count: 2
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-043 | CCT-043-TES-BROKEN-LATER3-001 | ok: 42 | ok: compile-selected-progid=OxVba.TestEventServer | true | Excel stage=completed; refs=name=OxVba_TestEventServer;guid={E2A30001-0001-0001-0001-000000000001};broken=False|name=;guid={E2A30001-0001-0001-0001-000000000101};broken=True|name=OxVba_TestEventServerAlt2;guid={E2A30001-0001-0001-0001-000000000201};broken=False; modal_observed=false; handler_signal=; handler_log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_unqualified_broken_later_oracle_20260327T063542Z\CCT-043-TES-BROKEN-LATER3-001.vba-dialog-handler.log; probe_exit_code=0; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_valid_base_then_broken_alt_then_valid_alt2_prefers_base_for_unqualified_testeventserver -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_unqualified_broken_later_oracle_20260327T063542Z\CCT-043-TES-BROKEN-LATER3-001.log.txt |
| CCT-043 | CCT-043-TES-BROKEN-LATER3-002 | ok: 126 | ok: compile-selected-progid=OxVba.TestEventServerAlt2 | true | Excel stage=completed; refs=name=OxVba_TestEventServerAlt2;guid={E2A30001-0001-0001-0001-000000000201};broken=False|name=;guid={E2A30001-0001-0001-0001-000000000001};broken=True|name=OxVba_TestEventServerAlt;guid={E2A30001-0001-0001-0001-000000000101};broken=False; modal_observed=false; handler_signal=; handler_log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_unqualified_broken_later_oracle_20260327T063542Z\CCT-043-TES-BROKEN-LATER3-002.vba-dialog-handler.log; probe_exit_code=0; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_valid_alt2_then_broken_base_then_valid_alt_prefers_alt2_for_unqualified_testeventserver -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_unqualified_broken_later_oracle_20260327T063542Z\CCT-043-TES-BROKEN-LATER3-002.log.txt |

# COM TestEventServer Three-Reference Qualified Broken-First Oracle Run

- Run ID: 20260327T064416Z
- Generated UTC: 2026-03-27T06:52:17Z
- Base TypeLib: C:\Work\DnaCalc\OxVba\tools\OxVba.TestEventServer\bin\Debug\net48\OxVba.TestEventServer.tlb
- Alt TypeLib: C:\Work\DnaCalc\OxVba\temp\generated\com_testeventserver_three_reference_qualified_broken_first\20260327T064416Z\OxVba.TestEventServerAlt\bin\Debug\net48\OxVba.TestEventServerAlt.tlb
- Alt2 TypeLib: C:\Work\DnaCalc\OxVba\temp\generated\com_testeventserver_three_reference_qualified_broken_first\20260327T064416Z\OxVba.TestEventServerAlt2\bin\Debug\net48\OxVba.TestEventServerAlt2.tlb
- Probe timeout seconds: 15
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_qualified_broken_first_oracle_20260327T064416Z\results.csv
- Excel popup handling note: this runner uses a harness-side VBE dialog helper to keep hidden Excel automation bounded. Popup handling is treated as automation hygiene and coarse failure classification, not user-facing parity.

- Total cases: 2
- Match count: 0
- Mismatch count: 2

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-043 | CCT-043-TES-BROKEN-FIRST-QUAL3-001 | error: ui-blocked-or-compile-failure | ok: compile-selected-progid=OxVba.TestEventServerAlt2 | false | Excel stage=reopened; refs=name=;guid={E2A30001-0001-0001-0001-000000000001};broken=True|name=OxVba_TestEventServerAlt;guid={E2A30001-0001-0001-0001-000000000101};broken=False|name=OxVba_TestEventServerAlt2;guid={E2A30001-0001-0001-0001-000000000201};broken=False; modal_observed=true; handler_signal=ui-blocked-or-compile-failure; handler_log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_qualified_broken_first_oracle_20260327T064416Z\CCT-043-TES-BROKEN-FIRST-QUAL3-001.vba-dialog-handler.log; probe_exit_code=; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_broken_base_then_valid_alt_then_valid_alt2_qualified_target_resolves_alt2_binding -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_qualified_broken_first_oracle_20260327T064416Z\CCT-043-TES-BROKEN-FIRST-QUAL3-001.log.txt |
| CCT-043 | CCT-043-TES-BROKEN-FIRST-QUAL3-002 | error: ui-blocked-or-compile-failure | ok: compile-selected-progid=OxVba.TestEventServerAlt | false | Excel stage=reopened; refs=name=;guid={E2A30001-0001-0001-0001-000000000201};broken=True|name=OxVba_TestEventServer;guid={E2A30001-0001-0001-0001-000000000001};broken=False|name=OxVba_TestEventServerAlt;guid={E2A30001-0001-0001-0001-000000000101};broken=False; modal_observed=true; handler_signal=ui-blocked-or-compile-failure; handler_log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_qualified_broken_first_oracle_20260327T064416Z\CCT-043-TES-BROKEN-FIRST-QUAL3-002.vba-dialog-handler.log; probe_exit_code=; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_broken_alt2_then_valid_base_then_valid_alt_qualified_target_resolves_alt_binding -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_three_reference_qualified_broken_first_oracle_20260327T064416Z\CCT-043-TES-BROKEN-FIRST-QUAL3-002.log.txt |

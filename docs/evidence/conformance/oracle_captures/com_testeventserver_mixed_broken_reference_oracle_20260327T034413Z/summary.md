# COM TestEventServer Mixed Broken Reference Oracle Run

- Run ID: 20260327T034413Z
- Generated UTC: 2026-03-27T03:47:15Z
- Base TypeLib: C:\Work\DnaCalc\OxVba\tools\OxVba.TestEventServer\bin\Debug\net48\OxVba.TestEventServer.tlb
- Alt TypeLib: C:\Work\DnaCalc\OxVba\temp\generated\com_testeventserver_reference_order\20260326T171500Z\OxVba.TestEventServerAlt\bin\Debug\net48\OxVba.TestEventServerAlt.tlb
- Probe timeout seconds: 15
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_mixed_broken_reference_oracle_20260327T034413Z\results.csv
- Excel popup handling note: this runner uses a harness-side VBE dialog helper to keep hidden Excel automation bounded. Popup handling is treated as automation hygiene and coarse failure classification, not user-facing parity.

- Total cases: 2
- Match count: 2
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-043 | CCT-043-TES-MIXED-001 | error: ui-blocked-or-compile-failure | ok: PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED | true | Excel stage=reopened; refs=name=;guid={E2A30001-0001-0001-0001-000000000001};broken=True|name=OxVba_TestEventServerAlt;guid={E2A30001-0001-0001-0001-000000000101};broken=False; modal_observed=true; window_titles=; handler_signal=ui-blocked-or-compile-failure; handler_log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_mixed_broken_reference_oracle_20260327T034413Z\CCT-043-TES-MIXED-001.vba-dialog-handler.log; probe_exit_code=; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_mixed_broken_base_then_valid_alt_reports_unresolved_importlib -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_mixed_broken_reference_oracle_20260327T034413Z\CCT-043-TES-MIXED-001.log.txt |
| CCT-043 | CCT-043-TES-MIXED-002 | error: ui-blocked-or-compile-failure | ok: PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED | true | Excel stage=reopened; refs=name=;guid={E2A30001-0001-0001-0001-000000000101};broken=True|name=OxVba_TestEventServer;guid={E2A30001-0001-0001-0001-000000000001};broken=False; modal_observed=true; window_titles=Microsoft Visual Basic for Applications - [MainModule (Code)]; handler_signal=ui-blocked-or-compile-failure; handler_log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_mixed_broken_reference_oracle_20260327T034413Z\CCT-043-TES-MIXED-002.vba-dialog-handler.log; probe_exit_code=; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_mixed_broken_alt_then_valid_base_reports_unresolved_importlib -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_mixed_broken_reference_oracle_20260327T034413Z\CCT-043-TES-MIXED-002.log.txt |

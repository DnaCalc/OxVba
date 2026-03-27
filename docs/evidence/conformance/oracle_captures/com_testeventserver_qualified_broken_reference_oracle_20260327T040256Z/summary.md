# COM TestEventServer Qualified Broken Reference Oracle Run

- Run ID: 20260327T040256Z
- Generated UTC: 2026-03-27T04:57:11Z
- Base TypeLib: C:\Work\DnaCalc\OxVba\tools\OxVba.TestEventServer\bin\Debug\net48\OxVba.TestEventServer.tlb
- Alt TypeLib: C:\Work\DnaCalc\OxVba\temp\generated\com_testeventserver_qualified_broken_reference\20260327T040256Z\OxVba.TestEventServerAlt\bin\Debug\net48\OxVba.TestEventServerAlt.tlb
- Probe timeout seconds: 15
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_qualified_broken_reference_oracle_20260327T040256Z\results.csv
- Excel popup handling note: this runner uses a harness-side VBE dialog helper to keep hidden Excel automation bounded. Popup handling is treated as automation hygiene and coarse failure classification, not user-facing parity.

- Total cases: 2
- Match count: 2
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-043 | CCT-043-TES-BROKEN-QUAL-001 | ok: 42 | ok: compile-selected-progid=OxVba.TestEventServer | true | Excel stage=completed; refs=name=OxVba_TestEventServer;guid={E2A30001-0001-0001-0001-000000000001};broken=False|name=;guid={E2A30001-0001-0001-0001-000000000101};broken=True; modal_observed=false; window_titles=; handler_signal=; handler_log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_qualified_broken_reference_oracle_20260327T040256Z\CCT-043-TES-BROKEN-QUAL-001.vba-dialog-handler.log; probe_exit_code=0; qualified_type=OxVba_TestEventServer.TestEventServer; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_valid_base_then_broken_alt_resolves_qualified_base_binding -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_qualified_broken_reference_oracle_20260327T040256Z\CCT-043-TES-BROKEN-QUAL-001.log.txt |
| CCT-043 | CCT-043-TES-BROKEN-QUAL-002 | ok: 84 | ok: compile-selected-progid=OxVba.TestEventServerAlt | true | Excel stage=completed; refs=name=OxVba_TestEventServerAlt;guid={E2A30001-0001-0001-0001-000000000101};broken=False|name=;guid={E2A30001-0001-0001-0001-000000000001};broken=True; modal_observed=false; window_titles=; handler_signal=; handler_log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_qualified_broken_reference_oracle_20260327T040256Z\CCT-043-TES-BROKEN-QUAL-002.vba-dialog-handler.log; probe_exit_code=0; qualified_type=OxVba_TestEventServerAlt.TestEventServer; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_valid_alt_then_broken_base_resolves_qualified_alt_binding -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_qualified_broken_reference_oracle_20260327T040256Z\CCT-043-TES-BROKEN-QUAL-002.log.txt |

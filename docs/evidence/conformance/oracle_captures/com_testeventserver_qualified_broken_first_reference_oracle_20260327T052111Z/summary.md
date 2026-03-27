# COM TestEventServer Qualified Broken-First Reference Oracle Run

- Run ID: 20260327T052111Z
- Generated UTC: 2026-03-27T05:46:49Z
- Base TypeLib: C:\Work\DnaCalc\OxVba\tools\OxVba.TestEventServer\bin\Debug\net48\OxVba.TestEventServer.tlb
- Alt TypeLib: C:\Work\DnaCalc\OxVba\temp\generated\com_testeventserver_qualified_broken_first_reference\20260327T052111Z\OxVba.TestEventServerAlt\bin\Debug\net48\OxVba.TestEventServerAlt.tlb
- Probe timeout seconds: 15
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_qualified_broken_first_reference_oracle_20260327T052111Z\results.csv
- Excel popup handling note: this runner uses a harness-side VBE dialog helper to keep hidden Excel automation bounded. Popup handling is treated as automation hygiene and coarse failure classification, not user-facing parity.

- Total cases: 2
- Match count: 2
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-043 | CCT-043-TES-BROKEN-FIRST-QUAL-001 | ok: 84 | ok: compile-selected-progid=OxVba.TestEventServerAlt | true | Excel stage=completed; refs=name=;guid={E2A30001-0001-0001-0001-000000000001};broken=True|name=OxVba_TestEventServerAlt;guid={E2A30001-0001-0001-0001-000000000101};broken=False; modal_observed=false; window_titles=; handler_signal=; handler_log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_qualified_broken_first_reference_oracle_20260327T052111Z\CCT-043-TES-BROKEN-FIRST-QUAL-001.vba-dialog-handler.log; probe_exit_code=0; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_broken_base_then_valid_alt_qualified_target_resolves_alt_binding -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_qualified_broken_first_reference_oracle_20260327T052111Z\CCT-043-TES-BROKEN-FIRST-QUAL-001.log.txt |
| CCT-043 | CCT-043-TES-BROKEN-FIRST-QUAL-002 | ok: 42 | ok: compile-selected-progid=OxVba.TestEventServer | true | Excel stage=completed; refs=name=;guid={E2A30001-0001-0001-0001-000000000101};broken=True|name=OxVba_TestEventServer;guid={E2A30001-0001-0001-0001-000000000001};broken=False; modal_observed=false; window_titles=; handler_signal=; handler_log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_qualified_broken_first_reference_oracle_20260327T052111Z\CCT-043-TES-BROKEN-FIRST-QUAL-002.vba-dialog-handler.log; probe_exit_code=0; OxVba anchor command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_loaded_basproj_broken_alt_then_valid_base_qualified_target_resolves_base_binding -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_qualified_broken_first_reference_oracle_20260327T052111Z\CCT-043-TES-BROKEN-FIRST-QUAL-002.log.txt |

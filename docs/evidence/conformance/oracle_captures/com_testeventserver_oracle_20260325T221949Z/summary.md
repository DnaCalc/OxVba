# COM TestEventServer Oracle Run

- Run ID: 20260325T221949Z
- Generated UTC: 2026-03-25T22:19:55Z
- Probe CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_oracle_20260325T221949Z\probe\com_testeventserver_typelib_probe_20260325T221949Z\results.csv
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_oracle_20260325T221949Z\results.csv
- Total cases: 2
- Match count: 2
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-027 | CCT-027-TES-001 | ok: 42 | ok: 42 | true | OxVba anchor: com_early_project_end_to_end::early_bound_project_executes_registered_testeventserver_ping; command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_executes_registered_testeventserver_ping -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_oracle_20260325T221949Z\CCT-027-TES-001.log.txt |
| CCT-027 | CCT-027-TES-002 | ok: 7 | ok: 7 | true | OxVba anchor: com_early_project_end_to_end::early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload (encodes payload 7 as runtime error 7007); command=cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_oracle_20260325T221949Z\CCT-027-TES-002.log.txt |

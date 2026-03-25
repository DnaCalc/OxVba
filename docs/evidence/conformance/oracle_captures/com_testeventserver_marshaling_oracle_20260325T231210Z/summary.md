# COM TestEventServer Marshaling Oracle Run

- Run ID: 20260325T231210Z
- Generated UTC: 2026-03-25T23:35:27Z
- Registration path: HKCU current-user reg import
- Results CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_marshaling_oracle_20260325T231210Z\results.csv
- Total cases: 5
- Match count: 5
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-026 | CCT-026-TES-001 | ok: 17 | ok: 17 | true | OxVba anchor: com_client_registered_lane::windows_registered_com_lane::registered_testeventserver_scalar_sum_pair_supported_subset; command=cargo test -p oxvba-host --test com_client_registered_lane windows_registered_com_lane::registered_testeventserver_scalar_sum_pair_supported_subset -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_marshaling_oracle_20260325T231210Z\CCT-026-TES-001.log.txt |
| CCT-026 | CCT-026-TES-002 | ok: rank=1;len=3;lb=0;ub=2;first=1 | ok: rank=1;len=3;lb=0;ub=2;first=1 | true | OxVba anchor: com_client_registered_lane::windows_registered_com_lane::registered_testeventserver_array_argument_supported_subset; command=cargo test -p oxvba-host --test com_client_registered_lane windows_registered_com_lane::registered_testeventserver_array_argument_supported_subset -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_marshaling_oracle_20260325T231210Z\CCT-026-TES-002.log.txt |
| CCT-026 | CCT-026-TES-003 | ok: True | ok: True | true | OxVba anchor: com_client_registered_lane::windows_registered_com_lane::registered_testeventserver_object_argument_supported_subset; command=cargo test -p oxvba-host --test com_client_registered_lane windows_registered_com_lane::registered_testeventserver_object_argument_supported_subset -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_marshaling_oracle_20260325T231210Z\CCT-026-TES-003.log.txt |
| CCT-026 | CCT-026-TES-004 | ok: 3,4 | ok: 3,4 | true | OxVba anchor: com_client_registered_lane::windows_registered_com_lane::registered_testeventserver_scalar_array_return_supported_subset; command=cargo test -p oxvba-host --test com_client_registered_lane windows_registered_com_lane::registered_testeventserver_scalar_array_return_supported_subset -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_marshaling_oracle_20260325T231210Z\CCT-026-TES-004.log.txt |
| CCT-026 | CCT-026-TES-005 | ok: 42 | ok: 42 | true | OxVba anchor: com_client_registered_lane::windows_registered_com_lane::registered_testeventserver_dispatch_array_return_supported_subset; command=cargo test -p oxvba-host --test com_client_registered_lane windows_registered_com_lane::registered_testeventserver_dispatch_array_return_supported_subset -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\com_testeventserver_marshaling_oracle_20260325T231210Z\CCT-026-TES-005.log.txt |

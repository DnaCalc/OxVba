# Host-Sensitive Oracle Run

- Run ID: 20260326T074730Z
- Generated UTC: 2026-03-26T07:47:36Z
- Excel version: 16.0
- Environment variable: OXVBA_ORACLE_ENV=oracle-033-value
- File probe path: C:\Work\DnaCalc\OxVba\temp\odg033-oracle-test\probe-file.txt
- Results CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\host_sensitive_oracle_20260326T074730Z\results.csv
- Total cases: 3
- Match count: 3
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-035 | CCT-035-ENV-001 | ok: oracle-033-value | ok: oracle-033-value | true | OxVba anchor: host_sensitive_oracle_lane::windows_host_sensitive_oracle_lane::windows_host_backed_environ_string_returns_actual_value; command=cargo test -p oxvba-host --test host_sensitive_oracle_lane windows_host_sensitive_oracle_lane::windows_host_backed_environ_string_returns_actual_value -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\host_sensitive_oracle_20260326T074730Z\CCT-035-ENV-001.log.txt |
| CCT-035 | CCT-035-DIR-001 | ok: probe-file.txt | ok: probe-file.txt | true | OxVba anchor: host_sensitive_oracle_lane::windows_host_sensitive_oracle_lane::windows_host_backed_dir_existing_file_returns_filename; command=cargo test -p oxvba-host --test host_sensitive_oracle_lane windows_host_sensitive_oracle_lane::windows_host_backed_dir_existing_file_returns_filename -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\host_sensitive_oracle_20260326T074730Z\CCT-035-DIR-001.log.txt |
| CCT-035 | CCT-035-SHELL-001 | ok: pid>0 | ok: pid>0 | true | OxVba anchor: host_sensitive_oracle_lane::windows_host_sensitive_oracle_lane::windows_host_backed_shell_returns_positive_process_identifier; command=cargo test -p oxvba-host --test host_sensitive_oracle_lane windows_host_sensitive_oracle_lane::windows_host_backed_shell_returns_positive_process_identifier -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\host_sensitive_oracle_20260326T074730Z\CCT-035-SHELL-001.log.txt |

# File I/O Oracle Run

- Run ID: 20260326T075929Z
- Generated UTC: 2026-03-26T07:59:33Z
- Excel version: 16.0
- File probe path: C:\Work\DnaCalc\OxVba\temp\file-io-oracle-test\roundtrip.txt
- Results CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\file_io_oracle_20260326T075929Z\results.csv
- Total cases: 1
- Match count: 1
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-033 | CCT-033-LINE-001 | ok: world | ok: world | true | OxVba anchor: file_io_host_backed_end_to_end::windows_file_io_host_backed_end_to_end::host_backed_file_print_line_input_roundtrip_returns_written_line; command=cargo test -p oxvba-host --test file_io_host_backed_end_to_end windows_file_io_host_backed_end_to_end::host_backed_file_print_line_input_roundtrip_returns_written_line -- --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\file_io_oracle_20260326T075929Z\CCT-033-LINE-001.log.txt |

# File I/O Oracle Run

- Run ID: 20260326T220420Z
- Generated UTC: 2026-03-26T20:04:55Z
- Excel version: 16.0
- File probe path: C:\Work\DnaCalc\OxVba\temp\file-io-oracle-test\roundtrip.txt
- Results CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\file_io_oracle_20260326T220420Z\results.csv
- Total cases: 4
- Match count: 4
- Mismatch count: 0

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-033 | CCT-033-LINE-001 | ok: world | ok: world | true | OxVba anchor: file_io_host_backed_end_to_end::windows_file_io_host_backed_end_to_end::host_backed_file_print_line_input_roundtrip_returns_written_line; command=cargo test -p oxvba-host --test file_io_host_backed_end_to_end windows_file_io_host_backed_end_to_end::host_backed_file_print_line_input_roundtrip_returns_written_line -- --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\file_io_oracle_20260326T220420Z\CCT-033-LINE-001.log.txt |
| CCT-033 | CCT-033-FILEPOS-001 | ok: False|7|1|world|True|8 | ok: False|7|1|world|True|8 | true | OxVba anchor: file_io_host_backed_end_to_end::windows_file_io_host_backed_end_to_end::host_backed_file_eof_lof_seek_matches_excel_shape; command=cargo test -p oxvba-host --test file_io_host_backed_end_to_end windows_file_io_host_backed_end_to_end::host_backed_file_eof_lof_seek_matches_excel_shape -- --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\file_io_oracle_20260326T220420Z\CCT-033-FILEPOS-001.log.txt |
| CCT-033 | CCT-033-WRITE-001 | ok: hello,world | ok: hello,world | true | OxVba anchor: file_io_host_backed_end_to_end::windows_file_io_host_backed_end_to_end::host_backed_file_write_input_preserves_embedded_comma_string; command=cargo test -p oxvba-host --test file_io_host_backed_end_to_end windows_file_io_host_backed_end_to_end::host_backed_file_write_input_preserves_embedded_comma_string -- --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\file_io_oracle_20260326T220420Z\CCT-033-WRITE-001.log.txt |
| CCT-033 | CCT-033-WRITE-002 | ok: 42|True|hello,world | ok: 42|True|hello,world | true | OxVba anchor: file_io_host_backed_end_to_end::windows_file_io_host_backed_end_to_end::host_backed_file_write_input_multi_field_typed_roundtrip_matches_excel_shape; command=cargo test -p oxvba-host --test file_io_host_backed_end_to_end windows_file_io_host_backed_end_to_end::host_backed_file_write_input_multi_field_typed_roundtrip_matches_excel_shape -- --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\file_io_oracle_20260326T220420Z\CCT-033-WRITE-002.log.txt |

# VBA Attribute Oracle Run

- Run ID: 20260329T233125Z
- Generated UTC: 2026-03-29T23:35:48Z
- Excel version: 16.0
- Results CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\vba_attribute_oracle_20260329T233125Z\results.csv
- Total cases: 2
- Match count: 0
- Mismatch count: 2

## Case Results
| Topic | Case | VBA | OxVba | Roundtrip | Match | Notes |
|---|---|---|---|---|---|---|
| CCT-049 | CCT-049-DEFAULTPROP-001 | ok: ERR|438|Object doesn't support this property or method | ok: 42 | dropped: <none> | false | OxVba anchor: vba_attribute_oracle_lane::windows_vba_attribute_oracle_lane::windows_defaultprop_vb_usermemid_zero_bare_assignment_matches_excel; command=cargo test -p oxvba-host --test vba_attribute_oracle_lane windows_vba_attribute_oracle_lane::windows_defaultprop_vb_usermemid_zero_bare_assignment_matches_excel -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\vba_attribute_oracle_20260329T233125Z\CCT-049-DEFAULTPROP-001.log.txt; Excel import/export roundtrip: dropped; summary=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\vba_attribute_oracle_20260329T233125Z\excel_import_CCT-049-DEFAULTPROP-001\roundtrip_summary.md |
| CCT-050 | CCT-050-NEWENUM-001 | ok: ERR|438|Object doesn't support this property or method | ok: 41,42, | dropped: <none> | false | OxVba anchor: vba_attribute_oracle_lane::windows_vba_attribute_oracle_lane::windows_newenum_vb_usermemid_minus4_for_each_matches_excel; command=cargo test -p oxvba-host --test vba_attribute_oracle_lane windows_vba_attribute_oracle_lane::windows_newenum_vb_usermemid_minus4_for_each_matches_excel -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\vba_attribute_oracle_20260329T233125Z\CCT-050-NEWENUM-001.log.txt; Excel import/export roundtrip: dropped; summary=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\vba_attribute_oracle_20260329T233125Z\excel_import_CCT-050-NEWENUM-001\roundtrip_summary.md |

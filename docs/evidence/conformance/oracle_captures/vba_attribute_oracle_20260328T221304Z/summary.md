# VBA Attribute Oracle Run

- Run ID: 20260328T221304Z
- Generated UTC: 2026-03-28T22:38:18Z
- Excel version: 16.0
- Results CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\vba_attribute_oracle_20260328T221304Z\results.csv
- Total cases: 2
- Match count: 0
- Mismatch count: 2

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-049 | CCT-049-DEFAULTPROP-001 | ok: ERR|438|Object doesn't support this property or method | error: lane-failed(exit=101) | false | OxVba anchor: vba_attribute_oracle_lane::windows_vba_attribute_oracle_lane::windows_defaultprop_vb_usermemid_zero_bare_assignment_matches_excel; command=cargo test -p oxvba-host --test vba_attribute_oracle_lane windows_vba_attribute_oracle_lane::windows_defaultprop_vb_usermemid_zero_bare_assignment_matches_excel -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\vba_attribute_oracle_20260328T221304Z\CCT-049-DEFAULTPROP-001.log.txt |
| CCT-050 | CCT-050-NEWENUM-001 | ok: ERR|438|Object doesn't support this property or method | error: error:compile-time diagnostic: PMR-E-BACKEND-COMPILE: type error: unsupported statement: For Each item In widget | false | OxVba anchor: vba_attribute_oracle_lane::windows_vba_attribute_oracle_lane::windows_newenum_vb_usermemid_minus4_for_each_matches_excel; command=cargo test -p oxvba-host --test vba_attribute_oracle_lane windows_vba_attribute_oracle_lane::windows_newenum_vb_usermemid_minus4_for_each_matches_excel -- --ignored --exact --test-threads=1 --nocapture; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\vba_attribute_oracle_20260328T221304Z\CCT-050-NEWENUM-001.log.txt |

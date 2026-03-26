# Host Extension Oracle Run

- Run ID: 20260326T144800Z
- Generated UTC: 2026-03-26T14:46:01Z
- Results CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\host_extension_oracle_20260326T144800Z\results.csv
- Output directory: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\host_extension_oracle_20260326T144800Z

## Case Results
| Topic | Case | VBA | OxVba | Match | Notes |
|---|---|---|---|---|---|
| CCT-042 | CCT-042-THISWORKBOOK-001 | ok: Public Sub Sync()
End Sub | ok: Public Sub Sync()
End Sub | true | OxVba anchor: host_project_excel_vbide_callbacks::excel_vbide_host_callbacks_attach_source_to_thisworkbook; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\host_extension_oracle_20260326T144800Z\cct_042_thisworkbook_001.oxvba_test.log.txt; bounded supported host-extension attach on ThisWorkbook |
| CCT-042 | CCT-042-MISSING-TARGET-001 | error: Subscript out of range | error: AdapterFault [HAL-E-ADAPTER-FAULT] profile=Windows capability=ProjectMutation op=attach_host_extension_module: OperationStopped: 
Line |
  26 |              $component = $wb.VBProject.VBComponents.Item($ModuleName)
     |              ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
     | Subscript out of range | true | OxVba anchor: host_project_excel_vbide_callbacks::excel_vbide_host_callbacks_missing_target_reports_error; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\host_extension_oracle_20260326T144800Z\cct_042_missing_target_001.oxvba_test.log.txt; error parity is status-normalized because Excel/VBIDE error strings are host-specific |
| CCT-042 | CCT-042-THISWORKBOOK-OVERWRITE-001 | ok: Public Sub AfterSync()
End Sub | ok: Public Sub AfterSync()
End Sub | true | OxVba anchor: host_project_excel_vbide_callbacks::excel_vbide_host_callbacks_replace_existing_thisworkbook_source; log=C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\host_extension_oracle_20260326T144800Z\cct_042_thisworkbook_overwrite_001.oxvba_test.log.txt; bounded overwrite-on-occupied-target behavior |

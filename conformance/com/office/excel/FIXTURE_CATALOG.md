# Excel COM Fixture Catalog

| Row | Fixture | Purpose | V0.2 classification |
| --- | --- | --- | --- |
| `OFFICE-COM-007` | `excel_application_activation_smoke.bas` | `Excel.Application` activation, root property get, and explicit Quit cleanup shape. | environment-dependent |
| `OFFICE-COM-008` | `excel_workbook_range_smoke.bas` | Workbook collection, workbook creation, worksheet indexing, native `Range("A1")` object access, close, and cleanup; range value/default-member mutation remains open. | environment-dependent |
| `OFFICE-COM-009` | `excel_named_argument_smoke.bas` | Named-argument dispatch shape for metadata-backed Excel `Worksheets.Add After:=sheet` calls. | environment-dependent |
| `OFFICE-COM-010` | `excel_unsupported_event_sink_boundary.bas` | Explicit unsupported V0.2 boundary for real Excel application event sinks. | unsupported-v02 |
| `OFFICE-COM-016` | `excel_dispatchinvoke_range_smoke.bas` | Explicit intrinsic `DispatchInvoke(..., "member", ...)` workbook, worksheet, and `Range("A1")` object access; range value/default-member mutation remains open. | environment-dependent |
| `OFFICE-COM-017` | `excel_find_null_result_smoke.bas` | Named-argument Excel `Cells.Find` call whose no-match result is `Nothing`; mutation remains open. | environment-dependent |
| `OFFICE-COM-018` | `excel_range_value_put_smoke.bas` | Late-bound Excel `Range("A1").Value` property-put observed through named-argument `Cells.Find`; default-member mutation remains open. | environment-dependent |

These fixtures are intentionally late-bound so they remain parse/compile
fixtures on machines without Excel. Live evidence is refreshed separately under
`bd-bqm8.7.5`.

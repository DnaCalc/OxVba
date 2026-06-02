# Excel COM Fixture Catalog

| Row | Fixture | Purpose | V0.2 classification |
| --- | --- | --- | --- |
| `OFFICE-COM-007` | `excel_application_activation_smoke.bas` | `Excel.Application` activation, root property get, and explicit Quit cleanup shape. | environment-dependent |
| `OFFICE-COM-008` | `excel_workbook_range_smoke.bas` | Workbook/worksheet/range access, including a default-member shaped range value access. | environment-dependent |
| `OFFICE-COM-009` | `excel_named_argument_smoke.bas` | Named-argument dispatch shape for metadata-backed Excel object-model calls. | environment-dependent |
| `OFFICE-COM-010` | `excel_unsupported_event_sink_boundary.bas` | Explicit unsupported V0.2 boundary for real Excel application event sinks. | unsupported-v02 |

These fixtures are intentionally late-bound so they remain parse/compile
fixtures on machines without Excel. Live evidence is refreshed separately under
`bd-bqm8.7.5`.

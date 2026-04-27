# V0.2 Excel COM Corpus Fixtures

Date: 2026-04-27

Bead: `bd-bqm8.7.3`

## Scope

This bead adds the Excel half of the Office COM corpus fixture pack selected by
`V02_OFFICE_COM_CORPUS_MATRIX_2026-04-27.md`. The fixtures are durable source
artifacts and compiler-accepted active tests. They do not claim that Microsoft
Excel is present in default CI.

## Added Fixtures

| Row | Fixture | Coverage |
| --- | --- | --- |
| `OFFICE-COM-007` | `conformance/com/office/excel/excel_application_activation_smoke.bas` | Late-bound `Excel.Application` activation plus root property get/set shape. |
| `OFFICE-COM-008` | `conformance/com/office/excel/excel_workbook_range_smoke.bas` | Workbook, worksheet, range, value, and default-member shaped interactions routed through `DispatchInvoke`. |
| `OFFICE-COM-009` | `conformance/com/office/excel/excel_named_argument_smoke.bas` | Named-argument dispatch shape for metadata-sensitive Excel object-model calls. |
| `OFFICE-COM-010` | `conformance/com/office/excel/excel_unsupported_event_sink_boundary.bas` | Explicit unsupported V0.2 row for real Excel application event sink hookup. |

Catalog/docs:

- `conformance/com/office/excel/README.md`
- `conformance/com/office/excel/FIXTURE_CATALOG.md`

## Active Test

Added host formal test:

- `formal_v02_7_excel_com_fixture_pack_exists_and_compiles`

The test verifies the Excel fixture catalog files exist and compiles each `.bas`
fixture with `oxvba_compiler::compile`. This keeps syntax/lowering coverage
active on all machines while live Excel execution remains an environment-gated
evidence lane.

## Incidental Compile Fix

The targeted host test build exposed stale `SafeArray::elements` field access in
`crates/oxvba-host/tests/com_client_registered_lane.rs`. That test now calls the
current `elements()` accessor.

## Checks Run

- `cargo test -p oxvba-host formal_v02_7_excel_com_fixture_pack_exists_and_compiles -- --nocapture`

## Result

`bd-bqm8.7.3` is complete for Excel corpus fixture delivery. The Office COM
corpus lane remains in-progress pending Access/JET fixtures, refreshed evidence,
and the final checklist.

# Excel COM Corpus Fixtures

Status: `environment-dependent-fixture-pack` (`v02.7`)

These fixtures define the V0.2 Excel COM corpus rows without making Microsoft
Excel a default CI dependency. They are compiler-accepted fixture sources first;
live execution belongs to the Office-enabled evidence lane.

Current fixtures:

- `excel_application_activation_smoke.bas`: late-bound
  `CreateObject("Excel.Application")` activation, root property-get smoke, and
  explicit `Quit` cleanup.
- `excel_workbook_range_smoke.bas`: workbook collection, workbook creation,
  worksheet indexing, native `Range("A1")` object access, workbook close, and
  explicit application cleanup. Range value/default-member mutation remains a
  separate open oracle lane.
- `excel_dispatchinvoke_range_smoke.bas`: the same workbook/worksheet/range
  object path through explicit `DispatchInvoke(..., "member", ...)` calls,
  including `DispatchInvoke(sheet, "Range", "A1")`. Range value/default-member
  mutation remains a separate open oracle lane.
- `excel_named_argument_smoke.bas`: metadata-sensitive named-argument call
  shape over an Excel object-model member through explicit `Worksheets.Add
  After:=sheet`.
- `excel_find_null_result_smoke.bas`: metadata-sensitive named-argument
  `Cells.Find` call that returns Excel `Nothing` when no cell matches.
- `excel_range_value_put_smoke.bas`: late-bound `Range("A1").Value`
  property-put followed by named-argument `Cells.Find` observation.
- `excel_unsupported_event_sink_boundary.bas`: deterministic V0.2 unsupported
  row for real `Excel.Application` event sinks beyond controlled TestEventServer
  coverage.

Execution rule:

- Default CI validates fixture presence and syntax only.
- Live automation runs must classify absent Excel/VBOM as environment skips.
- Hidden Excel/VBE automation should use `scripts/excel-dialog-guardian.ps1`
  and `scripts/excel-vbe-dialog-handler.ps1`.

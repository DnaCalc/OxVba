# SQLiteForExcel Excel Ground Truth

Date: 2026-04-03
Owner: Codex
Status: captured

This note records the direct Excel automation probes used to distinguish
SQLiteForExcel upstream behavior from OxVba behavior.

## Environment

- Excel automation available: `Excel.Application` version `16.0`
- Upstream workbook used:
  - `C:\Work\SqliteForExcel\Distribution\SQLiteForExcel_64.xlsm`
- Upstream raw source files used:
  - `C:\Work\SqliteForExcel\Source\SQLite3VBAModules\Sqlite3_64.bas`
  - `C:\Work\SqliteForExcel\Source\SQLite3VBAModules\Sqlite3Demo_64.bas`

## Probe 1: Shipped Workbook As-Is

Probe script:
- [run-sqliteforexcel-excel-probe.ps1](/C:/Work/DnaCalc/OxVba/scripts/run-sqliteforexcel-excel-probe.ps1)

Result:
- workbook opens successfully
- staged wrapper logs:

```text
START
CALL SQLite3Initialize
INIT_RETURN=0
CALL TestVersion
DONE TestVersion
CALL TestOpenClose
RUN_FAIL
53
File not found
```

Interpretation:
- the shipped workbook compiles and runs in Excel
- SQLite initialization succeeds in Excel from the shipped workbook
- at least one real SQLite call path executes before failure
- the first observed runtime failure in this automation pass is during or after
  `TestOpenClose`, surfacing as VBA error `53` (`File not found`)

This is sufficient to establish that OxVba's current compile-time failures on
the normalized probes are genuine differences from Excel behavior.

## Probe 2: Raw Source Import Into Fresh Workbook

Probe script:
- [run-sqliteforexcel-excel-import-probe.ps1](/C:/Work/DnaCalc/OxVba/scripts/run-sqliteforexcel-excel-import-probe.ps1)

Result:
- Excel imports raw upstream source files with `_64` filenames successfully
- staged wrapper logs:

```text
START
CALL SQLite3Initialize
INIT_RETURN=1
INIT_FAIL_ERR=126
```

Interpretation:
- Excel accepts the raw imported files
  - `Sqlite3_64.bas`
  - `Sqlite3Demo_64.bas`
- therefore OxVba's current hard rejection on filename vs `VB_Name`
  (`PMR-E-MODULE-HEADER-VB-NAME`) is a real ingestion-policy difference from
  Excel, not just a fixture artifact
- this import probe stops at DLL initialization because the fresh workbook has
  no meaningful `ThisWorkbook.Path` pointing at the upstream `Distribution\x64`
  location

## Confirmed Excel-vs-OxVba Differences

Confirmed by these probes:

1. Raw-source filename mismatch acceptance
- Excel imports raw `*_64.bas` files whose internal `VB_Name` values are
  `Sqlite3` / `Sqlite3Demo`
- OxVba previously rejected that source shape during project import; this is now
  fixed under the `VB_Name`-wins external-identity rule

2. Normalized SQLite compile-time failures are OxVba-side
- Excel compiles and starts running the shipped workbook
- OxVba has already moved past earlier normalized-probe failures on
  `ThisWorkbook.Path` and `sqlite3open`
- the current normalized-probe boundary is later:
  - `unsupported statement: ReDim with runtime expression bounds is not yet supported for array 'buf': buf(length - 1)`

## Related Follow-On Beads

- `bd-sql1.8.5` `VB_Name`-wins external source identity with stable file-path writes
- `bd-sql1.16` runtime-sized `ReDim` support for the SQLite UTF-8 helper lane

The Excel runtime `File not found` during `TestOpenClose` is not yet classified
as an OxVba delta. It is recorded here as upstream/runtime ground truth only.

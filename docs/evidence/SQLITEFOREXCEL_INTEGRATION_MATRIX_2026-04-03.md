# SQLiteForExcel Integration Matrix

Date: 2026-04-03
Owner: Codex
Status: in-progress

This matrix records the first controlled fixture runs for the SQLiteForExcel
declare integration lane.

## Fixtures

- `HostEnvironmentProbe`
  - proves the tiny host reference project by itself
- `Demo64`
  - raw upstream 64-bit filenames
- `Core64Normalized`
  - normalized core-only 64-bit probe
- `Demo64Normalized`
  - normalized full demo 64-bit probe

## Current Results

| Fixture | Mode | Command shape | Expected | Actual | Classification |
| --- | --- | --- | --- | --- | --- |
| `HostEnvironmentProbe` | default | `oxvba run-project .external\sqliteforexcel\fixtures\HostEnvironmentProbe\HostEnvironmentProbe.basproj` | compile/run cleanly | compile/run cleanly | positive baseline |
| `HostEnvironmentProbe` | `--jit` | `oxvba run-project .external\sqliteforexcel\fixtures\HostEnvironmentProbe\HostEnvironmentProbe.basproj --jit` | compile/run cleanly | JIT panic: `missing helper: oxrt_host_console_print` | unexpected runtime/JIT limitation |
| `Demo64` | default | `oxvba run-project .external\sqliteforexcel\fixtures\Demo64\SQLiteForExcelDemo64.basproj` | likely import failure because raw filenames keep upstream `_64` suffix | `PMR-E-MODULE-HEADER-VB-NAME` on `Sqlite3_64.bas` / `Sqlite3Demo_64.bas` | expected negative / import-shape mismatch |
| `Demo64` | `--jit` | `oxvba run-project .external\sqliteforexcel\fixtures\Demo64\SQLiteForExcelDemo64.basproj --jit` | same import failure before runtime choice matters | same `PMR-E-MODULE-HEADER-VB-NAME` import failure | expected negative / import-shape mismatch |
| `Core64Normalized` | default | `oxvba run-project .external\sqliteforexcel\fixtures\Core64Normalized\SQLiteForExcelCore64Normalized.basproj` | if core compiles, should begin native init/version lane | `PMR-E-BACKEND-COMPILE`: `use of undeclared variable: thisworkbook_path` | unexpected compile-time limitation |
| `Core64Normalized` | `--jit` | `oxvba run-project .external\sqliteforexcel\fixtures\Core64Normalized\SQLiteForExcelCore64Normalized.basproj --jit` | same as default or same compile-time failure | same `PMR-E-BACKEND-COMPILE`: `use of undeclared variable: thisworkbook_path` | unexpected compile-time limitation |
| `Demo64Normalized` | default | `oxvba run-project .external\sqliteforexcel\fixtures\Demo64Normalized\SQLiteForExcelDemo64Normalized.basproj` | if demo compiles, should reach broader SQLite sample lane | `PMR-E-NAME-QUALIFICATION-REQUIRED`: `sqlite3open` declared in multiple modules | unexpected compile-time limitation |
| `Demo64Normalized` | `--jit` | `oxvba run-project .external\sqliteforexcel\fixtures\Demo64Normalized\SQLiteForExcelDemo64Normalized.basproj --jit` | same as default or same compile-time failure | same `PMR-E-NAME-QUALIFICATION-REQUIRED`: `sqlite3open` declared in multiple modules | unexpected compile-time limitation |

## Notes

- The tiny host shim originally used a private class constant inside
  `ThisWorkbook.cls`; that failed with `use of undeclared variable:
  sqliteforexcel_distribution_path` and was reduced to an inline literal.
- After that reduction, the standalone host probe passed, so the current
  blocking failures are beyond the minimal host shim itself.
- The original `--jit` host-only panic on `oxrt_host_debug_print` has now been
  fixed as part of the diagnostics-host ownership follow-on.
- Re-running the same host-only `--jit` row now exposes the next limitation:
  `Print`/console-host lowering reaches the JIT helper table, but
  `oxrt_host_console_print` is still missing.
- Excel-side ground truth is recorded separately in
  [SQLITEFOREXCEL_EXCEL_GROUND_TRUTH_2026-04-03.md](/C:/Work/DnaCalc/OxVba/docs/evidence/SQLITEFOREXCEL_EXCEL_GROUND_TRUTH_2026-04-03.md).

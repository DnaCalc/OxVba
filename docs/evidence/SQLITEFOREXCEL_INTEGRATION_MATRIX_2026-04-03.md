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
| `HostEnvironmentProbe` | `--jit` | `oxvba run-project .external\sqliteforexcel\fixtures\HostEnvironmentProbe\HostEnvironmentProbe.basproj --jit` | compile/run cleanly | compile/run cleanly | positive baseline |
| `Demo64` | default | `oxvba run-project .external\sqliteforexcel\fixtures\Demo64\SQLiteForExcelDemo64.basproj` | if raw `_64` imports now work, should reach the same demo compile boundary as the normalized fixture | `PMR-E-NAME-QUALIFICATION-REQUIRED`: `sqlite3open` declared in multiple modules | compile-time limitation after `VB_Name`-wins ingest fix |
| `Demo64` | `--jit` | `oxvba run-project .external\sqliteforexcel\fixtures\Demo64\SQLiteForExcelDemo64.basproj --jit` | same as default or same compile-time failure | same `PMR-E-NAME-QUALIFICATION-REQUIRED`: `sqlite3open` declared in multiple modules | compile-time limitation after `VB_Name`-wins ingest fix |
| `Core64Normalized` | default | `oxvba run-project .external\sqliteforexcel\fixtures\Core64Normalized\SQLiteForExcelCore64Normalized.basproj` | if core compiles, should begin native init/version lane | `PMR-E-BACKEND-COMPILE`: `call to unknown procedure: strptr` | compile-time limitation after `Exit Function` support fix |
| `Core64Normalized` | `--jit` | `oxvba run-project .external\sqliteforexcel\fixtures\Core64Normalized\SQLiteForExcelCore64Normalized.basproj --jit` | same as default or same compile-time failure | same `PMR-E-BACKEND-COMPILE`: `call to unknown procedure: strptr` | compile-time limitation after `Exit Function` support fix |
| `Demo64Normalized` | default | `oxvba run-project .external\sqliteforexcel\fixtures\Demo64Normalized\SQLiteForExcelDemo64Normalized.basproj` | if demo compiles, should reach broader SQLite sample lane | `PMR-E-NAME-QUALIFICATION-REQUIRED`: `sqlite3open` declared in multiple modules | unexpected compile-time limitation |
| `Demo64Normalized` | `--jit` | `oxvba run-project .external\sqliteforexcel\fixtures\Demo64Normalized\SQLiteForExcelDemo64Normalized.basproj --jit` | same as default or same compile-time failure | same `PMR-E-NAME-QUALIFICATION-REQUIRED`: `sqlite3open` declared in multiple modules | unexpected compile-time limitation |

## Notes

- Raw upstream `_64` filenames are no longer blocked at import because external
  source identity now follows `VB_Name` while file paths remain stable.
- The earlier `thisworkbook_path` failure is fixed. The current compiler and host
  regressions now pin the moved boundary one step later at a statement-shape
  limitation for `StrPtr`.
- The earlier `loadlibrary` boundary is also fixed. The SQLite core fixture now
  moves past `Private Declare` binding and built-in `Win64` conditional selection
  before failing at the later `Exit Function` statement form.
- The later `Debug.Print "...", Err.LastDllError` boundary is also fixed. The
  SQLite core fixture now moves past multi-expression `Debug.Print` lowering in
  both VM and JIT-bound compile paths.
- The later `Exit Function` boundary is also fixed. The SQLite core fixture now
  moves past procedure-level early-exit control flow and stops at a narrower
  built-in/native helper binding frontier for `StrPtr`.
- The `sqlite3open` duplicate-name failure is likewise pinned at the host/basproj
  boundary and direct compiler boundary, and it also reproduces in both compiler
  lowering strategies.
- The earlier `Private Const` shim hypothesis is not currently reproduced by the
  minimal direct compiler probe, so it remains a narrower follow-on question
  rather than a confirmed active blocker.
- The original `--jit` host-only panic on `oxrt_host_debug_print` was fixed as
  part of the diagnostics-host ownership follow-on.
- The next exposed JIT helper-table omission for console-host instructions was
  then fixed as part of the console-host follow-on, so the tiny host shim now
  runs cleanly under both VM and JIT.
- Excel-side ground truth is recorded separately in
  [SQLITEFOREXCEL_EXCEL_GROUND_TRUTH_2026-04-03.md](/C:/Work/DnaCalc/OxVba/docs/evidence/SQLITEFOREXCEL_EXCEL_GROUND_TRUTH_2026-04-03.md).

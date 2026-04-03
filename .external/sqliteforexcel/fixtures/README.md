## SQLiteForExcel Fixtures

This directory contains OxVba-owned fixture projects for the SQLiteForExcel
declare integration lane.

Current fixtures:

- `HostEnvironment/`
  - tiny reusable project-reference shim for host-ish VBA globals used by the
    upstream demo modules
  - currently supplies `ThisWorkbook.Path` and `Debug.Print`
  - `ThisWorkbook.Path` is intentionally hardwired to the controlled SQLite
    distribution path for this first probe, so the shim does not depend on an
    extra mutable helper API
- `Demo64/`
  - raw 64-bit probe project that imports the upstream 64-bit SQLite modules
    under their upstream filenames and calls `Sqlite3Demo.AllTests`
  - current expected first result: OxVba rejects the raw import because
    `Sqlite3_64.bas` / `Sqlite3Demo_64.bas` do not match the internal
    `VB_Name` values `Sqlite3` / `Sqlite3Demo`
- `Demo64Normalized/`
  - adapted 64-bit probe project that uses renamed copies matching `VB_Name`
  so the native-call exploration can continue after the raw import boundary
  is captured
- `Core64Normalized/`
  - adapted 64-bit core-only probe project
  - imports only the normalized SQLite core module plus the host shim
  - intended to separate SQLite core-module compile/load behavior from the
    larger upstream demo module
- `HostEnvironmentProbe/`
  - minimal sanity probe for the tiny host-environment reference project alone
  - intended to prove whether bare `ThisWorkbook.Path` and `Debug.Print`
  resolve across a project-reference boundary before SQLite-specific code is
  involved
- a first broader mutable shim attempt also showed that a direct
  `HostEnv.SetWorkbookPath` call did not resolve across the project-reference
  boundary in the current setup, so the tiny shim was reduced to only the
  required host-ish names

Important constraints:

- The host shim is intentionally minimal. It is a probe fixture, not a claim
  that OxVba already has full Excel host-environment parity.
- `ThisWorkbook.Path` is currently set to the controlled in-repo SQLite
  distribution path using a repo-relative string:
  `.external\sqliteforexcel\upstream\Distribution`
- That means the current CLI-driven probes should be run with the OxVba repo
  root as the working directory.

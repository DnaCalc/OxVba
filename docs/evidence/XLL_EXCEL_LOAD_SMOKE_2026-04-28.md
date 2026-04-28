# XLL Excel Load Smoke

Date: 2026-04-28
Bead: `bd-xll1.5.5`
Workset: `docs/worksets/WORKSET_2026-04-28_XLL_EXCEL_HOST_VALIDATION_EXECUTION.md`

## Scope

Added `scripts/run-xll-excel-load-smoke.ps1`, a bounded Excel COM automation
harness that consumes a staged XLL manifest and records host load evidence under
`target/xll-host-validation/excel-load/`.

The harness captures:

- staged artifact path and size,
- Excel version, build, path, operating system, and PID,
- load method,
- load result,
- dialog guardian log path,
- JSON result artifact.

## First Attempt

Command:

```powershell
./scripts/run-xll-excel-load-smoke.ps1 `
  -StagingManifest target/xll-host-validation/scalar_addin/20260428T000000Z/manifest.json `
  -RunId 20260428T000000Z `
  -AllowUnavailable
```

Result:

```text
Unable to get the Add property of the AddIns class
```

Evidence:

- `target/xll-host-validation/excel-load/20260428T000000Z/excel_load_result.json`

The failure happened through the `Application.AddIns.Add(...)` path before a
usable AddIn object was returned. Excel itself was available.

## Passing Load Path

The harness default was changed to use `Application.RegisterXLL(...)`, while
leaving `AddIns` available as an explicit alternate load method.

Command:

```powershell
./scripts/run-xll-excel-load-smoke.ps1 `
  -StagingManifest target/xll-host-validation/scalar_addin/20260428T000000Z/manifest.json `
  -RunId 20260428T001000Z `
  -AllowUnavailable
```

Result:

```text
xll excel load smoke: registered_and_excel_quit
result: target\xll-host-validation\excel-load\20260428T001000Z\excel_load_result.json
```

Result summary:

- status: `registered_and_excel_quit`
- load method: `RegisterXLL`
- `RegisterXLL` return: `true`
- Excel version: `16.0`
- Excel build: `19929`
- Excel operating system: `Windows (64-bit) NT 10.00`
- Excel path: `C:\Program Files\Microsoft Office\Root\Office16`
- artifact bytes: `2282496`

No lingering `EXCEL.EXE` process remained after cleanup.

## Boundary

This proves a generated OxVba `.xll` can be loaded by Excel through
`Application.RegisterXLL(...)` and that Excel can quit cleanly afterward. It
does not yet prove per-function `xlfRegister` success or worksheet invocation.
Those are the next beads.

# XLL Examples

These examples are for the XLL/Addin lane. They are not part of the basic
language example corpus because they require native wrapper packaging and, for
host validation, Microsoft Excel.

## Scalar Addin

`scalar_addin` is the deterministic fixture for
`WORKSET_2026-04-28_XLL_EXCEL_HOST_VALIDATION_EXECUTION.md`.

It intentionally exports only a small scalar function set:

- `AddDouble`
- `EchoText`
- `NotFlag`
- `IncLong`

The fixture is designed to build through the normal `OutputType=Addin` path and
to be used by the staged Excel-host validation beads.

```powershell
./scripts/stage-xll-scalar-addin.ps1
```

The staging script writes source snapshots, build logs, manifests, and generated
`.xll` artifacts under `target/xll-host-validation/`.

## Array Addin

`array_addin` is the deterministic fixture for bounded XLOPER12 `xltypeMulti`
validation.

It exports:

- `MakeVector`
- `SumArray`

The fixture proves both array return marshaling and array argument
unmarshalling in the generated XLL shim.

```powershell
./scripts/stage-xll-array-addin.ps1
```

## Application Addin

`application_addin` validates the XLL-hosted Excel `Application` injection lane.

It references `excel_host` as a host-injected project, then exports:

- `ExcelVersion`
- `ExcelHwnd`

The exported functions call `Application.Value` and dispatch onto the injected
Excel root object. `ExcelVersion()` is used by the worksheet smoke harness.
`ExcelHwnd()` returns `Application.Hwnd` and is used by the multi-instance
identity smoke to prove the XLL receives the hosting Excel process, not a decoy
Excel process from the ROT.

```powershell
./scripts/stage-xll-application-addin.ps1
./scripts/run-xll-excel-application-identity-smoke.ps1 -StagingManifest target/xll-host-validation/application_addin/<run-id>/manifest.json -AllowUnavailable
```

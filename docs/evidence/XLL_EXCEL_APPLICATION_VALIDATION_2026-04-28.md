# XLL Excel Application Validation - 2026-04-28

## Completed Validation

- Generated source and compile validation passed:
  - `cargo test -p oxvba-build --lib xll --quiet`
- Host runtime consumption validation passed:
  - `cargo test -p oxvba-host --test xll_application_binding --quiet`
  - This proves project `CreateObject("OxVba.TestDispatch")` returns a pre-bound native host object before normal activation, which is the same registry path used by `Excel.Application`.
- Excel host validation passed:
  - `.\scripts\stage-xll-array-addin.ps1 -RunId 20260428T060000Z`
  - `.\scripts\run-xll-excel-worksheet-smoke.ps1 -StagingManifest target\xll-host-validation\array_addin\20260428T060000Z\manifest.json -ExpectedCsv examples\xll\array_addin\expected.csv -OutputRoot target\xll-host-validation\excel-array-worksheet -RunId 20260428T060000Z -AllowUnavailable`
  - Result: `passed=3 failed=0`
  - Trace includes `Excel.Application host root bound object=20001`
- Direct `Application.Value` Excel-host validation passed:
  - `.\scripts\stage-xll-application-addin.ps1 -RunId 20260428T063000Z`
  - `.\scripts\run-xll-excel-worksheet-smoke.ps1 -StagingManifest target\xll-host-validation\application_addin\20260428T063000Z\manifest.json -ExpectedCsv examples\xll\application_addin\expected.csv -OutputRoot target\xll-host-validation\excel-application-worksheet -RunId 20260428T063000Z -AllowUnavailable`
  - Result: `passed=1 failed=0`
  - Trace includes `Excel.Application host root bound object=20001`
  - Worksheet result: `=ExcelVersion()` observed `16.0`, matching Excel `Application.Version`.

## Fixture Surface

The generated XLL now acquires and binds the running Excel `Application` object in Excel, and project runtime tests prove the bound object is consumed by `CreateObject` before ordinary activation.

The first-class fixture uses a CLI/basproj host-injected project surface:

```vb
Attribute VB_Name = "Application"
Attribute VB_PredeclaredId = True

Public Property Get Value() As Object
    Set Value = CreateObject("Excel.Application")
End Property
```

The `.basproj` model now supports `<ProjectReference><Kind>HostInjected</Kind></ProjectReference>` for this fixture.

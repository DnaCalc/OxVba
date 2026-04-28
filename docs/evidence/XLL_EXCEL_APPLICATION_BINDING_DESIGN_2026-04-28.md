# XLL Excel Application Binding Design - 2026-04-28

## Goal

Give XLL-hosted OxVba code a real Excel root `Application` object without using CLI/LSP placeholders, launching a second Excel instance, or failing add-in load when Excel is not ready.

## Design

- Generated XLL shims set `HostConfig.root_object_name = Some("Application")` and lazily try to bind the running Excel application when the first runtime session is created.
- The Windows acquisition path is:
  - `CoInitializeEx(..., COINIT_APARTMENTTHREADED)`
  - `CLSIDFromProgID("Excel.Application")`
  - `GetActiveObject`
  - `IUnknown::QueryInterface(IDispatch)`
  - bind that retained `IDispatch` into the engine COM state under `Excel.Application`
- The host COM state now has an explicit host-object-by-ProgID lane. This is intentionally separate from normal `CreateObject` allocation so XLL-injected roots can be reused without globally changing COM activation behavior.
- `CreateObject("Excel.Application")` on the same engine first checks the host-object registry and returns the bound running Excel object when present.
- Existing host-injected project roots can surface the object through their public members. The immediate supported source pattern is:

```vb
Attribute VB_Name = "Application"
Attribute VB_PredeclaredId = True

Public Property Get Value() As Object
    Set Value = CreateObject("Excel.Application")
End Property
```

Consumer code can then use `Application.Value` as the injected Excel object.

## Non-Goals

- This bead does not generate a full Excel object-model facade as source code.
- This bead does not claim direct unqualified `Application.Workbooks` routing unless the compiled host surface exposes that member.
- This bead does not perform background-thread Excel automation.

## Evidence Plan

- Generated XLL source assertions prove the shim contains the acquisition and binding path.
- Generated XLL compile test proves the temporary shim project has the required dependencies and APIs.
- Excel worksheet smoke should continue to pass for existing XLL arrays, proving the optional acquisition path does not break early add-in load.
- Host runtime tests prove project `CreateObject` traffic consumes the pre-bound native host object.
- Excel-host fixture validation proves `Application.Value` can dispatch `Version` against the running Excel root object.

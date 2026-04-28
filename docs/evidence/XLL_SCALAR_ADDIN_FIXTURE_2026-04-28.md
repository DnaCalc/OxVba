# XLL Scalar Addin Fixture

Date: 2026-04-28
Bead: `bd-xll1.5.3`
Workset: `docs/worksets/WORKSET_2026-04-28_XLL_EXCEL_HOST_VALIDATION_EXECUTION.md`

## Scope

Created the deterministic scalar Addin fixture for Excel-host XLL validation:

- `examples/xll/scalar_addin/ScalarAddin.basproj`
- `examples/xll/scalar_addin/ScalarExports.bas`
- `examples/xll/scalar_addin/expected.csv`

The fixture exports:

- `AddDouble(ByVal x As Double, ByVal y As Double) As Double`
- `EchoText(ByVal s As String) As String`
- `NotFlag(ByVal b As Boolean) As Boolean`
- `IncLong(ByVal n As Long) As Long`

## Validation

Command:

```powershell
New-Item -ItemType Directory -Force -Path target/xll-host-validation/scalar_addin | Out-Null
cargo run -p oxvba-cli -- build examples/xll/scalar_addin -o target/xll-host-validation/scalar_addin/ScalarAddin.xll
```

Result:

```text
built examples/xll/scalar_addin -> target/xll-host-validation/scalar_addin/ScalarAddin.xll (2282496 bytes)
```

## Fixture Adjustment

The first build attempt exposed a fixture-language issue:

```text
PMR-E-BACKEND-COMPILE: type error: unsupported statement: pmr_scalaraddin_scalarexports_notflag = Not b
```

The Boolean fixture stayed in scope but was rewritten as an explicit `If` branch
so it remains inside the currently supported basic-language surface.

## Boundary

This evidence proves only that the fixture is valid enough to build through the
current local `OutputType=Addin` XLL packaging path. It does not prove Excel host
loading, `xlAutoOpen`, `xlfRegister`, or worksheet invocation.

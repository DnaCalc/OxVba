# XLL Array Excel Host Validation

Date: 2026-04-28
Bead: `bd-iyx4.2.4`
Workset: `docs/worksets/WORKSET_2026-04-28_OXIDE_XLL_ARRAY_APPLICATION_EXECUTION.md`

## Fixture

Added:

- `examples/xll/array_addin/ArrayAddin.basproj`
- `examples/xll/array_addin/ArrayExports.bas`
- `examples/xll/array_addin/expected.csv`
- `scripts/stage-xll-array-addin.ps1`

Exports:

- `MakeVector() As Variant`
- `SumArray(ByVal values As Variant) As Long`

## Implementation Finding Fixed

The first Excel run passed XLL array return checks but failed the array argument
check:

```text
SumArray({1,2,3}) expected 6 observed 12
```

This was not a fixture problem. Excel recalculation invoked the function more
than once, and the VM retained the function return slot between repeated
procedure invocations. The fix clears invoked procedure parameter/local/return
slots before applying new arguments while preserving session/global state.

Regression added:

```text
crates/oxvba-host/tests/invoke_procedure_tests.rs
invoke_function_clears_return_slot_between_repeated_calls
```

## Passing Run

Staging:

```powershell
./scripts/stage-xll-array-addin.ps1 -RunId 20260428T043000Z
```

Artifact:

```text
target/xll-host-validation/array_addin/20260428T043000Z/ArrayAddin.xll
bytes: 2305536
sha256: 33C6DC5603110583ECEAD0AE99468C3FD3FE275DE7B2D82556A4EC1B72AA636F
```

Excel worksheet validation:

```powershell
./scripts/run-xll-excel-worksheet-smoke.ps1 `
  -StagingManifest target/xll-host-validation/array_addin/20260428T043000Z/manifest.json `
  -ExpectedCsv examples/xll/array_addin/expected.csv `
  -OutputRoot target/xll-host-validation/excel-array-worksheet `
  -RunId 20260428T043000Z `
  -AllowUnavailable
```

Result:

```text
xll excel worksheet smoke: passed passed=3 failed=0
```

Observed rows:

| Function | Formula | Expected | Observed |
| --- | --- | --- | --- |
| `MakeVectorFirst` | `=INDEX(MakeVector(),1,1)` | `10` | `10` |
| `MakeVectorThird` | `=INDEX(MakeVector(),3,1)` | `30` | `30` |
| `SumArrayLiteral` | `=SumArray({1,2,3})` | `6` | `6` |

Registration trace:

```text
xlAutoOpen start
resolve_excel12v module=EXCEL.EXE symbol=MdCallBack12 found=true
xlGetName status=0 success=true
xlfRegister procedure=MakeVector type_text=Q status=0 success=true
xlfRegister procedure=SumArray type_text=QQ status=0 success=true
xlAutoOpen complete
```

## Validation Commands

```powershell
cargo test -p oxvba-build --lib xll --quiet
cargo test -p oxvba-host --test invoke_procedure_tests invoke_function_clears_return_slot_between_repeated_calls --quiet
```

Both passed.

## Boundary

This validates the bounded XLL array support matrix in Excel 16.0 build 19929
on Windows 64-bit: scalar-element `xltypeMulti` returns and scalar-element
`xltypeMulti` arguments. It does not claim nested arrays, object-valued array
elements, references, async arrays, RTD, or complete dynamic-array behavior.

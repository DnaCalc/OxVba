# XLL Excel Worksheet Invocation

Date: 2026-04-28
Bead: `bd-xll1.5.7`
Workset: `docs/worksets/WORKSET_2026-04-28_XLL_EXCEL_HOST_VALIDATION_EXECUTION.md`

## Scope

Prove worksheet invocation for the staged scalar Addin fixture after Excel host
registration succeeds.

## Harness

Added:

```powershell
./scripts/run-xll-excel-worksheet-smoke.ps1
```

The script loads a staged `.xll` with `Application.RegisterXLL(...)`, creates a
workbook, writes formulas from `examples/xll/scalar_addin/expected.csv`,
calculates, and compares observed values/text with expected values.

## Host Findings Fixed

The worksheet bead exposed two implementation-owned problems after registration
started passing:

- `#NAME?` for every formula: `xlfRegister` was missing the proper
  Excel module text / SDK-shaped registration argument vector.
- Excel RPC failure/crash on formula entry: the registered type strings claimed
  a typed scalar C ABI while the generated exports actually use the XLOPER12
  pointer ABI.

Fixes:

- `xlfRegister` now obtains module text via `xlGetName` (`0x4009`), passes a
  32-argument registration vector, uses macro type `"1"`, and fills unused
  optional slots with `xltypeMissing`.
- Registration type strings now match the generated wrapper ABI: return and
  arguments use `Q` (`LPXLOPER12`). Typed compiler/native-export metadata is
  still used by the wrapper to decode XLOPER12 values into retained `Variant`
  arguments before runtime invocation.

## Passing Run

Staging:

```powershell
./scripts/stage-xll-scalar-addin.ps1 -RunId 20260428T020000Z
```

Worksheet invocation:

```powershell
./scripts/run-xll-excel-worksheet-smoke.ps1 `
  -StagingManifest target/xll-host-validation/scalar_addin/20260428T020000Z/manifest.json `
  -RunId 20260428T020000Z `
  -AllowUnavailable
```

Result:

```text
xll excel worksheet smoke: passed passed=4 failed=0
```

Observed values:

| Function | Formula | Expected | Observed |
| --- | --- | --- | --- |
| `AddDouble` | `=AddDouble(2.5,3.25)` | `5.75` | `5.75` |
| `EchoText` | `=EchoText("abc")` | `abc` | `abc` |
| `NotFlag` | `=NotFlag(TRUE)` | `FALSE` | `FALSE` |
| `IncLong` | `=IncLong(41)` | `42` | `42` |

Registration trace:

```text
xlAutoOpen start
resolve_excel12v module=EXCEL.EXE symbol=MdCallBack12 found=true
xlGetName status=0 success=true
xlfRegister procedure=AddDouble type_text=QQQ status=0 success=true
xlfRegister procedure=EchoText type_text=QQ status=0 success=true
xlfRegister procedure=NotFlag type_text=QQ status=0 success=true
xlfRegister procedure=IncLong type_text=QQ status=0 success=true
xlAutoOpen complete
```

Result artifacts:

- `target/xll-host-validation/excel-worksheet/20260428T020000Z/worksheet_result.json`
- `target/xll-host-validation/excel-worksheet/20260428T020000Z/worksheet_results.csv`
- `target/xll-host-validation/excel-worksheet/20260428T020000Z/xll_trace.log`
- `target/xll-host-validation/excel-worksheet/20260428T020000Z/worksheet_smoke.xlsx`

## Validation

Commands run during this bead:

```powershell
cargo test -p oxvba-build xloper --quiet
cargo test -p oxvba-build --lib xll --quiet
cargo test -p oxvba-project --test validate_tests --quiet
```

Results:

- `cargo test -p oxvba-build xloper --quiet`: pass, 4/4
- `cargo test -p oxvba-build --lib xll --quiet`: pass, 5/5 before the final
  worksheet crash fix; final staging also compiled the generated XLL
- `cargo test -p oxvba-project --test validate_tests --quiet`: pass, 19/19

## Boundary

This proves the scoped scalar worksheet invocation matrix for Excel 16.0 build
19929 on Windows 64-bit. It does not claim arrays, async functions, RTD, macro
commands, custom UI, or macOS Excel parity.

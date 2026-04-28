# XLL Excel Registration Trace

Date: 2026-04-28
Bead: `bd-xll1.5.6`
Workset: `docs/worksets/WORKSET_2026-04-28_XLL_EXCEL_HOST_VALIDATION_EXECUTION.md`

## Scope

Prove `xlAutoOpen` and per-function `xlfRegister` behavior inside a real Excel
host for the staged scalar Addin fixture.

## Instrumentation

Generated XLL source now supports opt-in trace logging through
`OXVBA_XLL_TRACE`. The Excel host harness sets that variable for each run and
records whether the generated XLL wrote a trace file.

The generated XLL trace records:

- `xlAutoOpen start`,
- Excel callback resolver result,
- one `xlfRegister` row per exported function,
- `xlAutoOpen complete` or the failed procedure.

## Findings Fixed

First instrumented host run:

```text
xlAutoOpen start
xlfRegister procedure=AddDouble type_text=J status=-1 success=false
xlAutoOpen failed procedure=AddDouble
```

Implementation fixes:

- Generated XLL code no longer assumes `Excel12v` is exported directly.
  Microsoft's Excel C API documentation describes `Excel12v` as SDK source; the
  live Excel host exported `MdCallBack12`, so the generated shim now resolves
  `MdCallBack12` from `EXCEL.EXE` and calls it with the Excel12v argument vector.
- Compiler runtime procedure metadata now carries typed parameter and return
  metadata.
- Native export validation now uses decorated project runtime metadata by
  module/procedure identity and passes typed metadata to the XLL generator.
- Later worksheet invocation showed that typed scalar registration strings were
  the wrong exported ABI for the current generated wrapper. The final generator
  registers `Q` XLOPER12 pointer lanes and uses typed metadata only for
  wrapper-side decoding.

## Passing Host Run

Staging:

```powershell
./scripts/stage-xll-scalar-addin.ps1 -RunId 20260428T020000Z
```

Excel host run:

```powershell
./scripts/run-xll-excel-load-smoke.ps1 `
  -StagingManifest target/xll-host-validation/scalar_addin/20260428T020000Z/manifest.json `
  -RunId 20260428T020000Z `
  -AllowUnavailable
```

Trace:

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

Result summary:

- `RegisterXLL` returned `true`.
- Excel version: `16.0`
- Excel build: `19929`
- Excel operating system: `Windows (64-bit) NT 10.00`
- artifact: `target/xll-host-validation/scalar_addin/20260428T020000Z/ScalarAddin.xll`
- artifact bytes: `2301952`

## Validation

Commands:

```powershell
cargo test -p oxvba-build --lib xll --quiet
cargo check -p oxvba-compiler -p oxvba-project -p oxvba-build -p oxvba-cli --quiet
cargo test -p oxvba-project --test validate_tests --quiet
```

Results:

- `cargo test -p oxvba-build --lib xll --quiet`: pass, 5/5
- `cargo check -p oxvba-compiler -p oxvba-project -p oxvba-build -p oxvba-cli --quiet`: pass
- `cargo test -p oxvba-project --test validate_tests --quiet`: pass, 19/19

## Boundary

This proves registration. Worksheet invocation is captured separately in
`docs/evidence/XLL_EXCEL_WORKSHEET_INVOCATION_2026-04-28.md`.

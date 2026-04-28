# XLL Compile Staging Evidence

Date: 2026-04-27
Bead: `bd-xll1.5.1`

## Scope

This evidence covers the local build/staging prerequisite for the XLL host
validation lane. It does not claim Excel-loaded registration or worksheet
invocation parity.

## Changes

- `ShimOutputType::Xll` now compiles generated XLL source through the cdylib
  wrapper path and copies the resulting native library to the requested `.xll`
  output path.
- Generated XLL source now resolves Excel's `Excel12v` entry point at runtime
  from the loaded Excel/XLCALL module instead of requiring a link-time Excel
  import library.
- Generated XLL session state now uses thread-local storage because
  `ProjectRuntimeSession` is not `Send`/`Sync` and therefore cannot be placed
  behind a process-global `OnceLock<Mutex<_>>`.
- `xlAutoFree12` now accepts the generated `XLOPER12` pointer type directly.
- Generated XLL source now uses an Excel-compatible scalar `XLOPER12` layout
  with a `val` union before `xltype`, uses `xltypeInt = 0x0800`, and keeps
  counted-wide-string return buffers owned until `xlAutoFree12`.

## Validation Commands

```powershell
cargo fmt -p oxvba-build
cargo test -p oxvba-build --lib xll -- --nocapture
```

Results:

- `cargo test -p oxvba-build --lib xll --quiet`: pass, 5/5
- `xll::tests::xll_shim_compiles_to_xll_artifact`: pass; generated source
  compiled through `ShimOutputType::Xll` and produced a non-empty `.xll`
  artifact from a dummy embedded bundle file.

## Remaining Boundary

The next validation step is still external host evidence:

- load the generated `.xll` in Excel,
- verify `xlAutoOpen` registration,
- invoke at least one exported function from Excel,
- record exact pass/fail evidence.

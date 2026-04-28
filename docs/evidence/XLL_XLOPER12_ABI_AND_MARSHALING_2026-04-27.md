# XLL XLOPER12 ABI And Marshaling

Date: 2026-04-27
Bead: `bd-xll1.6`

## Scope

Replace the generated XLL placeholder scalar lane with the Excel 12 scalar
`XLOPER12` ABI shape and retained `Variant` marshaling.

## Changes

- Generated XLL source now emits `XLOPER12` with a `val` union followed by
  `xltype`, matching the Excel 12 header layout for scalar and bounded
  reference-compatible fields.
- `xltypeInt` is now `0x0800`; `0x0020` is treated as `xltypeFlow`, not an
  integer.
- Registration strings are length-prefixed UTF-16 buffers kept alive for the
  `Excel12v(xlfRegister, ...)` call.
- XLL arguments are decoded by `xltype` into retained `Variant` values for
  numeric, integer, Boolean, counted-wide-string, error, nil, and missing
  values.
- XLL returns allocate owned `XLOPER12` values with `xlbitDLLFree`; string
  returns keep their counted-wide buffer owned until `xlAutoFree12`.

## Validation

```powershell
cargo test -p oxvba-build xll_shim_has_required_entry_points --quiet
cargo test -p oxvba-build xll_registration_strings_are_counted_wide_and_owned_during_call --quiet
cargo test -p oxvba-build xll_argument_and_return_helpers_use_xltype_union_fields --quiet
cargo test -p oxvba-build xll_shim_compiles_to_xll_artifact --quiet
cargo test -p oxvba-build --lib xll --quiet
```

Results:

- all commands pass
- `cargo test -p oxvba-build --lib xll --quiet`: pass, 5/5

## Remaining Boundary

This proves generated source shape and local cdylib compilation. It still does
not prove Excel-loaded registration or worksheet invocation.

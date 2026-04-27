# XLL Registration Source Generation

Date: 2026-04-27
Bead: `bd-xll1.2`

## Scope

Deliver the first concrete XLL registration-generation slice on top of the
wrapper substrate.

## Change

`crates/oxvba-build/src/xll.rs` now emits generated XLL source with:

- a `REGISTRATIONS` table derived from `NativeExportDescriptor` rows,
- Excel type strings from the existing `xloper::build_type_string(...)` mapper,
- add-in category, function help, and argument help metadata from `.basproj`
  native-export metadata,
- `xlAutoOpen` iteration over the registration table,
- a Windows `Excel12v` / `xlfRegister` call path in generated source,
- a deterministic non-Windows registration stub for source-generation tests,
- and focused assertions that the generated source includes the registration
  table, `XLF_REGISTER`, `Excel12v`, type text, category, function help, and
  argument help fields.

## Validation

Commands:

```powershell
cargo fmt --check -p oxvba-build
cargo test -p oxvba-build --lib xll -- --nocapture
```

Results:

- `cargo fmt --check -p oxvba-build`: pass
- `cargo test -p oxvba-build --lib xll -- --nocapture`: pass, 2/2

## Remaining Boundary

This is registration source generation only. The next XLL delivery slice is the
runtime invocation bridge: XLOPER12 arguments into `RuntimeValue`, runtime
procedure invocation, and `RuntimeValue` results back to XLOPER12-compatible
return values.

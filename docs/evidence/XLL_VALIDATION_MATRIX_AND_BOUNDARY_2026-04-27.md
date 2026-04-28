# XLL Validation Matrix And Boundary

Date: 2026-04-27
Bead: `bd-xll1.4`

## Matrix

| Area | Current coverage | Result | Boundary |
| --- | --- | --- | --- |
| Addin project semantics and packaging | `.basproj` supports `OutputType=Addin`; add-in projects map to library project kind, reject top-level executable mainlines, and `oxvba build` packages the project as a generated `.xll`. | supported | Local packaging and the scoped Excel host scalar fixture are proven; broader add-in models remain out of scope. |
| Native export metadata | `NativeExportDescriptor` carries export name, module/procedure, calling convention, parameter/return type metadata, category, description, and argument descriptions. | supported | Metadata must be validated by project/native-export lanes before packaging. |
| XLL registration source | `generate_xll_shim(...)` emits `REGISTRATIONS`, `xlAutoOpen`, `MdCallBack12`-backed Excel callback resolution, `xlGetName`, and SDK-shaped `xlfRegister` calls derived from native export metadata. | supported by source-generation tests and Excel-host trace | Scoped scalar fixture registration is proven in Excel; broader Excel registration shapes remain future work. |
| XLL runtime invocation source | Generated XLL export wrappers marshal bounded XLOPER12 pointer values into retained `Variant` arguments, invoke `Engine::invoke_procedure_with_variants`, and allocate XLOPER12-compatible results with owned return storage. | supported by source-generation, generated-binary compile tests, and Excel worksheet invocation | Scoped Double, String, Boolean, and Long scalar invocation is proven in Excel. |
| XLOPER12 type strings | `xloper::build_type_string(...)` registers the generated wrapper ABI as `Q` XLOPER12 pointer return/argument lanes. Typed native-export metadata drives wrapper-side XLOPER12 decoding, not the exported C ABI shape. | supported by unit tests and Excel-host invocation | Direct typed scalar C ABI exports are not claimed. |
| Excel-loaded registration/invocation | Excel 16.0 build 19929 loads the staged scalar fixture, `xlAutoOpen` registers all four functions, and worksheet formulas return expected values. | proved for scoped scalar fixture | Arrays, async functions, RTD, macro commands, custom UI, and macOS Excel are not claimed. |

## Validation Commands

```powershell
cargo fmt --check -p oxvba-build
cargo test -p oxvba-build --lib xll -- --nocapture
./scripts/check-governance.ps1
git diff --check
```

Results:

- `cargo fmt --check -p oxvba-build`: pass
- `cargo test -p oxvba-build --lib xll --quiet`: pass, 5/5
- `cargo test -p oxvba-cli build_addin_project_produces_xll_artifact --quiet`: pass
- `./scripts/check-governance.ps1`: pass
- `git diff --check`: pass with CRLF conversion warnings only

## Supported Subset

The current XLL lane supports generated-source scaffolding for:

- add-in registration rows derived from canonical native export metadata,
- `oxvba build` default `.xll` package emission for `OutputType=Addin`,
- bounded Excel registration type strings,
- `xlAutoOpen`, `xlAutoClose`, and `xlAutoFree12` entry points,
- exported XLL wrapper functions with XLOPER12 pointer parameters,
- bounded numeric, Boolean, counted-wide-string, integer, error, nil, and
  missing-value conversion into retained `Variant`,
- runtime invocation via embedded `.oxb` session and
  `Engine::invoke_procedure_with_variants`,
- and bounded retained `Variant` to XLOPER12-compatible result allocation with
  `xlbitDLLFree` and `xlAutoFree12` ownership handling.

## Explicit Non-Claims

Do not claim:

- direct typed scalar C ABI exports,
- array/object XLL invocation,
- async functions, RTD, macro commands, or custom UI,
- macOS Excel parity,
- or full XLOPER12 ABI parity.

## Follow-Up

The scoped Excel-host scalar lane is now captured in:

- `docs/evidence/XLL_EXCEL_REGISTRATION_TRACE_2026-04-28.md`
- `docs/evidence/XLL_EXCEL_WORKSHEET_INVOCATION_2026-04-28.md`

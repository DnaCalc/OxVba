# XLL Validation Matrix And Boundary

Date: 2026-04-27
Bead: `bd-xll1.4`

## Matrix

| Area | Current coverage | Result | Boundary |
| --- | --- | --- | --- |
| Addin project semantics | `.basproj` supports `OutputType=Addin`; add-in projects map to library project kind and reject top-level executable mainlines. | supported | Project semantics only, not package emission by `oxvba build`. |
| Native export metadata | `NativeExportDescriptor` carries export name, module/procedure, calling convention, parameter/return type metadata, category, description, and argument descriptions. | supported | Metadata must be validated by project/native-export lanes before packaging. |
| XLL registration source | `generate_xll_shim(...)` emits `REGISTRATIONS`, `xlAutoOpen`, `Excel12v`, and `xlfRegister` source derived from native export metadata. | supported by source-generation tests | Not yet proven inside Excel. |
| XLL runtime invocation source | Generated XLL export wrappers marshal bounded XLOPER12-compatible pointer values into `RuntimeValue`, invoke `Engine::invoke_procedure`, and allocate XLOPER12-compatible results. | supported by source-generation tests | XLOPER12 layout is bounded source-generation scaffolding; Excel ABI parity is not yet proven. |
| XLOPER12 type strings | `xloper::build_type_string(...)` covers Double, Single, Long, Integer, Boolean, String, Currency, Date, Byte, LongLong, LongPtr, Variant, and Any. | supported by unit tests | Exact Excel SDK behavior for all edge cases remains future validation. |
| Excel-loaded registration/invocation | No current evidence in this run. | not proven | Requires a real Excel/XLL host validation lane. |

## Validation Commands

```powershell
cargo fmt --check -p oxvba-build
cargo test -p oxvba-build --lib xll -- --nocapture
./scripts/check-governance.ps1
git diff --check
```

Results:

- `cargo fmt --check -p oxvba-build`: pass
- `cargo test -p oxvba-build --lib xll -- --nocapture`: pass, 2/2
- `./scripts/check-governance.ps1`: pass
- `git diff --check`: pass with CRLF conversion warnings only

## Supported Subset

The current XLL lane supports generated-source scaffolding for:

- add-in registration rows derived from canonical native export metadata,
- bounded Excel registration type strings,
- `xlAutoOpen`, `xlAutoClose`, and `xlAutoFree12` entry points,
- exported XLL wrapper functions with XLOPER12 pointer-shaped parameters,
- bounded numeric, Boolean, string-placeholder, integer, and pointer-like
  conversion into `RuntimeValue`,
- runtime invocation via embedded `.oxb` session and `Engine::invoke_procedure`,
- and bounded `RuntimeValue` to XLOPER12-compatible result allocation.

## Explicit Non-Claims

Do not claim:

- the generated source has been compiled into an `.xll` binary in this run,
- Excel has loaded the generated `.xll`,
- `xlfRegister` succeeded in Excel,
- Excel worksheet invocation succeeded,
- or full XLOPER12 ABI parity.

## Follow-Up

The next bead is blocked on a real Excel-host validation path:

- build or stage a generated `.xll` binary,
- load it in Excel,
- verify `xlAutoOpen` registration,
- invoke at least one exported function from a worksheet or macro,
- record exact pass/fail evidence.

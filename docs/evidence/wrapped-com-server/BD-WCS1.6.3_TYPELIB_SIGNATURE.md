# bd-wcs1.6.3 WrappedComServer typelib signature export fidelity

Date: 2026-05-24

## Outcome

WrappedComServer generated type libraries now preserve compiler-derived VBA scalar signature metadata for COM class members instead of exporting every argument and retval as `VARIANT`.

Focused acceptance case verified:

```vb
Public Function AddThem(ByVal leftValue As Double, ByVal rightValue As Double) As Double
```

The generated TLB is loaded back through the Windows typelib loader and asserts:

- `AddThem.parameter_types == [Double, Double]`
- `AddThem.return_type == Some(Double)`

The same end-to-end test registers the generated wrapped COM server DLL, early-binds through `Dim calc As IMyCalc`, calls `calc.AddThem(1.25, 2.5)`, and observes `3.75`.

## Supported scalar subset exported to IDL/TLB

The typed export mapping now covers:

- `Integer` -> `VT_I2` / `short`
- `Long` -> `VT_I4` / `long`
- `Single` -> `VT_R4` / `float`
- `Double` -> `VT_R8` / `double`
- `Currency` -> `VT_CY` / `CY`
- `Date` -> `VT_DATE` / `DATE`
- `String` -> `VT_BSTR` / `BSTR`
- `Boolean` -> `VT_BOOL` / `VARIANT_BOOL`
- `Byte` -> `VT_UI1` / `unsigned char`
- `LongLong`, `LongPtr` -> `VT_I8` / `hyper`
- `Variant`, `Any`, object-like/unknown types -> `VT_VARIANT` / `VARIANT`

## Validation

- `cargo check -p oxvba-build --tests` — pass
- `cargo check -p oxvba-host --test com_early_project_end_to_end` — pass
- `cargo test -p oxvba-build --lib idl_` — pass
- `cargo test -p oxvba-project --test validate_tests` — pass
- `cargo test -p oxvba-com --lib typelib` — pass
- `cargo test -p oxvba-host --test com_early_project_end_to_end wrapped_com_server_build_register_and_early_bind_interface_addthem -- --nocapture` — pass, printed `3.75`
- `cargo check --workspace --all-targets` — pass

Note: `cargo test -p oxvba-build --lib` was attempted but timed out after 240 seconds while long-running generated artifact compile tests were still executing; targeted non-heavy `idl_` tests passed and the relevant wrapped COM server e2e acceptance passed.

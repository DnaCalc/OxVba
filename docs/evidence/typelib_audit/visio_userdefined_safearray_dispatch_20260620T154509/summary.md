# Visio user-defined SAFEARRAY dispatch audit

Date: 2026-06-20T15:45:09+02:00

Command:

```powershell
cargo run -p oxvba-com --example typelib_audit -- "Microsoft Visio 16.0 Type Library" "{00021A98-0000-0000-C000-000000000046}" 4 16 0
```

Result:

- The prior `VT_USERDEFINED` SAFEARRAY element rows now resolve as `VT_DISPATCH`.
- No `safearray_unresolved_userdefined` row is emitted for this Visio typelib.
- The object-array sites include `IVCell.Dependents` `retval`, `IVCell.Precedents` `retval`, and `IVSelection.ReplaceShape` `retval`, all with `VT_DISPATCH` element metadata.

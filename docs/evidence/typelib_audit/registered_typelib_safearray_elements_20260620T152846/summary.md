# Registered typelib SAFEARRAY element audit

Date: 2026-06-20T15:29:19.9724466+02:00

Command:

```powershell
cargo build -p oxvba-com --example typelib_audit
target/debug/examples/typelib_audit.exe <label> <libid> <major> <minor> <lcid>
```

Scope:

- Enumerated HKCR TypeLib registrations on this host.
- Audited the first 150 registered typelib version entries with the Rust `typelib_audit` example.
- Output rows are in `typelib_audit.csv`.

Result:

- See `safearray_element_vt`, `safearray_record`, and `safearray_unresolved_userdefined` rows for SAFEARRAY element evidence.
- `AcroBrokerLib` exposes one `safearray_record` row (`VT_RECORD` element).
- `Microsoft Visio 16.0 Type Library` exposes three unresolved user-defined SAFEARRAY elements.

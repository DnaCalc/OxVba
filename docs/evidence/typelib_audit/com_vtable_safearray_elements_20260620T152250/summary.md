# COM vtable SAFEARRAY element typelib audit

Date: 2026-06-20T15:22:53.7095385+02:00

Command:

```powershell
cargo run -p oxvba-com --example typelib_audit
```

Result:

- Excel, Office, VBA, and VBIDE registered typelibs were audited on this host.
- SAFEARRAY element rows were emitted for Excel, Office, and VBA.
- No safearray_record or safearray_unresolved_userdefined rows were emitted, so this host did not provide a foreign SAFEARRAY(VT_RECORD) specimen in that set.
- The audit output is stored in `typelib_audit.csv`.

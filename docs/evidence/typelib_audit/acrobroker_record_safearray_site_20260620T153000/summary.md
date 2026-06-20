# AcroBrokerLib SAFEARRAY(VT_RECORD) site audit

Date: 2026-06-20T15:30:00+02:00

Command:

```powershell
cargo run -p oxvba-com --example typelib_audit -- AcroBrokerLib '{41738EEA-442F-477F-92CF-2889BD6CD7E7}' 1 0 0
```

Result:

- The typelib exposes one `safearray_record` row.
- The record SAFEARRAY site is `IBroker.BrokerUpdateIEContextMenu` zero-based `param2` with element `VT_RECORD`.
- Other SAFEARRAY sites in this typelib use `VT_UI1`.

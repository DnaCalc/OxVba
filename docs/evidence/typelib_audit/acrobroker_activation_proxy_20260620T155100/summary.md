# AcroBroker activation and proxy facts

Date: 2026-06-20T15:51:00+02:00

Commands:

```powershell
cargo run -p oxvba-com --example typelib_audit -- AcroBrokerLib "{41738EEA-442F-477F-92CF-2889BD6CD7E7}" 1 0 0
```

Registry inspection:

```powershell
HKCR\CLSID\{BD57A9B2-4E7D-4892-9107-9F4106472DA4}
HKCR\Interface\{D3F22039-E3CF-4FC4-9A30-426A46056B8C}
```

Result:

- The typelib coclass row identifies `Broker` as `{BD57A9B2-4E7D-4892-9107-9F4106472DA4}`.
- The coclass is registered as `AcroBroker.Broker.1` / `AcroBroker.Broker`.
- The coclass is a local-server COM class: `C:\Program Files\Adobe\Acrobat DC\Acrobat\AcroBroker.exe`.
- `IBroker` is `{D3F22039-E3CF-4FC4-9A30-426A46056B8C}` and has `ProxyStubClsid32={00020424-0000-0000-C000-000000000046}` (`PSOAInterface`).
- The captured `SAFEARRAY(VT_RECORD)` member remains `IBroker.BrokerUpdateIEContextMenu` zero-based `param2`.
- Value-oracle invocation is still blocked: the captured member name indicates external side effects against IE context-menu state, so it should not be called without an isolated oracle design.

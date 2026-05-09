# Wrapped COM Events Controlled Sink Evidence

Capture: `wrapped_com_events_20260509T000000Z`
Bead: `bd-wcs1.8.3`
Matrix row: `COM-0010`

## Command

```powershell
./scripts/run-com-wrapped-server-events.ps1 -EvidenceDir docs/evidence/conformance/oracle_captures/wrapped_com_events_20260509T000000Z
```

## Result

- Controlled wrapped COM event sink test passed:
  `cargo test -p oxvba-build wrapped_com_server_build_compiles_dll_with_standard_exports --quiet`
- Log: `controlled_sink_test.log`
- Office probe: `office_probe.log`

## Evidence Covered

- Generated/registerable wrapped COM DLL and TypeLib build.
- Source dispinterface metadata for wrapped class events.
- `IConnectionPointContainer::FindConnectionPoint` for `_WidgetEvents`.
- `IConnectionPoint::Advise` with a live `IDispatch` sink and nonzero cookie.
- Wrapped `Widget.FireChanged(123)` dispatch into sink `IDispatch::Invoke`.
- Payload verification: event DISPID `1`, `VT_I4` payload `123`.
- `IConnectionPoint::Unadvise` and no callback after unsubscribe.

## Office/VBA Note

The environment probe found Excel COM automation available (`Excel available:
16.0`). This capture does not claim an Excel/VBA `WithEvents` client because the
current reproducible script uses the controlled sink path. Office/VBA
`WithEvents` coverage remains a deferred expansion beyond this controlled
COM-0010 subset.

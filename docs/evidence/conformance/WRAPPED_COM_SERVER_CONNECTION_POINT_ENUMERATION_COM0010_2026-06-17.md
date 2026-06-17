# WrappedComServer connection point enumeration COM-0010 evidence

Date: 2026-06-17
Bead: `bd-i91u`
Matrix row: `COM-0010`

## Scope

This evidence covers the clean `WrappedComServer` event lane after the generated
shim learned the standard COM connection-point enumeration surfaces:

- `IConnectionPointContainer::EnumConnectionPoints`
- `IConnectionPoint::EnumConnections`

The implemented subset remains intentionally bounded to the generated single
source dispinterface per event-capable class and the current `IDispatch` sink
table for that source interface.

## Command

```powershell
cargo test -p oxvba-build --test wrapped_com_server_smoke -- --ignored --nocapture
```

Result: passed on Windows with Excel installed.

## Verified behavior

- The smoke builds a throwaway `OutputType=ComServer`,
  `BuildTarget=WrappedComServer` project with `Calculator` events and a `Pinger`
  dual-interface probe.
- The generated DLL registers with `regsvr32.exe /s`, and the TypeLib is linked
  from the per-user CLSID registry entry.
- A raw COM client activates `DemoServer.Calculator`, queries
  `IConnectionPointContainer`, calls `EnumConnectionPoints`, and receives the
  generated source connection point.
- The enumerated connection point reports the same source IID used by
  `FindConnectionPoint`.
- `IEnumConnectionPoints::Next` returns `S_FALSE` after the single source
  connection point is exhausted, and `Reset` allows the source connection point
  to be enumerated again.
- After `Advise`, a raw COM client calls `IConnectionPoint::EnumConnections` and
  receives the currently advised sink cookie plus an `IUnknown` that supports
  `IDispatch`.
- `IEnumConnections::Next` returns `S_FALSE` after the one advised sink is
  exhausted, and `Reset` allows the snapshot to be enumerated again.
- The same smoke still proves event delivery by invoking `FireChanged(42)` and
  observing the dispatch sink callback.
- The same smoke also proves the Excel/VBA typed `WithEvents` client path by
  referencing the generated TypeLib and observing `Changed(77)`.

## Residual

`COM-0010` remains an implemented subset. The supported enumeration shape is
single-source connection-point enumeration plus snapshot enumeration of currently
advised `IDispatch` sinks. Multi-source event selection, richer event payload
families, broader callback ordering cases, and Office/VBA vtable-event-client
parity remain outside this slice.

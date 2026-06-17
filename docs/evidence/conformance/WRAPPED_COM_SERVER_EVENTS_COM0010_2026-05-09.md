# WrappedComServer events COM-0010 evidence

Date: 2026-05-09
Refreshed: 2026-06-17
Beads: `bd-wcs1.8.1`, `bd-wcs1.8.2`, `bd-wcs1.8.3`, `bd-i91u`
Matrix row: `COM-0010`

## Scope

This evidence covers the metadata publication and first controlled
connection-point runtime slice for wrapped COM events. It
proves that event descriptors already persisted in the compiled `.oxb` can feed
the wrapped server TypeLib generation path and produce source dispinterfaces for
event-capable classes. It also proves that a controlled external sink can
subscribe to the generated connection point, receive a wrapped event payload,
unsubscribe, and receive no later callbacks.

The original 2026-05-09 capture was not Office/VBA `WithEvents` oracle
evidence. The 2026-06-17 clean smoke now adds Excel/VBA `WithEvents` evidence
and bounded connection-point enumeration evidence; see
`docs/evidence/conformance/WRAPPED_COM_SERVER_CONNECTION_POINT_ENUMERATION_COM0010_2026-06-17.md`
and
`docs/evidence/conformance/WRAPPED_COM_SERVER_CLEAN_LATEBOUND_2026-06-17.md`.

## Commands

```powershell
cargo test -p oxvba-build generate_typelib --quiet
cargo test -p oxvba-build wrapped_com_server_build_compiles_dll_with_standard_exports --quiet
cargo check -p oxvba-build --quiet
./scripts/run-com-wrapped-server-events.ps1 -EvidenceDir docs/evidence/conformance/oracle_captures/wrapped_com_events_20260509T000000Z
cargo test -p oxvba-build --test wrapped_com_server_smoke -- --ignored --nocapture
```

## Verified behavior

- `compile_wrapped_com_server_shim` reads `descriptor_inventory.com_events`
  from the input `.oxb` and passes those event descriptors into TypeLib
  generation.
- `generate_typelib_with_events` emits a source dispinterface named
  `_<ClassName>Events` for matching class event descriptors.
- Source dispinterface IIDs are deterministic from the project name and source
  interface name.
- Event members are emitted as dispatch events with stable DISPIDs from
  `event_token` when present, otherwise deterministic ordinal fallback.
- Coclasses with events add the source dispinterface as a default source
  implemented type.
- The active TypeLib test covers a synthetic `Emitter.Changed` source
  dispinterface, and the wrapped server DLL test remains green after the
  event-aware TypeLib path.
- Generated wrapped objects with source events expose
  `IConnectionPointContainer`.
- `FindConnectionPoint` resolves the generated source IID for `_WidgetEvents`.
- `IConnectionPoint::Advise` queries and retains an `IDispatch` sink, allocates
  a nonzero cookie, and `Unadvise` releases that cookie.
- The Windows wrapped server DLL test fires `Widget.FireChanged(123)` and
  verifies the advised sink receives `IDispatch::Invoke` for event DISPID `1`
  with a `VT_I4` payload of `123`.
- The same test invokes `FireChanged` again after `Unadvise` and verifies no
  second callback reaches the sink.
- Timestamped controlled sink evidence is captured under
  `docs/evidence/conformance/oracle_captures/wrapped_com_events_20260509T000000Z/`.
- The Office probe in that capture found Excel automation available
  (`Excel available: 16.0`), but this evidence does not claim an Excel/VBA
  `WithEvents` client.
- 2026-06-17 clean smoke evidence proves `EnumConnectionPoints` returns the
  generated source connection point and `EnumConnections` returns a snapshot of
  the currently advised dispatch sink cookie/`IDispatch` identity.
- 2026-06-17 clean smoke evidence also proves an Excel/VBA typed `WithEvents`
  client can reference the generated TypeLib, subscribe to the source
  dispinterface, and observe `Changed(77)`.

## Residual

`COM-0010` is an `implemented-subset` for controlled dispatch-sink
connection-point events, bounded connection-point enumeration, and Excel/VBA
`WithEvents`. Multi-source event selection, richer payload shapes, broader
external ordering evidence, and Office/VBA vtable-event-client parity are not
claimed by this subset.

# WrappedComServer events COM-0010 evidence

Date: 2026-05-09
Beads: `bd-wcs1.8.1`, `bd-wcs1.8.2`
Matrix row: `COM-0010`

## Scope

This evidence covers the metadata publication and first controlled
connection-point runtime slice for wrapped COM events. It
proves that event descriptors already persisted in the compiled `.oxb` can feed
the wrapped server TypeLib generation path and produce source dispinterfaces for
event-capable classes. It also proves that a controlled external sink can
subscribe to the generated connection point, receive a wrapped event payload,
unsubscribe, and receive no later callbacks.

This is not Office/VBA `WithEvents` oracle evidence.

## Commands

```powershell
cargo test -p oxvba-build generate_typelib --quiet
cargo test -p oxvba-build wrapped_com_server_build_compiles_dll_with_standard_exports --quiet
cargo check -p oxvba-build --quiet
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

## Residual

`COM-0010` remains `in-progress` pending `bd-wcs1.8.3` oracle evidence. The
current runtime subset is controlled and dispatch-sink based; Office/VBA
`WithEvents`, `EnumConnectionPoints`, `EnumConnections`, multi-event selection,
richer payload shapes, and documented external ordering evidence are not claimed
by this bead.

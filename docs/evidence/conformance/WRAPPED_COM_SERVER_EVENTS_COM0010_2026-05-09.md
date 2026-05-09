# WrappedComServer events COM-0010 evidence

Date: 2026-05-09
Bead: `bd-wcs1.8.1`
Matrix row: `COM-0010`

## Scope

This evidence covers the metadata publication half of wrapped COM events. It
proves that event descriptors already persisted in the compiled `.oxb` can feed
the wrapped server TypeLib generation path and produce source dispinterfaces for
event-capable classes.

This is not connection-point runtime evidence. `IConnectionPointContainer`,
`Advise`, `Unadvise`, and live sink dispatch remain owned by later beads.

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

## Residual

`COM-0010` remains `in-progress`. Runtime connection-point publication,
`Advise`/`Unadvise`, `RaiseEvent` sink dispatch, sink payload evidence, and
Office/VBA `WithEvents` evidence are not claimed by this bead.

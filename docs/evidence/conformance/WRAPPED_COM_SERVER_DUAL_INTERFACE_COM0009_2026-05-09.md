# WrappedComServer dual-interface COM-0009 evidence

Date: 2026-05-09
Bead: `bd-wcs1.7.1`
Matrix row: `COM-0009`

## Scope

This evidence covers the first bounded dual-interface projection for
`WrappedComServer`. It proves that generated wrapped objects can expose a
deterministic custom interface IID, return that interface from `QueryInterface`,
and execute one Automation-safe vtable slot over the same runtime object used by
the dispatch path.

The supported signature slice is intentionally narrow: a no-argument method
published as `HRESULT Method([out, retval] LONG*)`. Broader parameters, property
vtable slots, object returns, arrays, records, and arbitrary native structs are
outside this bead.

## Commands

```powershell
cargo test -p oxvba-build com_server_has --quiet
cargo test -p oxvba-build generate_typelib --quiet
cargo test -p oxvba-build wrapped_com_server_build_compiles_dll_with_standard_exports --quiet
```

## Verified behavior

- Generated server source now emits deterministic `I<ClassName>` IIDs using the
  same project/class naming convention as the generated TypeLib.
- `OxVbaDispatchInstance` uses a dual-compatible vtable prefix: `IUnknown`,
  `IDispatch`, then the first bounded custom slot.
- `IClassFactory::CreateInstance` and object `QueryInterface` accept the custom
  interface IID for the class.
- The Windows DLL test queries `IWidget` from the wrapped `Widget` object and
  calls the custom `Ping` vtable slot. The vtable call returns `S_OK` and the
  expected `Long` value `7`.
- The generated TypeLib now emits the default interface as a dual Automation
  interface (`TYPEFLAG_FDUAL`, `TYPEFLAG_FOLEAUTOMATION`,
  `TYPEFLAG_FDISPATCHABLE`) with vtable offsets for members.
- Existing TypeLib generation and dispatch-backed wrapped server tests remain
  green after the dual-interface projection change.

## Residual

`COM-0009` remains `in-progress` until `bd-wcs1.7.2` proves dispatch/vtable
equivalence across the scoped member set and updates the validation row to the
final evidenced subset.

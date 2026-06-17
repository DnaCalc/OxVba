# WrappedComServer dual-interface COM-0009 evidence

Date: 2026-05-09
Refreshed: 2026-06-17
Beads: `bd-wcs1.7.1`, `bd-wcs1.7.2`, `bd-l7xl`, `bd-3wy1`, `bd-bgd9`, `bd-e7tj`, `bd-0id1`
Matrix row: `COM-0009`

## Scope

This evidence covers the bounded dual-interface projection for
`WrappedComServer`. It proves that generated wrapped objects can expose a
deterministic custom interface IID, return that interface from `QueryInterface`,
and execute one Automation-safe vtable slot over the same runtime object used by
the dispatch path. It also proves that the dispatch and vtable paths return the
same value for the supported member.

The original supported signature slice was intentionally narrow: a no-argument
method published as `HRESULT Method([out, retval] LONG*)`. The 2026-06-17 clean
smoke widens that bounded tier to a second scalar slot,
`HRESULT Method(LONG, LONG, [out, retval] LONG*)`. The same date's `bd-3wy1`
refresh adds a third bounded scalar slot,
`HRESULT Method(DOUBLE, DOUBLE, [out, retval] DOUBLE*)`; see
`docs/evidence/conformance/WRAPPED_COM_SERVER_DUAL_VTABLE_SCALAR_ARGS_COM0009_2026-06-17.md`.
The `bd-bgd9` refresh adds a separate two-slot `Long` property vtable shape,
`[propget] HRESULT Property(LONG*)` plus `[propput] HRESULT Property(LONG)`.
The `bd-e7tj` refresh adds a separate object-return method vtable shape,
`HRESULT Method(IDispatch**)`, with an optional no-argument `Long` slot for a
returned-object behavioral proof. The `bd-0id1` refresh adds a separate
same-server object-argument method shape, `HRESULT Method(IDispatch*, LONG*)`,
with inbound `VT_DISPATCH`/default-interface arguments bound back to generated
OxVBA project objects. Broader parameters, indexed/default or non-`Long`
property vtable slots, foreign COM object argument binding, arrays, records,
and arbitrary native structs remain outside this bead.

## Commands

```powershell
cargo test -p oxvba-build com_server_has --quiet
cargo test -p oxvba-build generate_typelib --quiet
cargo test -p oxvba-build wrapped_com_server_build_compiles_dll_with_standard_exports --quiet
cargo test -p oxvba-build --test wrapped_com_server_smoke -- --ignored --nocapture
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
- The same test invokes `Ping` through `IDispatch::Invoke` and asserts that the
  dispatch `VT_I4` result equals the vtable `[out, retval]` result from the same
  wrapped object.
- The generated TypeLib now emits the default interface as a dual Automation
  interface (`TYPEFLAG_FDUAL`, `TYPEFLAG_FOLEAUTOMATION`,
  `TYPEFLAG_FDISPATCHABLE`) with vtable offsets for members.
- The generated vtable surface exposes only the first supported no-argument
  method slot in this bead; later eligible members remain dispatch-only until a
  broader ABI tier is explicitly implemented.
- 2026-06-17 clean smoke widens the generated dual vtable surface to three
  bounded scalar slots for eligible classes: slot 7 no-argument `Long` return,
  slot 8 two `Long` inputs returning `Long`, and slot 9 two `Double` inputs
  returning `Double`.
- The clean smoke proves raw COM dispatch/vtable parity for `Pinger.Ping()` and
  `Pinger.AddPair(19, 23)` on the same wrapped object, then extends that parity
  to `Pinger.Average(10.5, 21.5)` through `VT_R8` dispatch arguments and the
  direct `double, double, double*` vtable ABI.
- The clean Excel smoke references the generated `.tlb`, creates
  `Dim pinger As Pinger`, and successfully calls `pinger.Ping()` plus
  `pinger.AddPair(19, 23)` plus `pinger.Average(10.5, 21.5)`.
- The clean smoke also publishes a separate `Counter.Value As Long` dual
  property shape, proves raw COM property get/put vtable calls agree with
  dispatch property get/put on the same object, and proves Excel/VBA early-bound
  `counter.Value` write/read through the generated TypeLib.
- The clean smoke also publishes a separate `Returner.ReturnSelf() As Object`
  dual object-return shape, proves raw COM vtable `IDispatch**` return and
  dispatch `VT_DISPATCH` return both produce usable returned dispatch objects,
  and proves Excel/VBA early-bound `ReturnSelf().Ping()` through the generated
  TypeLib.
- The clean smoke also publishes a separate
  `ObjectRelay.EchoPing(ByVal other As Object) As Long` object-argument shape,
  proves PowerShell and raw `IDispatch::Invoke` pass the generated object as
  `VT_DISPATCH`, proves raw COM vtable slot 8 accepts generated wrapper and
  default-interface object pointers, and proves Excel/VBA early-bound
  `relay.EchoPing(relay)` through the generated TypeLib.
- Existing TypeLib generation and dispatch-backed wrapped server tests remain
  green after the dual-interface projection change.

## Residual

`COM-0009` is an `implemented-subset` for the bounded scalar dual-interface
tier: slot 7 no-argument `Long` return, slot 8 two `Long` inputs returning
`Long`, slot 9 two `Double` inputs returning `Double`, plus a separate slot 7
`Long` property get and slot 8 `Long` property put shape, plus a separate slot 7
`Object` return method shape with optional slot 8 no-argument `Long` return,
plus a separate slot 7 no-argument `Long` return and slot 8 same-server
`Object` argument returning `Long` method shape.
Indexed/default properties, non-`Long` property signatures, object
`Set`/`PutRef` properties, ByRef writebacks, foreign COM object-argument
binding, object identity equivalence beyond same-server generated-object
argument/return behavioral proofs, arrays, error parity, optional/default
arguments, scalar signatures outside these exact slots, and arbitrary
additional vtable slots remain deferred.

# WrappedComServer bounded dual vtable COM-0009 evidence

Date: 2026-06-17
Beads: `bd-l7xl`, `bd-3wy1`, `bd-bgd9`, `bd-e7tj`, `bd-0id1`
Matrix row: `COM-0009`

## Scope

This evidence covers the clean `WrappedComServer` bounded dual-interface vtable
expansion from one no-argument scalar method to three bounded member shapes.

The scalar method shape is a contiguous prefix:

- slot 7: `HRESULT Ping([out, retval] long* result)`
- slot 8: `HRESULT AddPair(long a, long b, [out, retval] long* result)`
- slot 9: `HRESULT Average(double a, double b, [out, retval] double* result)`

The scalar property shape is a separate vtable layout:

- slot 7: `[propget] HRESULT value([out, retval] long* result)`
- slot 8: `[propput] HRESULT value(long newValue)`

The object-return method shape is also a separate vtable layout:

- slot 7: `HRESULT ReturnSelf([out, retval] IDispatch** result)`
- optional slot 8: `HRESULT Ping([out, retval] long* result)`

The object-argument method shape is another separate vtable layout:

- slot 7: `HRESULT Ping([out, retval] long* result)`
- slot 8: `HRESULT EchoPing([in] IDispatch* other, [out, retval] long* result)`

Only classes whose generated TypeLib surface exactly fits the bounded contiguous
method ABI prefix, the bounded `Long` property ABI, the bounded object-return
ABI, or the bounded same-server object-argument ABI are published as dual
interfaces. Other generated classes remain dispatch-only `dispinterface`s.

The `bd-e7tj` slice also fixes source-surface metadata so bare `As Object`
remains a COM object boundary type (`IDispatch*` / `IDispatch**`) while still
late-binding as an untyped object in the binder when it is not a known project
class.

## Command

```powershell
cargo test -p oxvba-build --test wrapped_com_server_smoke -- --ignored --nocapture
```

Result: passed on Windows with Excel installed.

## Verified behavior

- The smoke builds and registers a throwaway `OutputType=ComServer`,
  `BuildTarget=WrappedComServer` project with a creatable dual-eligible
  `Pinger` class.
- Generated IDL publishes `IPinger : IDispatch` with `Ping()` at slot 7,
  `AddPair(ByVal a As Long, ByVal b As Long) As Long` at slot 8, and
  `Average(ByVal a As Double, ByVal b As Double) As Double` at slot 9.
- Raw COM `QueryInterface(IPinger)` returns the generated dual vtable subobject.
- Raw COM slot 7 returns `42` through `HRESULT Ping(long*)`.
- Raw COM slot 8 passes two `Long` values and returns `42` through
  `HRESULT AddPair(long, long, long*)`.
- Raw COM slot 9 passes two `Double` values and returns `16` through
  `HRESULT Average(double, double, double*)`.
- The same object is invoked through `IDispatch::Invoke` for `Ping` and
  `AddPair(19, 23)` using `VT_I4`, plus `Average(10.5, 21.5)` using
  `VT_R8`, and dispatch results match the vtable results.
- Excel/VBA references the generated `.tlb`, creates `Dim pinger As Pinger`,
  and successfully calls `pinger.Ping()`, `pinger.AddPair(19, 23)`, and
  `pinger.Average(10.5, 21.5)`.
- Generated IDL also publishes `ICounter : IDispatch` for a dedicated
  `Counter.Value As Long` get/let class, with `propget` and `propput` HRESULT
  vtable signatures in distinct slots.
- Raw COM `QueryInterface(ICounter)` returns the generated property vtable
  subobject; raw slot 7 reads the same `Long` value as `IDispatch` property
  get, and raw slot 8 property put is immediately visible through
  `IDispatch` property get on the same object.
- Excel/VBA references the same `.tlb`, creates `Dim counter As Counter`, and
  successfully executes early-bound `counter.Value = 271` plus
  `counter.Value` readback.
- Generated IDL publishes `IReturner : IDispatch` for a dedicated
  `Returner.ReturnSelf() As Object` class, with slot 7 emitted as
  `HRESULT ReturnSelf(IDispatch** result)` and optional slot 8 emitted as
  `HRESULT Ping(long* result)`.
- Raw COM `QueryInterface(IReturner)` returns the generated object-return
  vtable subobject; raw slot 7 returns a non-null `IDispatch*`, and invoking
  `Ping()` through that returned dispatch object returns `42`.
- The same object-return member is invoked through `IDispatch::Invoke`, returns
  `VT_DISPATCH`, and the returned dispatch object also answers `Ping() = 42`.
- Excel/VBA references the same `.tlb`, creates `Dim returner As Returner`,
  executes `Set returnedFromReturner = returner.ReturnSelf()`, and successfully
  calls `returnedFromReturner.Ping()`.
- Generated IDL publishes `IObjectRelay : IDispatch` for a dedicated
  `ObjectRelay.EchoPing(ByVal other As Object) As Long` class, with slot 8
  emitted as `HRESULT EchoPing(IDispatch* other, long* result)`.
- PowerShell late-bound `ObjectRelay.EchoPing($relay)` passes the generated
  object as a `VT_DISPATCH` argument and returns `42`.
- Raw COM `QueryInterface(IObjectRelay)` returns the generated object-argument
  vtable subobject; raw slot 8 accepts both the wrapper `IDispatch*` and the
  generated default-interface pointer and returns `42`.
- The same object-argument member is invoked through `IDispatch::Invoke` with a
  `VT_DISPATCH` argument and returns the same `Long` value as the vtable path.
- Excel/VBA references the same `.tlb`, creates `Dim relay As ObjectRelay`, and
  successfully calls `relay.EchoPing(relay)`.

## Residual

`COM-0009` remains an implemented subset. Indexed/default properties,
non-`Long` property signatures, object `Set`/`PutRef` properties, ByRef
writebacks, foreign COM object-argument binding, object identity equivalence
beyond same-server generated-object argument/return behavioral proofs, arrays,
error parity, optional/default arguments, scalar signatures outside the exact
`Long` and `Double` slots above, and arbitrary numbers of vtable slots remain
outside this bounded tier.

# WrappedComServer dual vtable scalar-argument COM-0009 evidence

Date: 2026-06-17
Beads: `bd-l7xl`, `bd-3wy1`
Matrix row: `COM-0009`

## Scope

This evidence covers the clean `WrappedComServer` bounded dual-interface vtable
expansion from one no-argument scalar method to a three-slot scalar tier:

- slot 7: `HRESULT Ping([out, retval] long* result)`
- slot 8: `HRESULT AddPair(long a, long b, [out, retval] long* result)`
- slot 9: `HRESULT Average(double a, double b, [out, retval] double* result)`

Only classes whose generated TypeLib surface exactly fits the bounded contiguous
ABI prefix are published as dual interfaces. Other generated classes remain
dispatch-only `dispinterface`s.

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

## Residual

`COM-0009` remains an implemented subset. Properties, ByRef writebacks, object
identity equivalence across vtable returns/arguments, arrays, error parity,
optional/default arguments, scalar signatures outside the exact `Long` and
`Double` slots above, and arbitrary numbers of vtable slots remain outside this
bounded tier.

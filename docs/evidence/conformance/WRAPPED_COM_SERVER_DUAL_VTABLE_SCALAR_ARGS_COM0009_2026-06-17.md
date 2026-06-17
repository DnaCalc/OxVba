# WrappedComServer dual vtable scalar-argument COM-0009 evidence

Date: 2026-06-17
Bead: `bd-l7xl`
Matrix row: `COM-0009`

## Scope

This evidence covers the clean `WrappedComServer` bounded dual-interface vtable
expansion from one no-argument scalar method to a two-slot scalar tier:

- slot 7: `HRESULT Ping([out, retval] long* result)`
- slot 8: `HRESULT AddPair(long a, long b, [out, retval] long* result)`

Only classes whose generated TypeLib surface exactly fits the bounded ABI are
published as dual interfaces. Other generated classes remain dispatch-only
`dispinterface`s.

## Command

```powershell
cargo test -p oxvba-build --test wrapped_com_server_smoke -- --ignored --nocapture
```

Result: passed on Windows with Excel installed.

## Verified behavior

- The smoke builds and registers a throwaway `OutputType=ComServer`,
  `BuildTarget=WrappedComServer` project with a creatable dual-eligible
  `Pinger` class.
- Generated IDL publishes `IPinger : IDispatch` with `Ping()` at slot 7 and
  `AddPair(ByVal a As Long, ByVal b As Long) As Long` at slot 8.
- Raw COM `QueryInterface(IPinger)` returns the generated dual vtable subobject.
- Raw COM slot 7 returns `42` through `HRESULT Ping(long*)`.
- Raw COM slot 8 passes two `Long` values and returns `42` through
  `HRESULT AddPair(long, long, long*)`.
- The same object is invoked through `IDispatch::Invoke` for `Ping` and
  `AddPair(19, 23)`, and dispatch results match the vtable results.
- Excel/VBA references the generated `.tlb`, creates `Dim pinger As Pinger`,
  and successfully calls `pinger.Ping()` and `pinger.AddPair(19, 23)`.

## Residual

`COM-0009` remains an implemented subset. Properties, ByRef writebacks, object
identity equivalence across vtable returns/arguments, arrays, error parity,
optional/default arguments, non-`Long` scalar signatures, and arbitrary numbers
of vtable slots remain outside this bounded tier.

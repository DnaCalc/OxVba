# WrappedComServer dispatch type-info COM-0008 evidence

Date: 2026-06-17
Bead: `bd-47j0`
Matrix row: `COM-0008`

## Scope

This evidence covers the generated `WrappedComServer` object's own
`IDispatch::GetTypeInfoCount` and `IDispatch::GetTypeInfo` surface. It proves
that clients do not need to independently load the sibling `.tlb` to discover
the default dispatch interface for a live generated object.

This is not formula/UDF evidence, and it is not a broader dual-interface vtable
claim.

## Command

```powershell
cargo test -p oxvba-build --test wrapped_com_server_smoke -- --ignored --nocapture
```

Result: passed on Windows with Excel installed.

## Verified behavior

- The smoke builds and registers a throwaway `OutputType=ComServer`,
  `BuildTarget=WrappedComServer` project with generated `Calculator` and
  `Pinger` classes plus a sibling TypeLib.
- Raw COM clients call `IDispatch::GetTypeInfoCount` on live generated objects
  and receive count `1`.
- Raw COM clients call `IDispatch::GetTypeInfo(0, 0, ...)` and receive a live
  non-null `ITypeInfo`.
- The returned `ITypeInfo::GetTypeAttr().guid` matches the generated class
  descriptor's default-interface IID.
- The smoke releases the COM-owned `TYPEATTR` through
  `ITypeInfo::ReleaseTypeAttr` and releases the returned `ITypeInfo`.
- Raw COM clients call `IDispatch::GetTypeInfo(1, 0, ...)` and receive
  `DISP_E_BADINDEX` with a null output pointer.
- The same live smoke still covers registered activation, late-bound dispatch,
  bounded dual vtable dispatch equivalence, connection-point enumeration, Excel
  project-reference early binding, and Excel `WithEvents`.

## Residual

`COM-0008` remains an implemented subset. This evidence covers the standard
single default-interface `ITypeInfo` publication path for generated wrapped
objects. It does not claim `ITypeComp`, localization-sensitive type-info
selection, broken reference repair, broader Office version matrices,
Excel-facing error description parity, or VBA dual/vtable calls.

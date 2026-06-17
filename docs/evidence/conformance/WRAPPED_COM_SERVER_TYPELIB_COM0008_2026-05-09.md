# WrappedComServer typelib COM-0008 evidence

Date: 2026-05-09
Refreshed: 2026-06-17
Beads: `bd-wcs1.6.1`, `bd-wcs1.6.2`, `bd-9h5j`, `bd-47j0`
Matrix row: `COM-0008`

## Scope

This evidence covers the generated type-library and first controlled
TypeLib-aware client slice for `WrappedComServer`. It proves that wrapped COM
class descriptors can produce a sibling `.tlb`, that the `.tlb` loads through
the Windows type-library loader, that `DllRegisterServer` writes TypeLib
registry metadata alongside CLSID/ProgID registration, and that a controlled
client can bind through the generated `IWidget` type info before invoking the
wrapped class.

The original 2026-05-09 client execution path was explicitly bounded to
TypeLib-derived DISPIDs plus `IDispatch::Invoke`. The 2026-06-17 clean smoke now
adds Office/VBA project-reference evidence through Excel; see
`docs/evidence/conformance/WRAPPED_COM_SERVER_EXCEL_EARLY_BOUND_COM0008_2026-06-17.md`.
It also adds live generated-object `IDispatch::GetTypeInfo(0)` evidence; see
`docs/evidence/conformance/WRAPPED_COM_SERVER_DISPATCH_TYPEINFO_COM0008_2026-06-17.md`.

## Commands

```powershell
cargo test -p oxvba-build generate_typelib --quiet
cargo test -p oxvba-build wrapped_com_server_build_compiles_dll_with_standard_exports --quiet
cargo test -p oxvba-build --test wrapped_com_server_smoke -- --ignored --nocapture
```

## Verified behavior

- `generate_typelib` creates loadable `.tlb` files from
  `ComClassExportDescriptor` metadata.
- TypeLib roundtrip verification loads generated `.tlb` output with the Windows
  loader.
- Explicit DISPIDs, default bind flags, hidden/restricted flags, methods,
  property gets, property puts, and coclasses are covered by the active
  `oxvba-build` typelib tests.
- `compile_wrapped_com_server_shim` now emits a sibling `.tlb` next to the
  generated DLL and returns the path in `WrappedComServerBuildOutput`.
- The Windows `wrapped_com_server_build_compiles_dll_with_standard_exports`
  test loads the generated `.tlb`, resolves `Ping`, `Value`, `ReturnChild`,
  `Numbers`, and `Boom` through `ITypeLib::GetTypeInfoOfGuid` plus
  `ITypeInfo::GetIDsOfNames`, and verifies the resulting DISPIDs match the
  generated descriptor contract.
- The same test uses the TypeLib-derived DISPIDs to call the wrapped server for
  scalar method return, default property get/let, object return, array return,
  and error/`EXCEPINFO` behavior.
- The registered activation leg also validates that `CoCreateInstance` can call
  `Ping` through the TypeLib-derived DISPID.
- Generated `DllRegisterServer` writes per-user TypeLib entries under
  `HKCU\Software\Classes\TypeLib\{LIBID}\1.0\0\win64` and links each CLSID to
  the deterministic TypeLib LIBID.
- Generated `DllUnregisterServer` removes the per-user TypeLib tree during test
  cleanup.
- 2026-06-17 clean raw COM smoke proves generated objects report one dispatch
  type info, return an `ITypeInfo` for `IDispatch::GetTypeInfo(0)` whose
  `TYPEATTR.guid` matches the generated default-interface IID, and reject
  `GetTypeInfo(1)` with `DISP_E_BADINDEX` and a null output pointer.
- 2026-06-17 clean Excel smoke references the generated `.tlb` with
  `VBProject.References.AddFromFile`, compiles typed VBA against the generated
  `Calculator` class, and proves early-bound method, property put/get, object
  return, array return, and external Automation error `440` behavior.

## Residual

`COM-0008` is an `implemented-subset` for controlled TypeLib-aware
dispatch-backed early binding, live generated-object default-interface
`ITypeInfo` publication, and bounded Office/VBA project-reference calls. Broken
or missing reference behavior, broader Office version matrices, Excel-facing
error description parity, localization-sensitive type-info selection, and VBA
dual/vtable calls remain outside this subset.

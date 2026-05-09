# WrappedComServer typelib COM-0008 evidence

Date: 2026-05-09
Beads: `bd-wcs1.6.1`, `bd-wcs1.6.2`
Matrix row: `COM-0008`

## Scope

This evidence covers the generated type-library and first controlled
TypeLib-aware client slice for `WrappedComServer`. It proves that wrapped COM
class descriptors can produce a sibling `.tlb`, that the `.tlb` loads through
the Windows type-library loader, that `DllRegisterServer` writes TypeLib
registry metadata alongside CLSID/ProgID registration, and that a controlled
client can bind through the generated `IWidget` type info before invoking the
wrapped class.

This is not Office/VBA oracle evidence. The client execution path is explicitly
bounded to TypeLib-derived DISPIDs plus `IDispatch::Invoke`.

## Commands

```powershell
cargo test -p oxvba-build generate_typelib --quiet
cargo test -p oxvba-build wrapped_com_server_build_compiles_dll_with_standard_exports --quiet
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

## Residual

`COM-0008` is an `implemented-subset` for controlled TypeLib-aware
dispatch-backed early binding. Office/VBA project-reference evidence and broken
or missing reference behavior are still outside this subset and remain deferred
until a later oracle bead.

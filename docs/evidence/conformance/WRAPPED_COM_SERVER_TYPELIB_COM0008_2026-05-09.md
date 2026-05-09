# WrappedComServer typelib COM-0008 evidence

Date: 2026-05-09
Bead: `bd-wcs1.6.1`
Matrix row: `COM-0008`

## Scope

This evidence covers the generated type-library slice for `WrappedComServer`.
It proves that wrapped COM class descriptors can produce a sibling `.tlb`, that
the `.tlb` loads through the Windows type-library loader, and that
`DllRegisterServer` writes TypeLib registry metadata alongside CLSID/ProgID
registration.

This is not yet the early-bound client-call proof. Office/VBA or controlled
typed-client binding remains owned by `bd-wcs1.6.2`.

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
- Generated `DllRegisterServer` writes per-user TypeLib entries under
  `HKCU\Software\Classes\TypeLib\{LIBID}\1.0\0\win64` and links each CLSID to
  the deterministic TypeLib LIBID.
- Generated `DllUnregisterServer` removes the per-user TypeLib tree during test
  cleanup.

## Residual

`COM-0008` remains `in-progress`. The next required proof is a real Office/VBA
or controlled typelib-aware early-bound client that binds through the generated
TLB and successfully calls wrapped members.

# WrappedComServer late-bound COM-0007 evidence

Date: 2026-05-09
Bead: `bd-wcs1.5.4`
Matrix row: `COM-0007`

## Scope

This evidence covers the controlled Windows client slice for the generated
`WrappedComServer` DLL. It proves that the build emits an in-process COM DLL,
exports the standard server entry points, can be loaded by a native Windows
client, creates a wrapped OxVba class instance through `IClassFactory`, resolves
a member through `IDispatch::GetIDsOfNames`, and invokes the member through
`IDispatch::Invoke`.

This is not yet a registry or Office/VBA `CreateObject` claim. It is a
controlled native DLL-load client claim for late-bound method, property,
default member, object return, array return, and supported error propagation
through the generated `IDispatch` surface.

## Reusable command

```powershell
./scripts/run-com-wrapped-server-latebound.ps1
```

The script runs the controlled Windows Rust/Win32 client test:

```powershell
cargo test -p oxvba-build wrapped_com_server_build_compiles_dll_with_standard_exports --quiet
```

## Verified behavior

- `compile_wrapped_com_server_shim` emits a Windows `cdylib` DLL for
  `BuildTarget=WrappedComServer`.
- The DLL export table contains `DllGetClassObject`, `DllCanUnloadNow`,
  `DllRegisterServer`, and `DllUnregisterServer`.
- A native client loads the DLL with `LoadLibraryW`.
- `DllGetClassObject` returns `IClassFactory` for the deterministic wrapped
  class CLSID.
- `IClassFactory::LockServer` changes `DllCanUnloadNow` as expected.
- `IClassFactory::CreateInstance` returns an `IDispatch` object backed by
  `Engine::create_class_instance`.
- `IDispatch::GetIDsOfNames("Ping")` resolves the emitted member descriptor.
- `IDispatch::Invoke(DISPATCH_METHOD)` routes through the descriptor-backed
  call frame and returns `VT_I4` value `7` after `Class_Initialize` state is
  applied.
- `IDispatch::Invoke(DISPATCH_PROPERTYPUT)` routes to the `Property Let`
  descriptor for default DISPID `0`.
- `IDispatch::Invoke(DISPATCH_PROPERTYGET)` routes to the `Property Get`
  descriptor for default DISPID `0` and returns the updated `VT_I4` property
  value.
- A wrapped method returning a project class object marshals as `VT_DISPATCH`
  using a generated dispatch wrapper over the same runtime session.
- A wrapped method returning `Array(2, 4, 6)` marshals as
  `VT_ARRAY | VT_VARIANT` with lower bound `0`, upper bound `2`, and first
  element `VT_I4` value `2`.
- A wrapped method raising `Err.Raise 77` returns `DISP_E_EXCEPTION` and
  populates `EXCEPINFO.bstrDescription`.
- Releasing the object and factory restores `DllCanUnloadNow == S_OK`.

## Residual delivery work

`COM-0007` remains `implemented-subset`, not complete. Remaining delivery
surface is the registered per-user load path or equivalent `CreateObject`
evidence. Office/VBA early-bound behavior remains owned by `COM-0008`, not this
late-bound controlled-client row.

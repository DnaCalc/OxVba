# XLL Excel Application Binding Implementation - 2026-04-28

## Changes

- `crates/oxvba-build/src/xll.rs`
  - generated shims now create the engine with `root_object_name = "Application"`;
  - first session creation calls `try_bind_excel_application_root`;
  - Windows shims acquire the running Excel object with `CLSIDFromProgID` + `GetActiveObject`, query `IDispatch`, release the intermediate `IUnknown`, and bind the retained dispatch pointer into the engine;
  - non-Windows shims trace unavailability and continue.
- `crates/oxvba-host/src/engine.rs`
  - added `Engine::bind_native_dispatch_object` as the narrow host-facing native COM binding seam.
- `crates/oxvba-hal/src/traits.rs`
  - added `ComHal::bind_native_dispatch_object_variant`.
- `crates/oxvba-hal/src/adapters/standard/com.rs`
  - Windows standard host binds the retained `IDispatch` into the COM bridge and checks the bound host object before normal `CreateObject` activation.
- `crates/oxvba-com/src/windows_bridge.rs` and `crates/oxvba-com/src/windows_runtime_state.rs`
  - added host-object-by-ProgID state and binding helpers.
- `crates/oxvba-build/src/compile.rs`
  - generated shim compile projects now include `oxvba-com` and `windows-sys` for the acquisition path.

## Validation

- `cargo check -p oxvba-host --quiet` passed.
- `cargo test -p oxvba-hal --lib bind_native_dispatch_object_variant --quiet` passed as a compile-only filtered run.
- `cargo test -p oxvba-build --lib xll --quiet` passed, including the generated XLL compile test.
- `cargo test -p oxvba-host --test xll_application_binding --quiet` passed, proving project `CreateObject` traffic consumes a pre-bound native host object.
- `.\scripts\stage-xll-array-addin.ps1 -RunId 20260428T060000Z` produced `ArrayAddin.xll` (2,323,968 bytes), SHA256 `A19B275C534EFE83F2290B3AEDDB22571837D9DD57D28866A9C1DA4D3F0F143C`.
- `.\scripts\run-xll-excel-worksheet-smoke.ps1 ... -RunId 20260428T060000Z -AllowUnavailable` passed with `passed=3 failed=0`.
- The Excel XLL trace includes `Excel.Application host root bound object=20001`, proving the XLL acquired and bound the running Excel application object during an Excel-hosted run.
- `.\scripts\stage-xll-application-addin.ps1 -RunId 20260428T063000Z` produced `ApplicationAddin.xll` (2,312,192 bytes), SHA256 `F3D727F54788C337B5ACACF8C8E0C901CD09EF3B79B471A3A14A185AB256E2D3`.
- `.\scripts\run-xll-excel-worksheet-smoke.ps1 ... -RunId 20260428T063000Z -AllowUnavailable` passed with `passed=1 failed=0`.
- The Application fixture called `Application.Value`, dispatched `Version` against the injected Excel root object, and observed `16.0`.

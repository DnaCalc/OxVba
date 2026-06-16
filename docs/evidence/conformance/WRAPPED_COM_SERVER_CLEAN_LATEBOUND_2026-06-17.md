# WrappedComServer Clean Late-Bound Evidence

Date: 2026-06-17

Scope: clean `oxvba-build` / `oxvba-cli` implementation after legacy COM-server
paths were removed.

## Claim

`oxvba build <project.basproj> --target WrappedComServer --out-dir <dir>` emits
the canonical `.oxb` package, COM descriptor JSON, IDL, auditable generated Rust
shim source, and a compiled Windows in-process COM DLL for a bounded late-bound
Automation subset.

The generated DLL exports the standard in-process COM entry points, registers
creatable classes under `HKCU\Software\Classes`, activates by ProgID/CLSID, and
dispatches scalar `IDispatch::Invoke` calls through the package-backed runtime
session. Type library registration, dual-interface vtable calls, connection
points/events, and Office/VBA client evidence remain outside this slice.

## Evidence

- `cargo test -p oxvba-build`: passed, 4 library tests plus ignored Windows COM
  smoke test present.
- `cargo test -p oxvba-build --test wrapped_com_server_smoke -- --ignored --nocapture`:
  passed, 1 Windows COM smoke test.
- `cargo test -p oxvba-cli`: passed, 9 tests.
- `cargo check -p oxvba-build`: passed.
- Manual Windows smoke:
  - built a throwaway `OutputType=ComServer`, `BuildTarget=WrappedComServer`
    project with creatable `DemoServer.Calculator`.
  - `cargo run -p oxvba-cli -- build ... --target WrappedComServer --out-dir ...`
    produced `DemoServer.dll`.
  - `regsvr32.exe /s DemoServer.dll` succeeded.
  - `New-Object -ComObject DemoServer.Calculator` succeeded.
  - late-bound `$obj.Add(2, 3)` returned `5`.
  - `regsvr32.exe /u /s DemoServer.dll` was run after the smoke.

Repeatable test hook: `cargo test -p oxvba-build --test wrapped_com_server_smoke
-- --ignored` on Windows builds/registers a generated DLL and performs the same
late-bound `Add(2, 3)` activation smoke.

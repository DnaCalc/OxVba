# WrappedComServer Clean Dispatch And Event Evidence

Date: 2026-06-17

Scope: clean `oxvba-build` / `oxvba-cli` implementation after legacy COM-server
paths were removed.

## Claim

`oxvba build <project.basproj> --target WrappedComServer --out-dir <dir>` emits
the canonical `.oxb` package, COM descriptor JSON, IDL, compiled `.tlb`,
auditable generated Rust shim source, and a compiled Windows in-process COM DLL
for a bounded dispatch-backed Automation subset.

The generated DLL exports the standard in-process COM entry points, registers
creatable classes and the generated type library under
`HKCU\Software\Classes`, activates by ProgID/CLSID, and dispatches scalar
`IDispatch::Invoke` calls through the package-backed runtime session. For
classes with project events, it exposes the generated source dispinterface
through `IConnectionPointContainer`/`IConnectionPoint` and publishes
`RaiseEvent` payloads to advised `IDispatch` sinks with Automation argument
ordering.

Dual-interface vtable calls remain outside this slice. The generated TypeLib
therefore publishes the default class interface as a dispatch-only interface:
Excel/VBA can use typed member calls and `WithEvents`, but those calls bind
through `IDispatch`, not custom vtable slots.

## Evidence

- `cargo test -p oxvba-build`: passed, 4 library tests plus ignored Windows COM
  smoke test present.
- `cargo test -p oxvba-build --test wrapped_com_server_smoke -- --ignored --nocapture`:
  passed, 1 Windows COM smoke test, including generated `.tlb` existence,
  `CLSID\TypeLib` registration, TypeLib `win64` path registration, ProgID
  activation, late-bound `Add(2, 3)`, raw COM `IConnectionPoint` `Advise` /
  `RaiseEvent Changed(42)` / sink `Invoke` / `Unadvise`, and an Excel/VBA
  typed dispatch-interface `WithEvents` client receiving `Changed(77)`.
- `cargo test -p oxvba-cli`: passed, 9 tests.
- `cargo check --workspace`: passed.
- `./scripts/meta-check.ps1 -Fast -NoArtifacts`: passed, including governance,
  formatting, clippy with warnings denied, and workspace tests.
- Manual Windows smoke:
  - built a throwaway `OutputType=ComServer`, `BuildTarget=WrappedComServer`
    project with creatable `DemoServer.Calculator`.
  - `cargo run -p oxvba-cli -- build ... --target WrappedComServer --out-dir ...`
    produced `DemoServer.tlb` and `DemoServer.dll`.
  - `regsvr32.exe /s DemoServer.dll` succeeded.
  - `New-Object -ComObject DemoServer.Calculator` succeeded.
  - late-bound `$obj.Add(2, 3)` returned `5`.
  - a raw COM sink advised the generated source interface and observed
    `Changed(42)`.
  - Excel/VBA referenced `DemoServer.tlb`, created a typed `WithEvents`
    `Calculator` sink, invoked `Add`/`FireChanged`, and observed
    `Changed(77)`.
  - `regsvr32.exe /u /s DemoServer.dll` was run after the smoke.

Repeatable test hook: `cargo test -p oxvba-build --test wrapped_com_server_smoke
-- --ignored` on Windows builds/registers a generated DLL and performs the same
late-bound activation, connection-point event, and Excel/VBA `WithEvents`
smoke.

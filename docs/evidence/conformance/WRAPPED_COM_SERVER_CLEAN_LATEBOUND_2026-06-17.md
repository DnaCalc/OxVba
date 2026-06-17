# WrappedComServer Clean Dispatch, Event, And Dual Vtable Evidence

Date: 2026-06-17

Scope: clean `oxvba-build` / `oxvba-cli` implementation after legacy COM-server
paths were removed.

## Claim

`oxvba build <project.basproj> --target WrappedComServer --out-dir <dir>` emits
the canonical `.oxb` package, COM descriptor JSON, IDL, compiled `.tlb`,
auditable generated Rust shim source, and a compiled Windows in-process COM DLL
for a bounded Automation subset.

The generated DLL exports the standard in-process COM entry points, registers
creatable classes and the generated type library under
`HKCU\Software\Classes`, activates by ProgID/CLSID, and dispatches scalar
`IDispatch::Invoke` calls through the package-backed runtime session. For
classes with project events, it exposes the generated source dispinterface
through `IConnectionPointContainer`/`IConnectionPoint` and publishes
`RaiseEvent` payloads to advised `IDispatch` sinks with Automation argument
ordering. The event-capable class path also implements the standard bounded
enumeration surfaces: `IConnectionPointContainer::EnumConnectionPoints` returns
the generated source connection point, and `IConnectionPoint::EnumConnections`
returns a snapshot of the currently advised `IDispatch` sinks.

The generated TypeLib now uses a mixed default-interface policy. Classes outside
the implemented vtable subset are published as dispatch-only `dispinterface`s so
clients cannot accidentally vtable-call an unsupported face. Classes with exactly
one public no-argument `Long` method at vtable slot 7 are published as dual
Automation interfaces, and the generated DLL returns a real vtable subobject for
that interface IID. The supported vtable ABI is intentionally narrow:
`HRESULT Method([out, retval] long*)`.

## Evidence

- `cargo test -p oxvba-build`: passed, 4 library tests plus ignored Windows COM
  smoke test present.
- `cargo test -p oxvba-build --test wrapped_com_server_smoke -- --ignored --nocapture`:
  passed, 1 Windows COM smoke test, including generated `.tlb` existence,
  `CLSID\TypeLib` registration, TypeLib `win64` path registration, ProgID
  activation, late-bound `Add(2, 3)`, late-bound `Pinger.Ping()`, raw COM
  `IConnectionPointContainer::EnumConnectionPoints` /
  `IConnectionPoint::Advise` / `IConnectionPoint::EnumConnections` /
  `RaiseEvent Changed(42)` / sink `Invoke` / `Unadvise`, raw COM
  `QueryInterface(IPinger)` plus slot-7 vtable call, and an Excel/VBA typed
  dispatch-interface client covering method, property, object return, array
  return, external Automation error, and `WithEvents` receiving `Changed(77)`.
- `cargo test -p oxvba-cli`: passed, 9 tests.
- `cargo check --workspace`: passed.
- `./scripts/meta-check.ps1 -Fast -NoArtifacts`: passed, including governance,
  formatting, clippy with warnings denied, and workspace tests.
- Manual Windows smoke:
  - built a throwaway `OutputType=ComServer`, `BuildTarget=WrappedComServer`
    project with creatable `DemoServer.Calculator` and `DemoServer.Pinger`.
  - `cargo run -p oxvba-cli -- build ... --target WrappedComServer --out-dir ...`
    produced `DemoServer.tlb` and `DemoServer.dll`.
  - `regsvr32.exe /s DemoServer.dll` succeeded.
  - `New-Object -ComObject DemoServer.Calculator` succeeded.
  - late-bound `$obj.Add(2, 3)` returned `5`.
  - late-bound `$pinger.Ping()` returned `42`.
  - a raw COM sink advised the generated source interface and observed
    `Changed(42)`.
  - raw COM `EnumConnectionPoints` returned the generated source connection
    point, and raw COM `EnumConnections` returned the advised dispatch sink
    cookie.
  - a raw COM client queried the generated `IPinger` IID, called the custom
    vtable slot, and observed the same `42` result as the dispatch path.
  - Excel/VBA referenced `DemoServer.tlb`, created a typed `WithEvents`
    `Calculator` sink, invoked early-bound `Add`, `Value` property put/get,
    `ReturnSelf`, `Numbers`, `Boom`, and `FireChanged`, then observed
    `Changed(77)` plus Excel's external Automation error `440` for `Boom`.
  - `regsvr32.exe /u /s DemoServer.dll` was run after the smoke.

Repeatable test hook: `cargo test -p oxvba-build --test wrapped_com_server_smoke
-- --ignored` on Windows builds/registers a generated DLL and performs the same
late-bound activation, bounded dual-interface vtable, connection-point
enumeration/event, and Excel/VBA early-bound/`WithEvents` smoke.

Residual: broader dual-interface argument/property/byref/object/array/error
parity remains outside this clean slice. COM event evidence covers a single
generated source connection point and snapshot enumeration of advised dispatch
sinks; multi-source event selection, richer payload families, and broader
callback ordering cases remain outside this slice. Excel/VBA evidence here
covers typed dispatch-interface calls and `WithEvents`, but not broken reference
repair, broader Office version matrices, or Excel-facing error description
parity; the vtable evidence is the controlled raw-COM `IPinger` call.

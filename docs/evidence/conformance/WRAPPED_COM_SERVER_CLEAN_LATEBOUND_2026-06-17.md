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
`IDispatch::Invoke` calls through the package-backed runtime session. Live
generated objects also publish one dispatch type info:
`IDispatch::GetTypeInfo(0)` returns the generated default-interface
`ITypeInfo`, and invalid type-info indices return `DISP_E_BADINDEX`. For
classes with project events, it exposes the generated source dispinterface
through `IConnectionPointContainer`/`IConnectionPoint` and publishes
`RaiseEvent` payloads to advised `IDispatch` sinks with Automation argument
ordering. The event-capable class path also implements the standard bounded
enumeration surfaces: `IConnectionPointContainer::EnumConnectionPoints` returns
the generated source connection point, and `IConnectionPoint::EnumConnections`
returns a snapshot of the currently advised `IDispatch` sinks.

The generated TypeLib now uses a mixed default-interface policy. Classes outside
the implemented vtable subset are published as dispatch-only `dispinterface`s so
clients cannot accidentally vtable-call an unsupported face. Classes whose public
member surface fits the bounded scalar vtable tier are published as dual
Automation interfaces, and the generated DLL returns a real vtable subobject for
that interface IID. The supported vtable ABI is intentionally narrow: slot 7
`HRESULT Method([out, retval] long*)`, optionally followed by slot 8
`HRESULT Method(long, long, [out, retval] long*)`, optionally followed by slot 9
`HRESULT Method(double, double, [out, retval] double*)`; or, for a separate
property-only shape, slot 7 `[propget] HRESULT Property(long*)` followed by slot
8 `[propput] HRESULT Property(long)`; or, for a separate object-return shape,
slot 7 `HRESULT Method(IDispatch**)`, optionally followed by slot 8
`HRESULT Method(long*)`; or, for a separate same-server object-argument shape,
slot 7 `HRESULT Method(long*)` followed by slot 8
`HRESULT Method(IDispatch*, long*)`.

## Evidence

- `cargo test -p oxvba-build`: passed, 4 library tests plus ignored Windows COM
  smoke test present.
- `cargo test -p oxvba-build --test wrapped_com_server_smoke -- --ignored --nocapture`:
  passed, 1 Windows COM smoke test, including generated `.tlb` existence,
  `CLSID\TypeLib` registration, TypeLib `win64` path registration, ProgID
  activation, late-bound `Add(2, 3)`, live `IDispatch::GetTypeInfoCount`,
  `IDispatch::GetTypeInfo(0)` returning default-interface `ITypeInfo`,
  `ITypeInfo::GetTypeAttr().guid` matching the generated default-interface IID,
  `IDispatch::GetTypeInfo(1)` returning `DISP_E_BADINDEX`, late-bound
  `Pinger.Ping()`, raw COM
  `IConnectionPointContainer::EnumConnectionPoints` /
  `IConnectionPoint::Advise` / `IConnectionPoint::EnumConnections` /
  `RaiseEvent Changed(42)` / sink `Invoke` / `Unadvise`, raw COM
  `QueryInterface(IPinger)` plus slot-7, slot-8, and slot-9 vtable calls, and an
  Excel/VBA typed dispatch-interface/dual-interface client covering method,
  property, object return, array return, dual `Pinger.Ping()`,
  `Pinger.AddPair(19, 23)`, `Pinger.Average(10.5, 21.5)`, external Automation
  error, dual `Counter.Value` get/let, dual `Returner.ReturnSelf() As Object`
  returning a callable dispatch object, and `WithEvents` receiving
  `Changed(77)`.
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
  - raw COM `IDispatch::GetTypeInfo(0)` returned the generated
    default-interface `ITypeInfo`; `GetTypeInfo(1)` returned
    `DISP_E_BADINDEX`.
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

2026-06-17 live ignored smoke additionally proved late-bound
`Pinger.AddPair(19, 23)`, raw COM `IPinger` slot-8
`HRESULT AddPair(long, long, long*)`, dispatch/vtable parity for that slot, and
Excel/VBA early-bound calls to `Pinger.Ping` and `Pinger.AddPair`.

The same live smoke now also proves late-bound `Pinger.Average(10.5, 21.5)`,
raw COM `IPinger` slot-9 `HRESULT Average(double, double, double*)`,
`VT_R8` dispatch/vtable parity for that slot, and Excel/VBA early-bound calls
to `Pinger.Average`.

The same live smoke now also proves late-bound `Counter.Value` property put/get,
raw COM `ICounter` slot-7 `HRESULT value(long*)` and slot-8
`HRESULT value(long)`, dispatch/vtable parity for both property directions, and
Excel/VBA early-bound calls to `Counter.Value`.

The same live smoke now also proves bare `As Object` source metadata exports as
an object boundary type, late-bound `Returner.ReturnSelf().Ping()`, raw COM
`IReturner` slot-7 `HRESULT ReturnSelf(IDispatch**)`, dispatch `VT_DISPATCH`
return behavior for the same member, vtable-returned and dispatch-returned
objects callable through `IDispatch`, and Excel/VBA early-bound calls to
`Returner.ReturnSelf().Ping()`.

The same live smoke now also proves late-bound
`ObjectRelay.EchoPing(ObjectRelay)`, raw COM `IObjectRelay` slot-8
`HRESULT EchoPing(IDispatch*, long*)`, dispatch `VT_DISPATCH` object-argument
binding to the same generated project object, vtable calls with generated
wrapper and default-interface object pointers, and Excel/VBA early-bound calls
to `ObjectRelay.EchoPing`.

Repeatable test hook: `cargo test -p oxvba-build --test wrapped_com_server_smoke
-- --ignored` on Windows builds/registers a generated DLL and performs the same
late-bound activation, bounded dual-interface vtable including the two-`Long`
argument slot, the two-`Double` argument slot, and the two-slot `Long` property
shape plus the object-return `IDispatch**` shape and same-server
object-argument `IDispatch*` shape, connection-point enumeration/event, live
dispatch type-info publication, and Excel/VBA early-bound/`WithEvents` smoke.

Residual: broader dual-interface indexed/default property, non-`Long` property,
ByRef, foreign COM object-argument binding, array, and error parity,
optional/default arguments, scalar signatures outside the exact bounded `Long`
and `Double` slots, object identity equivalence beyond same-server
generated-object argument/return behavioral proofs, and arbitrary vtable slot
counts remain outside this clean slice. COM event evidence covers a
single generated source connection point and snapshot enumeration of advised
dispatch sinks; multi-source event selection, richer payload families, and
broader callback ordering cases remain outside this slice. Dispatch type-info
evidence here covers the generated default-interface `ITypeInfo`, but not
`ITypeComp` or localization-sensitive type-info selection. Excel/VBA evidence
here covers typed dispatch-interface calls, bounded dual-interface calls, and
`WithEvents`, but not broken reference repair, broader Office version matrices,
or Excel-facing error description parity.

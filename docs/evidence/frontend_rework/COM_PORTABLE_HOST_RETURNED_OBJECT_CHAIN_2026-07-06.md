# Portable Host-Returned COM Object Chain Evidence

Date: 2026-07-06
Bead: `bd-aprs.8.8` under IP-08B / FE-7.6.a
Worksets:
- `docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

The portable host COM projection can now retain object-valued results returned by a
host-provided dispatch object and route the next descriptor-backed member/default-member call
through that returned object.

The exercised source shape is:

```vb
n = Application.Workbooks.Count
n = n + Application.Workbooks
```

`Application.Workbooks` is a host-injected `Property Get` with an `InterfacePointer("Workbooks")`
wire type. At runtime the portable `Excel.Application` object returns a retained
`Excel.Workbooks` portable dispatch object. The VM then executes `Workbooks.Count` and the
`Workbooks` default member through the returned projection object, producing the expected
portable call trace:

```text
Application.Workbooks:get-object
Workbooks.Count:get
Application.Workbooks:get-object
Workbooks.Item:get
```

## Implementation Notes

- `PortableDispatch` now has optional object-valued `get_object` and `invoke_object` hooks.
- `oxvba-hal` converts a `PortableObjectResult` into the same projection-backed `ObjectRef`
  carrier used by existing host COM dispatch.
- Returned portable objects allocate fresh projection handles, while explicit portable
  `CreateObject` activation keeps its existing stable per-ProgID handle behavior.
- Portable registry hits are checked before the native COM capability gate, so registered
  host-provided objects can execute on Linux/macOS without advertising native COM support.
- Unregistered/native COM activation remains capability-gated; this does not make arbitrary
  Linux COM activation available.

## Checks

- `cargo test -p oxvba-host --test vba_web_com_lanes engine_preserves_portable_com_projection_across_policy_rebuild -- --nocapture`
- `cargo test -p oxvba-host --test vba_web_com_lanes engine_executes_host_returned_com_object_chain_through_portable_projection -- --nocapture`
- `cargo test -p oxvba-host --test vba_web_com_lanes -- --format terse`
- `cargo test -p oxvba-com -- --format terse`
- `cargo test -p oxvba-bind --test bind_roundtrip com_return -- --format terse`
- `cargo test -p oxvba-bind --test bind_roundtrip generic_com_object_return_stays_late_bound -- --format terse`
- `cargo fmt --all --check`
- `git diff --check`
- `cargo check -p oxvba-hal -p oxvba-com -p oxvba-host`
- `cargo check --workspace`
- `br lint bd-aprs.8.8 --json`
- `br dep cycles --json`

Attempted but not green in this Linux host:

- `cargo test -p oxvba-hal -- --format terse` still fails the existing Windows-profile HAL
  conformance rows for `typelib.resolve_typelib_reference` with
  `HAL-E-CAP-UNAVAILABLE`; this is outside the portable object-return path and the crate compiles
  cleanly with `cargo check`.

Unavailable on this host:

- PowerShell-backed staged/governance scripts; neither `pwsh` nor `powershell` is installed.

## Boundary

This is portable host-projection evidence for a host-returned object chain. It does not close live
Windows COM event writeback, arbitrary Office object-model breadth, COM export readiness, or the
V11 ByRef event row. The V11 blocker remains recorded in
`docs/evidence/frontend_rework/COM_BYREF_EVENT_WRITEBACK_BLOCKER_2026-07-02.md` and
`CURRENT_BLOCKERS.md`.

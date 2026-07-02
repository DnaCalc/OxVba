# COM Interface Return Provider Expansion Evidence

Date: 2026-07-02
Bead: `bd-aprs.8.8.3` under `bd-aprs.8.8`
Worksets:
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
- `docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`

## Outcome

Type-library provider construction now follows same-library COM members whose descriptors return
`TypeLibWireType::InterfacePointer { name }`.

Each imported or host-injected typelib request resolves its initial metadata blob, scans object
return descriptors for named interfaces, and resolves those interface names through the same
typelib request before building the provider chain. Requests are de-duplicated by
`(reference_name, requested_coclass)`, so recursive interface-return graphs do not loop.

This removes the temporary need for a second fake reference such as `Workbooks` when the real
reference is `Excel`/`Excel.Application`. Generic COM `Object` returns still have no static
interface identity and remain late-bound.

## Regression Shape

- `Dim app As Application: n = app.Workbooks.Count` lowers both hops as descriptor-backed
  `EarlyCom` calls with only the `Excel` type-library reference in scope.
- `n = app.Workbooks` reads the returned `Workbooks` default member through the same-library
  interface provider.
- `n = Application.Workbooks.Count` and `n = Application.Workbooks` prove the same expansion for a
  host-injected `Excel.Application` root without adding a separate `Workbooks` reference.
- `Application.DynamicThing.Count`, where `DynamicThing` returns a generic COM `Object`, remains a
  late-bound dispatch.

## Checks

- `cargo test -p oxvba-bind --test bind_roundtrip com_return -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip generic_com_object_return_stays_late_bound -- --nocapture`
- `cargo test -p oxvba-symbol -- --nocapture`
- `cargo clippy -p oxvba-bind --tests -- -D warnings`
- `cargo clippy -p oxvba-symbol --tests -- -D warnings`
- `cargo test -p oxvba-bind -- --nocapture`
- `cargo fmt --all --check`
- `git diff --check`
- `br dep cycles --json`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-governance.ps1`

Attempted but not counted as pass evidence:

- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\meta-check.ps1 -Fast -NoArtifacts`
  timed out twice while running workspace-wide cargo checks; owned leftover check PIDs were stopped.
- `cargo test --workspace` timed out in an isolated 15-minute run with no useful diagnostic output;
  the owned leftover cargo PIDs were stopped.

## Boundary

This is a provider-expansion slice for named same-typelib interface returns. It does not close the
full Excel/Office object model, runtime COM transport parity, or library-wide coclass/member
ownership scoping. Those remain open under `bd-aprs.8.8`/`IP-08B` until direct parity evidence and
tests cover them.

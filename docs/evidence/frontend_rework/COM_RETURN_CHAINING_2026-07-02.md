# COM Return Chaining Evidence

Date: 2026-07-02
Bead: `bd-aprs.8.8.2` under `bd-aprs.8.8`
Worksets:
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
- `docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`

## Outcome

Early-bound COM calls now infer a specific object type when the resolved member descriptor has
`return_wire_type = InterfacePointer { name }`.

That inferred `VarTypeRef::Object(name)` feeds the next member/default-member lookup, so a chained
call on a host-injected or typed imported COM receiver stays descriptor-backed instead of degrading
to `Variant`/late dispatch.

Generic COM `Object` returns without an interface-pointer name remain dynamic late-bound objects.
This slice does not treat legacy `Variant` degradation as a compatibility target.

## Regression Shape

- `Dim app As Application: n = app.Workbooks.Count` lowers the second hop (`Count`) as `EarlyCom`,
  with the receiver of that call coming from the `Workbooks` early-COM property descriptor.
- `n = app.Workbooks` in a value context can read the returned `Workbooks` default member through
  the returned type descriptor.
- `n = Application.Workbooks.Count` and `n = Application.Workbooks` prove the same behavior for a
  host-injected `Application` root.
- `Application.DynamicThing.Count`, where `DynamicThing` returns a generic COM `Object` rather than
  a named interface pointer, remains late-bound.

## Checks

- `cargo test -p oxvba-bind --test bind_roundtrip com_return -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip generic_com_object_return_stays_late_bound -- --nocapture`
- `cargo clippy -p oxvba-bind --tests -- -D warnings`
- `cargo test -p oxvba-bind -- --nocapture`
- `git diff --check`
- `br dep cycles --json`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-governance.ps1`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\meta-check.ps1 -Fast -NoArtifacts`

## Boundary

This is the typed COM return/chaining slice of `FE-7.6.a`. It does not close the broader Excel/Office
object model, runtime COM transport parity, library-wide coclass scoping, or all imported
member/property/default-member rows. Those lanes remain `in-progress`.

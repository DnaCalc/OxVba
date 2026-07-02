# COM Named Argument Binding Evidence

Date: 2026-07-02
Bead: `bd-yxpt` under `bd-aprs.8.8`
Worksets:
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
- `docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`

## Outcome

Early-bound COM and host-injected COM calls now bind named arguments against the resolved
`TypeLibMemberMetadata` parameter list at compile/bind time.

For statically known COM receivers, the binder now:

- validates named arguments against the typelib parameter names,
- rejects duplicate names before lowering,
- reorders named arguments into descriptor parameter order,
- preserves explicit omitted optional gaps where a later named argument is supplied,
- applies the existing typelib-driven ByRef and scalar coercion rules to named arguments, and
- excludes hidden `[lcid]` descriptor slots from the VBA-visible argument list.

Plain `Object`/`Variant` late binding remains dynamic and still preserves `CoreArg::Named` for
runtime COM dispatch. This slice does not preserve the old permissive early-COM behavior as a
compatibility target.

## Regression Shape

- `Application.Run(Arg1:=1, Macro:="MacroName")` on both a host-injected root and a typed COM
  receiver lowers as `receiver, Macro, Arg1`, with no `CoreArg::Named` reaching `EarlyCom`.
- `Application.OnTime Schedule:=False, Procedure:="MacroName", EarliestTime:=0` lowers as
  `receiver, EarliestTime, Procedure, Omitted, Schedule`.
- `Application.Run(NotAParam:="MacroName")` now fails during binding with `Named argument not
  found` instead of deferring the unknown name to dynamic COM dispatch.
- Duplicate COM named arguments fail during binding.

## Checks

- `cargo test -p oxvba-bind --test bind_roundtrip named_args -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip typed_com_named_argument_errors_are_bind_time_diagnostics -- --nocapture`
- `cargo test -p oxvba-bind error::tests::named_argument_not_found_has_stable_code -- --nocapture`
- `cargo test -p oxvba-bind -- --nocapture`
- `git diff --check`
- `br dep cycles --json`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-governance.ps1`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\meta-check.ps1 -Fast -NoArtifacts`

## Boundary

This is the early-bound COM named-argument slice of `FE-7.6.a`. It does not close broader imported
COM activation/member breadth, arbitrary Office host matrices, or runtime COM transport parity
beyond the descriptor-backed argument binding rows tested here. Those lanes remain `in-progress`.

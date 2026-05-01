# RuntimeValue Presentation DTO Split Evidence

Date: 2026-05-01
Bead: `bd-pn5i.6` / `cleanout-005`
Workset: `WORKSET_2026-04-30_RUNTIMEVALUE_IR_STUB_CLEANOUT.md`

## Outcome

Launcher, web, and language-service presentation surfaces no longer depend on
`RuntimeValue` for their result data shape.

- `oxvba-launcher` now executes VM/JIT snapshots through retained `Variant`
  APIs and projects the output into a local `LauncherSnapshotValue` presentation
  DTO before printing.
- `oxvba-web-host` already exposes explicit web DTOs (`WebImmediateOutput`,
  `WebDebugValue`, `WebDebugPauseState`, etc.) over host variant DTOs; no
  `RuntimeValue` imports remain.
- `oxvba-web-shell` consumes variant snapshots/evaluation outputs and projects
  them through web DTOs; no `RuntimeValue` imports remain.
- `oxvba-languageservice` host-session tests were moved from embedded
  RuntimeValue invocation helpers to `EmbeddedInvokeProcedureVariantRequest` and
  `invoke_procedure_variant`, so language-service code and tests use retained
  `Variant` result assertions.

## Search evidence

```text
rg -n "RuntimeValue" crates/oxvba-launcher/src crates/oxvba-web-host/src crates/oxvba-web-shell/src crates/oxvba-languageservice/src --glob '*.rs'
```

Result: no matches.

## Verification

Passed:

```text
cargo fmt --all
cargo test -p oxvba-launcher -p oxvba-web-host -p oxvba-web-shell -p oxvba-languageservice
```

This slice does not close the umbrella RuntimeValue search gate; residual
compatibility bridges in runtime/HAL/COM/VM/JIT/host are tracked by
`cleanout-007`.

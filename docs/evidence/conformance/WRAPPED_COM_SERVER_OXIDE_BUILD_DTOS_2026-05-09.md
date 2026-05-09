# WrappedComServer OxIde build DTO evidence

Date: 2026-05-09
Bead: `bd-wcs1.9.3`
Matrix rows: `COM-0007`, `COM-0008`, `COM-0009`, `COM-0010`, `PH-0011`

## Scope

This evidence covers the typed direct-host planning/result surface that OxIde
can consume for `BuildTarget=WrappedComServer` without parsing CLI text.

This is a DTO and planning surface. It does not by itself claim a new wrapped
COM runtime capability beyond the existing COM evidence rows.

## Commands

```powershell
cargo test -p oxvba-host wrapped_com_server_build_plan_reports_artifacts_and_registration_dtos --quiet
cargo test -p oxvba-host embedded_build_run_ids_events_and_command_status_are_correlated --quiet
cargo check -p oxvba-host --quiet
```

## Verified behavior

- `EmbeddedBuildRequest` now carries an explicit `EmbeddedBuildTarget`.
- `EmbeddedBuildTarget::WrappedComServer` has a canonical DTO spelling of
  `WrappedComServer`.
- `EmbeddedBuildRunHost::build_plan` returns a typed `EmbeddedBuildPlan`.
- WrappedComServer build plans include the expected artifact set: `.oxb`,
  `.dll`, `.tlb`, registration plan, and optional build log.
- WrappedComServer build plans publish required tool names for the host UI:
  `rustc`, `cargo`, and `windows-sdk`.
- WrappedComServer build plans publish a `EmbeddedComServerCapabilityProfile`
  with Windows availability, bitness, toolchain, and supported registration
  scopes.
- WrappedComServer build plans publish a default per-user registration plan that
  does not require administrative rights and names the `DllRegisterServer`
  action hint.
- `EmbeddedBuildResult` carries the build plan plus direct `dll_path`,
  `tlb_path`, and registration-plan fields so hosts do not need to infer them
  from output text.
- Existing embedded build/run request IDs, event correlation, and command
  availability tests remain green after adding the target-aware build request
  shape.

## Residual

This DTO surface is an implemented subset for OxIde/direct-host planning. It
does not execute the wrapper compiler itself from `oxvba-host`, does not perform
registration from the embedded facade, and does not replace the wrapped COM
runtime evidence already tracked under `COM-0007` through `COM-0010`.

2026-05-10 review correction: this residual is not merely future polish.
`EmbeddedBuildRunHost::build_workspace` currently returns a successful
`EmbeddedBuildResult` from source compilation and exposes planned `dll_path` and
`tlb_path` values without running the wrapped COM wrapper build or verifying that
those artifacts exist. The direct-host build-result lane is reopened under
`bd-wcs1.9.3`, with residual delivery tracked by `bd-wcs1.9.4`.

# WrappedComServer OxIde build DTO evidence

Date: 2026-05-10
Bead: `bd-wcs1.9.3`, `bd-wcs1.9.4`
Matrix rows: `COM-0007`, `COM-0008`, `COM-0009`, `COM-0010`, `PH-0011`

## Scope

This evidence covers the typed direct-host surface that OxIde can consume for
`BuildTarget=WrappedComServer` without parsing CLI text:
1. typed planning DTOs,
2. typed build result fields,
3. actual wrapped COM build execution and artifact verification.

## Commands

```powershell
cargo test -p oxvba-host wrapped_com_server_build_plan_reports_artifacts_and_registration_dtos --quiet
cargo test -p oxvba-host wrapped_com_server_build_workspace_requires_disk_only_source_policy --quiet
cargo test -p oxvba-host embedded_build_run_ids_events_and_command_status_are_correlated --quiet
cargo test -p oxvba-host embedded::tests --quiet
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
- `EmbeddedBuildRunHost::build_workspace` now executes the wrapped COM build
  pipeline for `EmbeddedBuildTarget::WrappedComServer` on Windows
  `DiskOnly` requests by:
  - compiling/serializing the `.oxb` artifact,
  - invoking the wrapped COM build lane through `cargo run -p oxvba-cli -- build`,
  - materializing a registration-plan artifact,
  - verifying required artifacts (`.oxb`, `.dll`, `.tlb`, registration JSON)
    before returning `EmbeddedBuildStatus::Succeeded`.
- `EmbeddedBuildRunHost::build_workspace` now returns
  `EmbeddedBuildStatus::Failed` with typed diagnostics for non-Windows hosts,
  non-`DiskOnly` requests, failed build commands, or missing artifacts.
- Existing embedded build/run request IDs, event correlation, and command
  availability tests remain green after adding the target-aware build request
  shape.

## Residual

This DTO surface remains an implemented subset:
1. embedded facade execution currently requires
   `EmbeddedExecutionSourcePolicy::DiskOnly` for WrappedComServer builds;
2. the direct-host API still exposes a registration plan artifact, but it does
   not perform registry mutation itself;
3. COM runtime parity claims still come from `COM-0007` through `COM-0010`
   evidence rows.

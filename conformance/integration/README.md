# project integration suite

Data-driven multi-module and multi-project integration fixtures for OxVba.

## layout

- `catalog.psv`: tracked suite catalog (status, profile/policy, expected behavior, deferred links).
- `projects/<CASE_ID>/main/*.bas`: active project modules. Filename format is `<ModuleName>.<kind>.bas`.
- `projects/<CASE_ID>/references/<ProjectName>/*.bas`: referenced project modules for cross-project cases.

Module kinds:
- `proc`
- `class`
- `document`
- `form`
- `extension`

## status model

- `active`: executed in suite runs; expected to pass.
- `active-limit`: executed and expected to fail in a known/intentional way (tracked limit).
- `deferred`: tracked but not executed yet.
- `planned`: cataloged for upcoming implementation.

## execution

```powershell
./scripts/run-project-integration-suite.ps1
./scripts/run-project-integration-suite.ps1 -CasePattern INTP-005
```

The runner executes `cargo test -p oxvba-host --test project_integration_suite` and writes run artifacts under `docs/evidence/conformance/project_integration/`.

Additional mixed end-to-end pressure/edge lane:

```powershell
cargo test -p oxvba-host --test end_to_end_mix
```

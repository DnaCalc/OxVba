# In-Process Host API

Date: 2026-05-24
Bead: `bd-hjys.5`
Workset: `docs/worksets/WORKSET_2026-05-24_HOST_PROJECT_CALLABLE_REFLECTION_AND_WRAPPER_GENERATION_REWORK.md`
Contract source: `docs/evidence/host_callable/NEUTRAL_DESCRIPTOR_MODEL.md`

## Implementation summary

Added the first-pass neutral in-process host facade in `crates/oxvba-host/src/vba_host.rs` and re-exported it from `oxvba-host`:

- `VbaHost`
- `VbaHostOptions`
- `ProjectSource`
- `ProjectModuleText`
- `ProjectFileSet`
- `ProjectFile`
- `LoadedVbaProject`
- `PreparedVbaProject`
- `HostDiagnostic`
- `HostDiagnosticPhase`

Supported load sources:

- `ProjectSource::ModuleTexts(Vec<ProjectModuleText>)` for in-memory module text.
- `ProjectSource::FileSet(ProjectFileSet)` for path-based module loading.
- `ProjectSource::BundleBytes(Vec<u8>)` and `VbaHost::load_bundle(&[u8])` for bundle bytes.

Lifecycle:

1. `VbaHost::load_project(...)` or `VbaHost::load_bundle(...)` returns `LoadedVbaProject`.
2. `LoadedVbaProject::reflection()` exposes neutral `ProjectReflection` before preparation.
3. `LoadedVbaProject::prepare()` returns `PreparedVbaProject`.
4. `PreparedVbaProject::reflection()` exposes prepared-session reflection.
5. `PreparedVbaProject::invoke_by_name_variant(...)` provides a first-pass invocation-after-prepare check using the existing module/procedure invocation path. Neutral `CallableId` invocation remains scoped to `bd-hjys.6`.

## Acceptance coverage

| Acceptance criterion | Evidence |
| --- | --- |
| VbaHost-style API supports in-memory module text load. | `vba_host_facade_tests::vba_host_loads_in_memory_reflects_before_prepare_and_invokes_after_prepare` loads `ProjectSource::ModuleTexts`. |
| File/path or loader-provided blob load is supported or explicitly scoped. | `vba_host_facade_tests::vba_host_loads_file_set` loads `ProjectSource::FileSet` from an on-disk `.bas` file. Bundle bytes cover loader-provided blob loading. |
| Bundle byte load is supported. | `vba_host_facade_tests::vba_host_loads_bundle_bytes_and_invokes_after_prepare` serializes an `OxBundle` and loads it through `VbaHost::load_bundle(&bytes)`. |
| Reflection before prepare and invocation after prepare are both covered. | In-memory and bundle tests assert `LoadedVbaProject::reflection()` before `prepare()` and successful `PreparedVbaProject::invoke_by_name_variant(...)` after preparation. |
| Multiple loaded projects remain isolated. | `vba_host_facade_tests::vba_host_loaded_projects_remain_isolated` loads two projects through one `VbaHost`, checks distinct descriptor module IDs, prepares both, and invokes each independently. |
| Evidence artifact required. | This file: `docs/evidence/host_callable/IN_PROCESS_HOST_API.md`. |

## Checks run

```text
cargo fmt
cargo test -p oxvba-host --test vba_host_facade_tests -- --nocapture
cargo check --workspace --all-targets
```

Results: all passed.

## Fresh-eyes review notes

- The facade uses neutral `ProjectReflection`; it does not expose new `HostUdf*` APIs.
- File loading is concrete path loading, not a TODO/proxy.
- Bundle loading consumes `OxBundle::project_reflection()` and therefore uses descriptor inventory rather than reparsing source.
- `invoke_by_name_variant` is deliberately named as a by-name first-pass bridge; callable-ID/context-aware invocation remains for `bd-hjys.6`.

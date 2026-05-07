# DNA OxIde ThinSliceHello Fixture Evidence (2026-05-07)

Bead: `bd-avdu.6.1`

This evidence records the OxVba-side fixture ladder for the DnaOxIde direct-host integration handoff.

## Fixture

Implemented in:

- `crates/oxvba-languageservice/tests/dnaoxide_thin_slice_hello.rs`

The fixture creates temp `.basproj` copies at test time rather than mutating repository fixtures.

## Covered direct-host seams

- Workspace load through `HostWorkspaceSession::load_workspace_path`
- Editor overlay through `HostWorkspaceSession::set_document_text`
- Roster overlay/version signal through `workspace_roster`
- Overlay build through `EmbeddedBuildRunHost::build_workspace`
- Runtime session creation through `EmbeddedBuildRunHost::run_project`
- Immediate attach through `EmbeddedRunSession::into_immediate_session`
- Immediate evaluation over overlay source through `ImmediateSession`
- Debug attach through `EmbeddedRunSession::into_debug_session`
- Debug watch registry/evaluation through `DebugSession::add_watch` and `evaluate_watches`
- Debug breakpoint binding DTO through `DebugSession::set_source_breakpoint`
- Stable frame/watch/breakpoint/runtime IDs in the returned DTOs
- Broken COM reference state through `ComSelectionService::inspect_workspace_project_state`
- COM runtime availability/capability DTOs through `ComSelectionService::capability_profile`

## Validation

Latest local validation:

```text
cargo test -p oxvba-languageservice dnaoxide_thin_slice_hello --quiet
```

Result: pass, 2 fixture tests passed.

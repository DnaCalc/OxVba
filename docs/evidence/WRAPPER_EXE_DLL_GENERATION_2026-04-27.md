# Wrapper EXE/DLL Generation Evidence

Date: 2026-04-27
Beads: `bd-wrap1.3`, `bd-wrap1.4`

## Scope

Record the current wrapper executable and wrapper DLL/shared-library generation
substrate over canonical `.oxb` bundles.

## Implementation State

- `crates/oxvba-build/src/exe.rs` generates executable shim source that embeds
  the compiled `.oxb` bundle with `include_bytes!`, deserializes `OxBundle`, and
  executes it through `oxvba_host::Engine`.
- `crates/oxvba-build/src/dll.rs` generates `cdylib` shim source with exported
  C ABI functions, shared prepared runtime session state, and typed
  `NativeExportDescriptor` handoff for module/procedure invocation.
- `crates/oxvba-build/src/compile.rs` stages generated shim source into a
  temporary Cargo project and supports EXE and DLL output kinds.

## Validation

Command:

```powershell
cargo test -p oxvba-build --lib -- --nocapture
```

Result: pass, 31/31.

Relevant rows:

- `exe::tests::exe_shim_contains_project_name`
- `dll::tests::dll_shim_generates_export`
- `dll::tests::dll_shim_sub_has_no_return`

## Remaining Work

This closes the generation/source substrate beads only. `bd-wrap1.5` remains
the next delivery bead for launch/config/reference behavior and metadata handoff
validation over wrapper outputs.

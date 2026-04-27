# V0.2 Host Observation Compat Progress

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.2.3`
Status: complete

## Change

This slice moved host-facing observation compatibility projection behind an
explicit host adapter boundary while preserving the retained `Variant` surfaces
as the normal execution contract.

Implemented:

- Added `oxvba_host::compat` as the explicit adapter for legacy
  `RuntimeValue` snapshots, session slot reads, bundle/source/project snapshot
  projections, and retained-`Variant` to legacy-value conversion.
- Rewired `ProjectRuntimeSession`, `ImmediateSession`, and `Engine` legacy
  snapshot methods to delegate to `oxvba_host::compat` instead of performing
  projection inline on the primary host types.
- Kept `Engine::execute_source` on the retained `Variant` execution path.
- Exported retained debugger and immediate-session variant result types from
  `oxvba_host` so downstream clients do not need the legacy debugger result
  shapes for ordinary observation.
- Moved CLI `--dump-slots` formatting into a named
  `format_compat_slot_dump` adapter. CLI execution continues to obtain retained
  `Variant` snapshots first; only the slot dump flag invokes compatibility
  projection.
- Migrated project hosting examples and the project integration suite to run
  through retained `Variant` project/source snapshot APIs. The integration
  suite still projects variants to legacy expected slots only inside a named
  expectation adapter.

`bd-bqm8.2.3` is complete for host, immediate, CLI, debugger export, and
project integration observation boundaries. The parent `bd-bqm8.2` remains
in-progress because COM/HAL compatibility bridges and the broader
test/conformance/doc normalization pass remain open under `bd-bqm8.2.4` and
`bd-bqm8.2.5`.

## Verification

Passed:

- `cargo fmt --check`
- `cargo check -p oxvba-host -p oxvba-cli`
- `cargo test -p oxvba-host --test project_hosting_examples_end_to_end`
- `cargo test -p oxvba-host --test project_integration_suite`
- `cargo test -p oxvba-host --test debug_session_host_harness`
- `cargo test -p oxvba-host immediate_session_snapshot_compat_values_projects_runtime_state --lib`
- `cargo test -p oxvba-host execute_source_returns_slot_snapshot --lib`

## Remaining Downstream Work

- `bd-bqm8.2.4`: reconcile COM/HAL legacy compatibility bridges.
- `bd-bqm8.2.5`: migrate remaining tests, conformance notes, and docs that
  still normalize legacy `RuntimeValue`/slot-shaped observation as ordinary
  execution truth.

# VM3 W6 GetObject Evidence

Date: 2026-07-02
Bead: `bd-9sed.7`
Plan: `docs/VM3_COMPLETION_AND_VM2_RETIREMENT_PLAN.md#W6`

## Outcome

W6 `GetObject` is green on vm3:

- `GetObject("", "Scripting.Dictionary")` creates a new live COM instance through the same activation path as `CreateObject`.
- `GetObject(, "Excel.Application")` binds a running Excel instance through the ROT. The committed live test creates and cleans up its own Excel instance.
- `GetObject(pathname)` binds a temporary `.xlsx` workbook file through the file-moniker path and reads a saved cell value.
- Null and Replay COM adapters now decline `get_object` with the normal capability-unavailable error instead of falling through the generic retained-variant companion fallback.
- The HAL trait docs now describe the live VBA error targets: 429 for running/create activation failure, 432 for file bind failure.

This keeps real VBA behavior as the target. Legacy OxVBA fallback behavior is not preserved as a compatibility lane.

## Checks

- `cargo fmt --all`
- `cargo test -p oxvba-symbol catalog_covers_every_native_impl_id -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip getobject -- --nocapture`
- `cargo test -p oxvba-hal null_backend_rejects_host_sensitive_domains -- --nocapture`
- `cargo test -p oxvba-hal replay_variant_companions_consume_journal_without_trait_projection -- --nocapture`
- `cargo test -p oxvba-host --test com_matrix_getobject --no-run`
- `cargo test -p oxvba-host --test com_matrix_getobject -- --ignored --test-threads=1 --nocapture`
- `cargo test -p oxvba-differential --test getobject_vm3 -- --nocapture`
- `cargo check -p oxvba-com -p oxvba-hal -p oxvba-lib -p oxvba-bind -p oxvba-host -p oxvba-vm3`

## Cleanup

`Get-Process EXCEL -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,MainWindowTitle` produced no Excel processes after the live matrix run.

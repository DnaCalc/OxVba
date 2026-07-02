# VM3 W5 COM-Foreign Legs Evidence

Date: 2026-07-02
Bead: `bd-9sed.6`
Plan: `docs/VM3_COMPLETION_AND_VM2_RETIREMENT_PLAN.md#W5`

## Outcome

The scoped W5 vm3 COM-foreign rows now run on vm3 for:

- G2-COM: `For Each` over a live Excel COM collection via `IEnumVARIANT`.
- G5: COM `WithEvents` delivery for the in-proc fixture and out-of-process Excel connection points.
- G7: bounded synchronous `CallWindowProcW(AddressOf ...)`, already closed by `bd-9sed.6.1`.

The implementation avoids treating legacy OxVBA fallbacks as compatibility targets:

- library-wide typelib metadata no longer exposes broad source-event bags;
- scoped typelib requests that miss do not fall back to whole-library members/events;
- provider closure expansion starts from used coclasses and normalized same-library followups;
- late-bound COM calls without ByRef writeback use the real dynamic `IDispatch` path instead of forcing metadata fallback;
- typed COM zero-arg collection properties such as `Workbook.Worksheets(1)` bind as VBA does: property get, then default member on the returned collection.

## Checks

- `cargo fmt --all`
- `cargo check -p oxvba-com -p oxvba-symbol -p oxvba-bind -p oxvba-hal -p oxvba-vm3 -p oxvba-host`
- `cargo test -p oxvba-com typelib_metadata -- --nocapture`
- `cargo test -p oxvba-symbol com_events_scope_to_receiver_coclass_without_library_fallback -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip withevents_com_source_ignores_events_from_other_coclasses -- --nocapture`
- `cargo test -p oxvba-host --test com_matrix_collections --no-run`
- `cargo test -p oxvba-host --test com_matrix_events --no-run`
- `cargo test -p oxvba-host --test com_matrix_collections c6_vm3_excel_for_each_worksheets_oop_enum -- --ignored --exact --test-threads=1 --nocapture`
- `cargo test -p oxvba-host --test com_matrix_events v3_on_pair_changed_arg_order_pin -- --ignored --exact --test-threads=1 --nocapture`
- `cargo test -p oxvba-host --test com_matrix_events v7_excel_new_workbook_oop_event -- --ignored --exact --test-threads=1 --nocapture`
- `cargo test -p oxvba-host --test com_matrix_events v8_excel_sheet_change_oop_object_arg -- --ignored --exact --test-threads=1 --nocapture`
- `cargo test -p oxvba-host --test com_matrix_events -- --ignored --test-threads=1 --nocapture`

## Boundary

V11 remains a documented fixture gap for ByRef COM event arguments; it is not hidden inside this closure claim.

Async or externally-owned native callback lifetimes beyond the bounded `CallWindowProcW` slice are split to `bd-9sed.17`.

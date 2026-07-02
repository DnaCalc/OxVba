# COM Event Source Scoping Evidence

Date: 2026-07-02
Bead: `bd-aprs.8.8.5` under `bd-aprs.8.8`
Worksets:
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
- `docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`

## Outcome

COM event-source lookup now scopes events to the receiver coclass before binding direct COM event
members or building `WithEvents` routes.

A library-wide typelib blob can still describe many coclasses, and untagged event metadata remains
visible when the metadata gives no coclass-specific basis for exclusion. When the metadata does tag
events by coclass, however, a typed receiver such as `Excel.Workbook` no longer inherits
`Excel.Application` events through the library-wide event list. The previous full-event-set fallback
after an empty coclass filter has been removed because it masked non-VBA phantom routes.

## Regression Shape

- `ComTypeLibProvider::source_events` returns an empty event list for a known receiver coclass when
  all tagged events belong to another coclass.
- `ComTypeLibProvider::resolve_member` uses the same scoped event set, so direct COM event-member
  lookup does not find unrelated coclass events.
- A `Private WithEvents wb As Excel.Workbook` sink with a handler named `wb_NewWorkbook` does not
  synthesize an event route from an `Excel.Application` `NewWorkbook` event.
- If a broad library provider owns the receiver coclass but has an empty scoped event result, a
  later scoped provider for the same coclass can still supply the real event route.
- Existing COM-source `WithEvents` route coverage for the `OxVba.TestEventServer` fixture still
  emits the intended route when the source type and handler match.
- Existing cross-project `WithEvents` routing is unchanged.

## Checks

- `cargo test -p oxvba-symbol com_events_scope_to_receiver_coclass_without_library_fallback -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip withevents_com_source_ignores_events_from_other_coclasses -- --nocapture`
- `cargo test -p oxvba-symbol com_event -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip withevents_com_source -- --nocapture`
- `cargo test -p oxvba-bind --test cross_project cross_project_withevents -- --nocapture`
- `cargo test -p oxvba-symbol -- --nocapture`
- `cargo clippy -p oxvba-symbol --tests -- -D warnings`
- `cargo clippy -p oxvba-bind --tests -- -D warnings`
- `cargo test -p oxvba-bind -- --nocapture`
- `cargo fmt --all --check`
- `git diff --check`
- `br dep cycles --json`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-governance.ps1`

## Boundary

This closes the COM event-source coclass scoping slice for frontend binding. It does not close the
full Excel/Office event matrix, runtime connection-point breadth, or every imported
member/property/default-member row under `bd-aprs.8.8`/`IP-08B`.

No Excel oracle run was needed for this slice; the target behavior is the VBA object-library shape
already represented by coclass-tagged typelib metadata.

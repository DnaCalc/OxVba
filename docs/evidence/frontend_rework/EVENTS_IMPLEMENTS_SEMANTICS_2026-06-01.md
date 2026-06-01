# Events and Implements Semantics Evidence

Date: 2026-06-01
Bead: `bd-aprs.8.5`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

This bead was initially only partially executed: the first pass added
`crates/oxvba-compiler/src/frontend_event_semantics.rs`, a typed routing surface for WithEvents,
RaiseEvent, event handler matching, Implements, and related diagnostics, but production event and
Implements behavior still lived in `project.rs` string scans and rewrites.

Current continuation progress:

- `ProjectSymbolIndex` now distinguishes declared events from ordinary class fields with
  `ProjectSymbolKind::Event`.
- Active-project event dispatch planning now gates each `WithEvents` handler route through the
  frontend event route and frontend handler procedure route before emitting guard wrappers.
- Referenced-project and imported typelib event sources remain compatibility paths because the
  active `ProjectSymbolIndex` is not yet composed across referenced projects/type libraries.

## Checks

- `cargo test -p oxvba-compiler frontend_event_semantics --quiet`
- `cargo test -p oxvba-compiler frontend_project_symbols --quiet`
- `cargo test -p oxvba-compiler event_dispatch_plan_requires_frontend_event_and_handler_routes --quiet`
- `cargo test -p oxvba-compiler event --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- Routes are symbol-based and do not depend on generated source text.
- Stable diagnostics are present for missing event handlers and missing Implements members.
- The first-run state was scaffold-only. The active-project event-dispatch plan now consumes
  frontend route facts, but WithEvents declaration binding, RaiseEvent diagnostics/rewrites, and
  Implements coverage still require production migration or explicit quarantine before FE-7.5 can
  close.

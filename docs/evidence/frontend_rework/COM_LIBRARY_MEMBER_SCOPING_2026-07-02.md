# COM Library Member Scoping Evidence

Date: 2026-07-02
Bead: `bd-aprs.8.8.4` under `bd-aprs.8.8`
Worksets:
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
- `docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`

## Outcome

Library-wide typelib providers now distinguish:

- type names the referenced library knows for activation/type-error purposes, and
- type names whose members are actually scoped by that metadata blob.

A flat library-wide blob can still make `Dictionary`, `Excel.Application`, or `Excel.Workbook`
known COM object types, but it no longer lets every coclass/interface see every member in the flat
library member list. Source-used COM type names and named `InterfacePointer` returns are resolved
through scoped same-library requests before member binding.

## Regression Shape

- A full library reference with `Application` and `Workbook` type names no longer binds
  `Workbook.Workbooks` through the `Application.Workbooks` descriptor.
- `Dim app As Application: app.Workbooks.Count` still binds early through the source-used
  `Application` scoped provider and the returned `Workbooks` scoped provider.
- `Application.Workbooks.Count` still binds early for host-injected `Excel.Application`.
- Generic COM `Object` returns remain late-bound.
- Direct provider tests now verify that a flat library-level blob can resolve bare and qualified
  coclass activation names without answering member lookups for those coclasses.

## Checks

- `cargo test -p oxvba-symbol library_level_coclass_resolves_bare_and_qualified_names -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip library_wide_com_member_lookup_is_scoped_to_receiver_type -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip com_return -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip generic_com_object_return_stays_late_bound -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip typed_com_receiver_member_call_lowers_to_early_com -- --nocapture`
- `cargo test -p oxvba-symbol -- --nocapture`
- `cargo clippy -p oxvba-symbol --tests -- -D warnings`
- `cargo clippy -p oxvba-bind --tests -- -D warnings`
- `cargo test -p oxvba-bind -- --nocapture`
- `cargo fmt --all --check`
- `git diff --check`
- `br dep cycles --json`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-governance.ps1`

## Boundary

This closes the library-wide COM member-leak slice for frontend binding. It does not close the full
Excel/Office object model, runtime COM transport parity, event connection-point breadth, or every
imported member/property/default-member row under `bd-aprs.8.8`/`IP-08B`.

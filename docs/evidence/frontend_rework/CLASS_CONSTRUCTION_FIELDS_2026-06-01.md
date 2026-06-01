# Class Construction and Fields Evidence

Date: 2026-06-01
Bead: `bd-aprs.8.4`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

This bead was initially only partially executed: the first pass added
`crates/oxvba-compiler/src/frontend_class_semantics.rs`, a typed route model for class
construction and object fields, but production class construction was still principally
authorised by legacy `project.rs` resolver/rewrite code.

The model records:

- `New` expression construction;
- `As New` lazy construction;
- predeclared instance routes;
- ordinary fields;
- WithEvents fields;
- runtime object-field metadata: slot, refcounted object storage, As New initialization, and
  WithEvents flag.

The reopened pass moved active-project class construction route selection into the new
front-end symbol index:

- `ProjectSymbolIndex::resolve_class_route` is now the authority that a name is an
  active-project class for `Dim x As New ClassName`.
- The same frontend class route authorises explicit `Set x = New ClassName`.
- Runtime/procedure identity still uses the manifest module id after frontend authorisation, so
  `Attribute VB_Name` spelling and manifest module identity do not drift.
- Active-project predeclared property read routes now require a frontend class route plus a
  frontend Property Get accessor route before the legacy backend text rewrite may emit the
  lowered property call.
- Dynamic object field-token emission receives the same frontend symbol index and filters ordinary
  field tokens through frontend-confirmed class field names when available; legacy source parsing
  remains the compatibility fallback for parser-incomplete modules and for excluding non-ordinary
  declarations such as `WithEvents`.

Production-route proof:

- `expand_bound_source_line_uses_frontend_class_route_for_active_project_new` uses a fixture where
  the public class name is supplied by `Attribute VB_Name = "Widget"` while the manifest module id
  is `WidgetFile`. The legacy active-project resolver only matches manifest module names; the test
  proves both `Dim widget As New Widget` and `Set other = New Widget` resolve through the frontend
  class route and then map back to the runtime module id `widgetfile`.
- `cargo test -p oxvba-compiler predeclared --quiet` covers the existing predeclared/default-root
  matrix after adding the frontend route gate.
- Existing host/runtime tests prove the bd-1ufc field/lifetime behavior remains executable:
  per-instance ordinary field storage, object-reference field teardown cascades, and class
  construction/reference-counted object identity all still pass.

Compatibility quarantine / residual classification:

- Referenced-project class construction and referenced-project predeclared roots still fall back to
  `resolve_interface_module` / procedure metadata because the current `ProjectSymbolIndex` is built
  for the active manifest only. This is classified as an out-of-scope compatibility route for
  FE-7.4 and should be migrated when reference-project symbol-index composition lands.
- Imported COM `As New` / `New` remains on the typelib metadata route, not the active-project class
  route.
- The legacy line parser remains as a field-token compatibility fallback when the new syntax
  collector cannot parse a module body or when declaration text is needed to exclude `WithEvents`
  from ordinary field storage.

## Checks

- `cargo test -p oxvba-compiler frontend_class_semantics --quiet`
- `cargo test -p oxvba-compiler frontend_project_symbols --quiet`
- `cargo test -p oxvba-compiler expand_bound_source_line_uses_frontend_class_route_for_active_project_new --quiet`
- `cargo test -p oxvba-compiler compile_project_internal_dynamic_routes_do_not_keep_transitional_token_table --quiet`
- `cargo test -p oxvba-compiler compile_project_ --quiet`
- `cargo test -p oxvba-compiler predeclared --quiet`
- `cargo test -p oxvba-host pure_oxvba_class --quiet`
- `cargo test -p oxvba-host pure_oxvba_class_fields_are_per_instance_storage --quiet`
- `cargo test -p oxvba-host pure_oxvba_class_terminate_cascades_through_object_field --quiet`
- `cargo check -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- An initial attempt to reuse `ObjectMemberKindDescriptor::Field` was wrong because that descriptor
  has no field variant. The bead now uses a local `ClassMemberRuntimeKind::Field` and keeps existing
  object descriptors only where they actually fit.
- The metadata shape preserves the bd-1ufc requirement that object fields have explicit runtime
  layout/lifetime facts rather than implicit handle integers.
- The production migration must not confuse frontend `VB_Name` spelling with runtime module ids.
  The new route proof deliberately uses different names and verifies that frontend authorisation
  maps back to the manifest module id used by procedure lowering and dynamic-object metadata.
- FE-7.4 is still not a full parser deletion. The remaining fallback routes are now explicit:
  active-project class construction is frontend-authorised, while referenced-project classes,
  imported COM activation, and parser-incomplete field enumeration are compatibility paths rather
  than hidden closure claims.

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
- Continuation work split ordinary class fields from `WithEvents` fields in the frontend project
  symbol table. `Private WithEvents source As T` is now collected as the `source`
  `WithEventsField` route rather than accidentally recording the keyword as a field name, and
  active-project dynamic object routes now emit ordinary field tokens directly from frontend field
  routes before falling back to legacy line parsing for referenced-project or parser-incomplete
  cases.
- The typed-local object classification path now uses the same frontend class-construction route
  for active-project `Dim x As ClassName` declarations. The previous `resolve_interface_module`
  check remains only as the fallback path when no active-project frontend class route is available.
- `As New` execution coverage now includes reset-to-`Nothing` re-instantiation for both local
  object slots and class fields. The binder emits slot metadata rather than eager construction,
  and VM3 lazily constructs a fresh instance on the next read after the slot is cleared.
- 2026-07-06 package/runtime metadata continuation: OxIR class descriptor verification now enforces
  that class lifecycle hooks and method/property descriptor targets start with the hidden `Me`
  receiver parameter. VM runtime member descriptor extraction and project-member argument name
  mapping skip only that explicit receiver, preserving source-visible parameter metadata if
  malformed hand-built OxIR bypasses verification.
- The same continuation now rejects class metadata kind drift: lifecycle hooks must target `Sub`
  procedures, property descriptor rows must target the matching `PropertyGet`/`PropertyLet`/
  `PropertySet` procedure kind, and ordinary method rows remain valid for either `Sub` or
  `Function`.
- The package contract now also rejects malformed class property setter descriptors whose
  `PropertyLet`/`PropertySet` target lacks a final source-visible value parameter after hidden
  `Me`, or stores that value parameter as ByRef. This preserves the binder's runtime-ByVal setter
  value lowering in OxIR metadata without claiming full `PropertySet` object-type compatibility.
- Default-member and `_NewEnum` class metadata now receive the same package-level consistency
  check: default rows may span Property Get/Let/Set for one logical member but not multiple names,
  explicit default DISPIDs must be `0`, enumerator rows must be unique, get/method-shaped,
  zero-visible-arg entries, and explicit enumerator DISPIDs must be `-4`.

Production-route proof:

- `expand_bound_source_line_uses_frontend_class_route_for_active_project_new` uses a fixture where
  the public class name is supplied by `Attribute VB_Name = "Widget"` while the manifest module id
  is `WidgetFile`. The legacy active-project resolver only matches manifest module names; the test
  proves both `Dim widget As New Widget` and `Set other = New Widget` resolve through the frontend
  class route and then map back to the runtime module id `widgetfile`.
- `project_symbol_index_resolves_class_routes_and_field_names` now covers the ordinary-field vs
  `WithEventsField` split in the frontend symbol index.
- `compile_project_dynamic_field_tokens_use_frontend_ordinary_field_routes` proves active-project
  dynamic object metadata uses frontend ordinary field routes and excludes `WithEvents` bindings
  from ordinary field-token storage.
- `record_internal_class_object_local_uses_frontend_class_route` uses a deliberately different
  manifest module id and `VB_Name` class spelling to prove typed object-local classification is
  accepted by the frontend class route rather than by manifest-name-only lookup.
- `as_new_local_reinstantiates_after_set_nothing` and
  `as_new_field_reinstantiates_after_set_nothing` prove `Dim x As New T` and `Private x As New T`
  retain their lazy-construction slot metadata after `Set x = Nothing`; the next read creates a
  distinct initialized instance.
- `cargo test -p oxvba-compiler predeclared --quiet` covers the existing predeclared/default-root
  matrix after adding the frontend route gate.
- Existing host/runtime tests prove the bd-1ufc field/lifetime behavior remains executable:
  per-instance ordinary field storage, object-reference field teardown cascades, and class
  construction/reference-counted object identity all still pass.
- `verifier_accepts_class_method_with_hidden_me_receiver`,
  `verifier_catches_class_lifecycle_without_hidden_me_receiver`, and
  `verifier_catches_class_method_without_hidden_me_receiver` prove the OxIR package contract
  distinguishes the hidden class receiver from source-visible member arguments.
- `runtime_member_params_skips_only_explicit_hidden_me_receiver` proves runtime descriptor metadata
  hides only the verified `Me` receiver; the same helper also drives project-member named-argument
  mapping so malformed descriptor input does not drop the first visible argument.
- `project_member_named_args_skip_only_explicit_hidden_me_receiver` proves the VM project-member
  named-argument mapper resolves names against source-visible parameters after the explicit
  receiver, not against a blind positional skip.
- `verifier_catches_class_lifecycle_proc_kind_mismatch` and
  `verifier_catches_class_member_proc_kind_mismatch` prove class descriptors cannot claim a
  lifecycle/member/property shape that disagrees with the target procedure descriptor.
- `verifier_accepts_class_property_setter_byval_value_param`,
  `verifier_catches_class_property_setter_without_value_param`, and
  `verifier_catches_class_property_setter_byref_value_param` prove class `PropertyLet`/
  `PropertySet` descriptors preserve VBA's trailing setter value parameter as runtime ByVal.
- `verifier_accepts_default_property_pair_and_enumerator_metadata`,
  `verifier_catches_ambiguous_class_default_member_names`,
  `verifier_catches_default_member_nonzero_dispid`,
  `verifier_catches_duplicate_class_enumerator_members`,
  `verifier_catches_class_enumerator_setter_kind`,
  `verifier_catches_class_enumerator_wrong_dispid`, and
  `verifier_catches_class_enumerator_visible_params` prove hand-built OxIR cannot publish
  contradictory default-member or `_NewEnum` descriptor flags that the VM and COM-facing
  descriptor layer would otherwise consume directly.

Compatibility quarantine / residual classification:

- Referenced-project class construction and arbitrary referenced-class member ownership still fall
  back to `resolve_interface_module` / procedure metadata because the current `ProjectSymbolIndex`
  is built for the active manifest only. Later project-boundary work admits mixed predeclared
  document/class roots plus unused procedural helpers through the full-source HIR route, but used
  referenced procedural helpers and reference-project class-construction/member ownership remain
  out-of-scope compatibility routes for FE-7.4 until reference-project symbol-index composition
  lands.
- Imported COM `As New` / `New` remains on the typelib metadata route, not the active-project class
  route.
- The legacy line parser remains as a field-token compatibility fallback when the new syntax
  collector cannot parse a module body or when the route belongs to a referenced project outside
  the active-project symbol index.

## Checks

- `cargo test -p oxvba-compiler frontend_class_semantics --quiet`
- `cargo test -p oxvba-compiler frontend_project_symbols --quiet`
- `cargo test -p oxvba-compiler compile_project_dynamic_field_tokens_use_frontend_ordinary_field_routes --quiet`
- `cargo test -p oxvba-compiler expand_bound_source_line_uses_frontend_class_route_for_active_project_new --quiet`
- `cargo test -p oxvba-compiler record_internal_class_object_local_uses_frontend_class_route --quiet`
- `cargo test -p oxvba-compiler compile_project_internal_dynamic_routes_do_not_keep_transitional_token_table --quiet`
- `cargo test -p oxvba-compiler compile_project_ --quiet`
- `cargo test -p oxvba-compiler predeclared --quiet`
- `cargo test -p oxvba-bind as_new_ --quiet`
- `cargo test -p oxvba-host pure_oxvba_class --quiet`
- `cargo test -p oxvba-host pure_oxvba_class_fields_are_per_instance_storage --quiet`
- `cargo test -p oxvba-host pure_oxvba_class_terminate_cascades_through_object_field --quiet`
- `cargo test -p oxvba-oxir class_ -- --nocapture`
- `cargo test -p oxvba-oxir -- --format terse`
- `cargo test -p oxvba-vm3 runtime_member_params_skips_only_explicit_hidden_me_receiver -- --nocapture`
- `cargo test -p oxvba-vm3 project_member_named_args_skip_only_explicit_hidden_me_receiver -- --nocapture`
- `cargo test -p oxvba-vm3 -- --format terse`
- `cargo test -p oxvba-differential --test class_lifecycle_vm3 -- --nocapture`
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
- Fresh-eyes issue from the package-metadata continuation: the new receiver verifier initially
  exposed a stale hand-built OxIR test that pointed a class property descriptor at `Main`.
  The fixture now uses a real receiver-bearing `Property Get` proc, keeping the test aligned with
  the class/member ABI instead of weakening the verifier.
- The setter verifier slice was checked against the existing binder lowering rule in
  `build_frame`: only the final `PropertyLet`/`PropertySet` value parameter is forced to runtime
  ByVal, while indexed parameters before it retain their declared direction. The verifier mirrors
  that exact package fact and intentionally does not add a broader `PropertySet` type rule in this
  slice.
- The default/enumerator verifier slice intentionally checks only contradictions visible in the
  package metadata. It does not infer Office-only export policy, require live COM interop, or close
  broader default-member runtime parity; those remain governed by the property/default-member and
  COM-export lanes.

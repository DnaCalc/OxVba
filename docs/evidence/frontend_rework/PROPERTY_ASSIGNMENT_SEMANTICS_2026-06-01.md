# Property and Assignment Semantics Evidence

Date: 2026-06-01
Bead: `bd-aprs.8.3`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_assignment_semantics.rs`, a typed property and
assignment semantics surface over HIR IDs.

The model records:

- Property Get/Let/Set accessor routes;
- default-member read/write/invoke routes;
- Let vs Set assignment intent;
- corresponding coercion descriptor kind;
- object/scalar assignment diagnostics.

The 2026-06-01 continuation added:

- `collect_assignment_semantics_from_typed_hir`, which derives Let/Set assignment semantics from
  the real typed-HIR/type-hook route instead of only constructing standalone examples;
- real HIR property arena population for parsed `Property Get/Let/Set` declarations;
- `collect_property_accessors_from_typed_hir`, which turns those parsed property declarations into
  accessor routes.
- distinct accessor symbols for `Property Get/Let/Set` declarations in the same property group, so
  valid VBA property groups such as `Property Get Value` plus `Property Let Value` no longer
  collide in the front-end symbol table.
- a production compile-path metadata validation gate that checks runtime property metadata against
  parsed front-end accessor facts when the front-end can parse the source.
- a production compile-path metadata validation gate for non-identity assignment coercions derived
  from typed HIR assignment semantics.
- project symbol routes now preserve `PropertyGet`/`PropertyLet`/`PropertySet` identity instead of
  flattening every accessor to `Procedure`, and property-group aliases keep multiple accessor
  candidates without overwriting each other.
- `compile_project_with_strategy` now validates active-project property declarations against the
  front-end project symbol routes before lowering, so drift between production `ProcedureDecl`
  metadata and front-end property routes is a compile-path error.
- project symbol indexing now falls back to a conservative signature-level legacy line scan when
  the syntax parser rejects a compatibility-heavy module body; this keeps the production route
  usable while the parser grammar is still being migrated.
- active-project explicit class property Get/Let/Set rewrites now consult the accessor-specific
  front-end project route first; the older `ProcedureDecl` scan remains as fallback for rewrite
  bridge, referenced projects, non-property members, and route gaps.
- active-project default-member property assignments and read assignments now re-bind the selected
  default accessor through the front-end property route before lowering. The legacy selector still
  chooses the default member candidate, but the emitted target is checked against the accessor route.
- active-project statement-form default-member reads now also re-bind the selected property target
  through the front-end accessor route before lowering.
- active-project default-member invocation/call-form targets now use the same front-end route
  rebound after the legacy selector chooses a property accessor.
- `ProjectSymbolIndex` now captures `VB_UserMemId = 0` default-member attributes for property
  groups and exposes default-member accessor lookup by owner/kind.
- active-project default-member candidate selection now consults the front-end default-member route
  before falling back to the legacy `ProcedureDecl` scan.
- the front-end default-member route also implements the existing unique single-candidate property
  rule when no `VB_UserMemId = 0` attribute exists.
- parseable sources now run front-end assignment diagnostics before legacy symbol resolution and
  type checking. The diagnostic messages are mapped to the established compiler wording so the
  production API shape remains stable while the decision comes from typed HIR facts.
- `bd-aprs.8.7` continuation: imported COM member classification now carries the typelib
  invocation kind, and early-bound COM property read plus property put/putref rewrite paths validate
  that the front-end dispatch classification matches both dispatch id and `PropertyGet`/`Method`/
  `PropertyPut`/`PropertyPutRef` kind before emitting the existing `DispatchInvoke` carrier.
- `bd-aprs.8.7` host continuation: selected host-injected property/default-member routes now
  validate through the front-end `HostGlobal` dispatch classification before the compatibility
  project rewrite bridge emits the selected host member call.
- `bd-aprs.8.7` named-argument HIR continuation: statement-form call arguments now preserve
  `name := expr` in `HirCallArg` and HIR production lowering carries that name into `BoundCallArg`,
  allowing existing call-site descriptor and argument-binding metadata to classify the argument as
  named from HIR facts. The syntax parser now accepts named arguments in both explicit no-paren
  `Call Proc name := value` and parenthesized `Call Proc(name := value)` forms so those routes
  reach the HIR call-argument path.
- `bd-aprs.8.7` default-member HIR continuation: HIR call lowering now allows non-procedure
  variable call targets such as `obj(42)` to lower as `BoundExpr::ProcCall`, which reaches the
  existing late-bound default-member emitter and records `LateBoundDefaultMember` call-site
  metadata with `DefaultMemberFallback` policy.
- `bd-aprs.8.7` indexed default-member assignment continuation: HIR assignment lowering now
  represents late-bound variable default-member writes as `BoundStmt::AssignDefaultMember`
  instead of trying to encode dispatch id `0` as a string member name. Emission targets dispatch
  member id `0`, preserves indexed argument names, and emits explicit `PropertyLet`/`PropertySet`
  hints for `obj(index := 2) = value` and `Set obj(2) = other`. Object-member binding metadata
  also records default-member `PropertyLet`/`PropertySet` rows for this late-bound variable subset.
  Follow-up metadata hardening records matching `LateBoundDefaultMember` call-site descriptors with
  `SyntheticPropertyAssignment`, `DefaultMemberFallback`, indexed arguments, and the synthetic
  `value` argument.
- `bd-aprs.8.7` default-member overload validation continuation: the compatibility project/member
  fallback now rejects multiple explicit `VB_UserMemId = 0` candidates of the required accessor
  kind instead of sorting and selecting one. Regression coverage includes authoritative
  default-member read, `Property Let`, and indexed `Property Set` ambiguity diagnostics.
  Follow-up arity hardening validates the selected active-project default-member accessor against
  the source argument count before rewriting, using source parameters with `Optional` and
  `ParamArray` awareness. Regression coverage includes authoritative and single-candidate
  non-authoritative default-member `Get`, `Let`, and `Set` wrong-arity diagnostics.
  Follow-up type hardening preserves accessor parameter source type text in the compatibility
  procedure index and rejects clear default-member assignment-form mismatches after route
  selection: `Let` into an explicitly object-typed value parameter and `Set` into a definitely
  scalar-typed value parameter now fail with a dedicated diagnostic.
- `bd-aprs.8.7` active-project dispatch-classification continuation: selected active-project
  property/default-member rewrite routes now validate that the front-end member-dispatch classifier
  reports the selected route as `EarlyBoundProject` with the expected accessor kind before
  retaining the compatibility rewrite carrier.
- `bd-aprs.8.7` predeclared-property rewrite-map continuation: the compatibility read-rewrite map
  for predeclared `Property Get` roots is now fallible and classifier-backed before rewrite.
  Active-project routes must classify as `EarlyBoundProject` property gets, and host-injected
  routes must classify as `HostGlobal`, before the legacy backend carrier is retained.
- `bd-aprs.8.7` imported default-member assignment continuation: early-bound COM assignment
  rewriting now recognizes bare/indexed imported default-member assignment syntax such as
  `obj(41) = value` and `Set obj(41) = other`. The path resolves a default member of the required
  `PropertyPut`/`PropertyPutRef` kind through typelib metadata before rewrite and therefore
  produces frontend/typelib diagnostics instead of falling through to backend parsing. A dedicated
  fixture alias, `OxVba.TestDispatchDefaultPut`, marks the indexed property put/putref members as
  default setters and proves positive `DispatchInvoke` rewrite for both Let and Set assignment
  forms.
- `bd-aprs.8.7` imported COM accessor production-route continuation: the positive property-get
  read/call rows, named/indexed property put/putref rows, and `OxVba.TestDispatchDefaultPut`
  default-member Let/Set rows now use accessor-specific internal early-invoke carriers and compile
  the active module through the HIR-capable project boundary for the single-active-module
  type-library-only subset. Bytecode assertions prove early-bound COM dispatch-id metadata plus
  `PropertyGet`/`PropertyLet`/`PropertySet` hints, and the compiled active source no longer includes
  projected typelib reference stubs on this route.
- `bd-aprs.8.7` host compatibility-helper metadata continuation: representative host-injected
  default-member/property get plus non-indexed and indexed let/set routes now assert patched
  `CallProc.project_member` bytecode metadata for the expected `pmr_hostproject_*` helper identity
  and accessor kind. This hardens the current rewrite-backed host route so it cannot silently lose
  property/default-member intent in bytecode metadata, while the broader host/reference HIR
  ownership and rewrite-body quarantine work remains open.
- `bd-aprs.8.7` language-service property continuation: compiler HIR property arena facts now
  project into IDE-facing `SemanticSnapshot` symbols as `SymbolKind::Property` with the
  user-facing property name, and `SemanticSnapshot::callables` also exposes a user-facing property
  callable alias backed by the canonical `property_get_*` HIR procedure. Signature help for
  `Value(1)` now resolves through those compiler-owned property facts rather than requiring a
  duplicate language-service semantic model. Follow-up coalescing now presents `Property Get`/
  `Property Let`/`Property Set` accessors for the same group as one user-facing property symbol
  identity, so go-to-definition and find-references for the getter use and setter declaration share
  the compiler HIR property-group fact instead of exposing duplicate IDE property symbols. The
  coalesced group prefers the getter accessor when present, preserving the read type and getter
  definition span even if a setter appears first in source order. The public callable surface now
  hides canonical `property_get_*`/`property_let_*`/`property_set_*` implementation names and keeps
  the logical getter alias for signature help. This covers the same-module property-group query
  surface; broader project/class/COM/default-member writeback and rewrite quarantine remain open.
- `bd-aprs.8.7` clean-binder indexed property continuation: `oxvba-bind` now handles
  member-qualified indexed project properties such as `w.Value(3) = 10` on the project-member
  property setter path. The binder resolves the project/class property accessor, binds index
  arguments against the accessor signature, appends/replaces the trailing assigned-value argument,
  and emits the project member dispatch instead of falling through to array/place assignment.
  Regression coverage proves the value `Property Let` setter receives the index before the
  assigned value, named index arguments are reordered by accessor signature before the assigned
  value is placed in the trailing slot, the paired indexed `Property Get` still reads through the
  property accessor, object-valued indexed `Property Set` carries the object value in the trailing
  parameter slot, and interface-typed receivers dispatch indexed property gets/lets through the
  implementing accessor name.
- `bd-aprs.8.7` clean-binder missing-accessor continuation: active-project property assignments
  now require the selected `Property Let`/`Property Set` accessor to exist before lowering a
  synthetic project-member setter call. Get-only scalar and indexed properties, plus object-valued
  properties that omit `Property Set`, now fail in the binder instead of silently fabricating a
  setter route.
- `bd-aprs.8.7` clean-binder default-member continuation: exported member attributes in the
  current scanner shape now associate `Attribute <Member>.VB_UserMemId = 0` with the logical
  project member, and active-project default-member syntax such as `w(3) = 10`, `w(2)`, and
  `Set b(3) = obj` lowers through the selected project `Property Let`/`Property Get`/
  `Property Set` accessor instead of falling through to object-as-array indexing.

## Checks

- `cargo test -p oxvba-compiler frontend_assignment_semantics --quiet`
- `cargo test -p oxvba-compiler frontend_hir --quiet`
- `cargo test -p oxvba-compiler frontend_type_hooks --quiet`
- `cargo test -p oxvba-compiler procedure_runtime_metadata_projects_first_signature_descriptor_view --quiet`
- `cargo test -p oxvba-compiler compile_property --quiet`
- `cargo test -p oxvba-compiler compile_project_ --quiet`
- `cargo test -p oxvba-compiler frontend_project_symbols --quiet`
- `cargo test -p oxvba-compiler frontend_assignment_semantics --quiet`
- `cargo test -p oxvba-compiler compile_options_frontend_v2 --quiet`
- `cargo test -p oxvba-compiler frontend_member_dispatch --quiet`
- `cargo test -p oxvba-compiler property_put_external --quiet`
- `cargo test -p oxvba-compiler imported_property --quiet`
- `cargo test -p oxvba-compiler host_injected_predeclared_property --quiet`
- `cargo test -p oxvba-compiler predeclared_property --quiet`
- `cargo test -p oxvba-compiler host_injected_global_namespace_property --quiet`
- `cargo test -p oxvba-compiler host_injected_predeclared_default_member --quiet`
- `cargo test -p oxvba-compiler host_injected_global_namespace_default_member --quiet`
- `cargo test -p oxvba-compiler host_injected --quiet`
- `cargo test -p oxvba-compiler hir_builder_preserves_named_call_arguments --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_preserves_named_call_arguments --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_late_bound_default_member_call --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_late_bound_default_member --quiet`
- `cargo test -p oxvba-compiler ambiguous_authoritative --quiet`
- `cargo test -p oxvba-compiler wrong_arity_for_authoritative_default_member --quiet`
- `cargo test -p oxvba-compiler optional_default_member_get_arity --quiet`
- `cargo test -p oxvba-compiler object_typed_default_member_value --quiet`
- `cargo test -p oxvba-compiler scalar_typed_default_member_value --quiet`
- `cargo test -p oxvba-compiler default_member --quiet`
- `cargo test -p oxvba-compiler external_member --quiet`
- `cargo test -p oxvba-compiler property_put_external --quiet`
- `cargo test -p oxvba-compiler routes_imported_default_member --quiet`
- `cargo test -p oxvba-compiler imported_default_member_property --quiet`
- `cargo test -p oxvba-compiler compile_project_rewrites_early_bound_member_call_to_dispatchinvoke_subset --quiet`
- `cargo test -p oxvba-compiler external_default_member --quiet`
- `cargo test -p oxvba-com default_put_fixture --quiet`
- `cargo test -p oxvba-compiler frontend_member_dispatch --quiet`
- `cargo test -p oxvba-compiler frontend_hir_lowering --quiet`
- `cargo test -p oxvba-compiler compile_project --quiet`
- `cargo test -p oxvba-languageservice snapshot_symbols_classify_properties_from_frontend_hir --quiet`
- `cargo test -p oxvba-languageservice signature_help_resolves_property_get_alias_from_frontend_hir --quiet`
- `cargo test -p oxvba-languageservice --quiet`
- `cargo test -p oxvba-compiler frontend_legacy_route_audit --quiet`
- `cargo test -p oxvba-bind indexed_property_get_let_roundtrip --quiet`
- `cargo test -p oxvba-bind named_indexed_property_let_roundtrip --quiet`
- `cargo test -p oxvba-bind indexed_property --quiet`
- `cargo test -p oxvba-bind implements_indexed_property_through_interface_var --quiet`
- `cargo test -p oxvba-bind property_is_bind_error --quiet`
- `cargo test -p oxvba-bind set_assigning_to_property_without_set_accessor_is_bind_error --quiet`
- `cargo test -p oxvba-symbol exported_member_attribute_marks_project_default_member --quiet`
- `cargo test -p oxvba-bind project_default_member --quiet`
- `cargo test -p oxvba-bind --quiet`
- `cargo test -p oxvba-symbol --quiet`
- `cargo check --workspace`
- `cargo fmt --check -p oxvba-bind -p oxvba-symbol`
- `cargo check -p oxvba-compiler --quiet`
- `cargo test -p oxvba-syntax call --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The semantics are represented as HIR/symbol facts, not source rewrites.
- Object/scalar diagnostics are deliberately conservative: invalid `Set` and object-target `Let`
  cases produce stable binder diagnostic codes.
- The assignment side now has executable front-end route proof: typed HIR assignment statements are
  converted into `AssignmentSemantics` records with assignment intent, coercion kind, inferred
  source/target types, and diagnostics.
- Parsed `Property Get/Let/Set` declarations now populate HIR property facts and accessor routes
  from real source.
- Fresh-eyes review found and fixed a property-group collision: the symbol model previously treated
  all accessors named `Value` as the same `Procedure/Value` declaration. Accessor declarations now
  use canonical `property_get_`, `property_let_`, and `property_set_` symbol names while preserving
  `HirPropertyKind`.
- A follow-up cross-bead review found that canonical accessor symbols hid user-facing property names
  from project/class lookup. `ProjectSymbolIndex` now records both the canonical accessor route and
  the property-group alias, so `Customer.DisplayName` still resolves after the accessor-symbol fix.
- Production metadata now has a front-end consistency check: compiled property procedure metadata
  must agree with HIR property accessor kind and property group facts for sources the front-end can
  parse.
- Non-identity assignment coercion metadata is now checked against typed-HIR assignment semantics.
  Identity assignments are intentionally skipped because they do not always emit coercion
  descriptors.
- Fresh-eyes review found that the project symbol index previously collapsed property accessors to
  generic procedures and stored only one owner/member route. That lost the distinction between Get,
  Let, and Set and allowed aliases to overwrite earlier accessors. The route table now stores
  accessor-specific candidates, exposes typed property-accessor lookup, and keeps ordinary
  qualified lookup unique-only.
- The route validation is intentionally active-project only. Referenced projects and host-injected
  roots are still handled by the existing project/COM rewrite lanes and belong to later FE-7/FE-8
  migration beads.
- Explicit active-project property member reads and writes now consume front-end route decisions in
  the production lowering path. Fresh-eyes review found that a missing route cannot be treated as
  drift because the old resolver also probes property routes while diagnosing non-property members;
  missing accessor routes therefore fall back, while present routes are checked for kind/module/name
  consistency before being used.
- Default-member read/write assignment lowering now uses the same route surface after candidate
  selection.
- Fresh-eyes review found that post-selection rebinding still left the actual default-member
  decision on the legacy scan. The project symbol index now records default-member attributes and
  active-project default-member candidate selection uses that front-end route first. A second
  fresh-eyes pass found that the non-authoritative single-candidate default-member rule was still
  legacy-only; the front-end route table now owns that rule too. The legacy scan remains as fallback
  for rewrite-bridge, referenced projects, non-property members, and route gaps.
- Front-end assignment diagnostics now participate in the production compile path for sources the
  front-end can type. Fresh-eyes review corrected the `Set` diagnostic rules first: Object/Variant
  lanes are not compile-time diagnostics because runtime guards handle them, while scalar target or
  scalar value lanes remain diagnostics.
- The legacy line-scan fallback is a compatibility bridge, not the desired terminal shape. It avoids
  rejecting modules that the current parser cannot fully parse, but it only records signature-level
  procedures/properties/fields.
- Closure review: the remaining `ProcedureDecl` scans in `project.rs` are no longer authoritative
  for active-project property/default-member property routes. Active-project property declarations
  are indexed and validated up front, explicit property Get/Let/Set rewrites consult the front-end
  route first, default-member attribute and single-candidate selection consult the front-end route
  first, and parseable assignment diagnostics run from typed-HIR facts before legacy type checking.
  Residual fallback scans are classified as compatibility fallback for rewrite-bridge mode,
  referenced projects, non-property function/sub/member probes, and parser-incomplete route gaps;
  those are outside FE-7.3 and are covered by later FE-7/FE-8 migration beads.
- `bd-aprs.8.7` continuation fresh-eyes review found a weak imported-COM lane: property
  put/putref rewrites resolved typelib metadata directly, while earlier read/invoke paths at least
  checked the imported-COM dispatch classifier. The classifier now records invocation kind and the
  early-bound property read/setter rewrite paths validate the classifier before keeping the
  compatibility `DispatchInvoke` source rewrite. This is still not full closure for the bead:
  host/project/imported-COM default-member writeback breadth, type overload validation, and
  replacement/quarantine of the remaining project rewrite bodies remain open.
- Host continuation fresh-eyes review found that `classify_host_global` existed only as a model
  test and did not protect the production host-injected route. The selected host member and
  default-member resolver exits now validate host/global classification before returning the
  lowered PMR target. This improves route proof for host fixtures, but it remains a guard around the
  compatibility rewrite path, not full HIR ownership.
- Named-argument HIR review found that the syntax parser preserved `:=` tokens, but HIR collapsed
  arguments to positional expressions. `HirCallArg` now carries an optional source name and
  statement-form HIR lowering preserves that into `BoundCallArg`; explicit no-paren `Call` parses
  the same bare argument list, and parenthesized argument lists now parse `name := expr` through the
  same argument parser. The late-bound variable indexed default-member assignment subset now
  consumes those named argument facts through `AssignDefaultMember`; broader project/host/imported
  COM writeback breadth remains open.
- Default-member HIR review found that `IndexExpr` on a variable receiver lowered as a call and was
  rejected before the emitter could apply its existing late-bound default-member fallback. HIR now
  lets that bound call reach the default-member emitter, preserving dispatch invoke bytecode and
  call-site metadata. This covers read/invoke fallback; the following continuation covers the
  late-bound variable indexed assignment subset.
- Indexed default-member assignment review found that using `AssignMember` would have been the
  wrong carrier because it emits a string selector and cannot faithfully represent default member
  dispatch id `0`. HIR now uses a separate `AssignDefaultMember` statement for late-bound variable
  receivers, checks the receiver through type checking, includes the indexed arguments in value,
  semantic, operator, and coercion descriptor collection, and emits late-bound property put/putref
  dispatch against member id `0`. Object-member binding descriptors record these writes as
  default-member property Let/Set rows. Follow-up review found that the bytecode and object-member
  rows were present but call-site metadata still omitted the synthetic property assignment. The
  emitter now records `LateBoundDefaultMember`/`SyntheticPropertyAssignment` call sites with the
  indexed arguments and synthetic `value` argument. This closes the HIR fact and call-site metadata
  path for the variable default-member assignment subset. Broader project/host/imported-COM
  default-member writeback breadth, type overload validation, and replacement/quarantine of
  remaining rewrite bodies remain open.
- Default-member overload review found that the front-end default-member route correctly returned
  no unique route for multiple explicit `VB_UserMemId = 0` candidates, but the legacy fallback then
  sorted the same candidates and selected one. The fallback now reports
  `PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS` for multiple authoritative candidates, matching the
  existing non-authoritative ambiguity policy. A follow-up arity review found that candidate
  selection still ignored the actual source argument count and could rewrite a selected default
  member into the wrong accessor shape. The default-member resolver now validates selected
  accessors against the supplied source argument count before returning the route, with
  `Optional`/`ParamArray`-aware bounds. This closes the selected active-project ambiguity and
  arity subset. A narrow type-validation follow-up now rejects definite Let/Object and Set/scalar
  value-parameter mismatches on selected default-member assignment routes; broader overload
  validation and host/imported writeback breadth remain open.
- Active-project route proof review found that project property/default-member rewrite paths used
  front-end symbol routes but did not assert the member-dispatch classifier at the final selected
  route. The selected route now must classify as `EarlyBoundProject` with the expected accessor kind
  before the rewrite carrier is retained. This is still route proof around the compatibility
  rewrite body, not full HIR-native replacement.
- Active-project compatibility-helper metadata review found the same source-only evidence weakness
  in representative project PMR rows. The `PropertyGet`, indexed/default-member `PropertyLet`, and
  indexed/default-member `PropertySet` rows now assert patched `CallProc.project_member` bytecode
  metadata for the expected normalized `pmr_projecta_widget_value` helper identity plus accessor
  kind. This hardens the current active-project compatibility route; broader HIR-native property
  writeback and rewrite-body quarantine remain open.
- Imported-COM accessor route review found that merely proving a `DispatchInvoke` source rewrite was
  too weak: type-library-only rows could still compile through the legacy project backend and the
  source carrier did not preserve get/put/putref intent. The project boundary now keeps separate
  full compatibility source and active-project source; all-procedural active projects with only
  synthetic type-library references compile the active source through HIR, including a two-module
  route proof where a helper module owns the imported COM use. The rewrite carrier
  preserves accessor intent through internal early-invoke names, and bytecode proof checks dispatch
  id, arity, early-bound COM metadata, and `PropertyGet`/`PropertyLet`/`PropertySet` hints for
  property-get read/call rows plus named, indexed, named-argument indexed, and default-member setter
  rows. Broader reference-project, host, imported-COM, and rewrite-body retirement remains open.
- Host route review found the analogous weakness in a different form: host-injected routes were
  already classifier-backed before retaining their PMR rewrite carrier, but many tests only checked
  helper text. The new assertions check the patched bytecode metadata for representative root and
  host-returned child get plus non-indexed/indexed let/set helpers, so current compatibility
  lowering preserves accessor intent until the route is migrated to HIR or explicitly quarantined.
- Active-project PMR route hardening continued on 2026-06-03: representative bare default-member
  `Property Let`, indexed default-member `Property Get`, and call-statement indexed `Property Get`
  compatibility-helper rows now assert emitted `CallProc.project_member` metadata for normalized
  `pmr_projecta_widget_value` with the expected `PropertyGet`/`PropertyLet` accessor kind. This is
  route-proof hardening only; native HIR replacement or explicit compatibility quarantine remains
  required before FE-7.3.a/FE-8.5.c can close.
- Clean-binder review on 2026-06-17 found that member-qualified indexed project properties had a
  real production ownership gap in the reimplemented binder: COM, cross-project, and late-bound
  receivers already used property put/set dispatch, but `ProjectMember` receivers returned `None`
  and then failed as non-assignable property-get places. The new branch keeps that route in
  symbol/signature-owned binder lowering. Follow-up coverage proves the same route for object
  `Property Set` and interface-typed receivers. This closes the project-class/interface indexed
  property setter subset only; broader host/reference/imported-COM writeback breadth and terminal
  rewrite retirement remain open.
- Missing-accessor review on 2026-06-17 found the paired failure mode introduced by the same
  binder/provider split: the project provider resolves the property group, while assignment syntax
  chooses `Let` or `Set` in the binder. Without an accessor-existence check, a get-only property
  group could still lower to a synthetic setter call. The binder now requires the requested
  accessor signature for bare, member-qualified, and indexed active-project property assignments.
- Default-member review on 2026-06-17 found two clean-path gaps after the old rewrite layer was
  removed. First, exported `.cls` member attributes were parsed as top-level `AttributeStmt` nodes,
  but the scanner only recognized `VB_UserMemId = 0` when the text appeared inside the procedure
  node. Second, `w(3)` on a known project class still took the array-index path unless the default
  member came from COM metadata. The scanner now associates exported member attributes with the
  logical member, and the binder routes active-project default-member reads and indexed Let/Set
  writes through the same accessor-signature path used by explicit property syntax.
- Bare default-member review on 2026-06-17 found a false pass in the clean binder: `w = 10;
  r = w` on a defaulted project object could appear to work only because the `Let` assignment
  overwrote the object variable slot with a scalar. The binder now treats bare object variables as
  default-member receivers only in Let/value contexts: `w = value` lowers to the default
  `Property Let`, `r = w` lowers to the default `Property Get`, and `Set w2 = w` remains an
  object-reference assignment. Property Let RHS binding uses the same value-context default-member
  rule, so `dst = src` passes `src`'s default value to `dst`'s setter. The VM assignment guard also
  rejects any remaining plain `Let` store into an object slot when no default-member setter route
  was selected. Regression coverage: `project_default_member_bare_get_let_roundtrip`,
  `set_assignment_keeps_defaulted_object_reference`,
  `project_property_let_rhs_uses_default_member_value`, and
  `let_assignment_to_object_without_default_member_is_runtime_error`. Excel oracle coverage:
  `Range` default-member macro probe passed for `cell = 10`, `Set cell2 = cell`, `cell2 = 12`,
  `r = cell`. Checks: `cargo test -p oxvba-bind default_member --quiet`,
  `cargo test -p oxvba-bind --quiet`, `cargo test -p oxvba-vm2 --quiet`,
  `cargo check --workspace`, `cargo fmt --check -p oxvba-bind -p oxvba-vm2`, `git diff --check`,
  and `./scripts/check-governance.ps1`.
- Reference-project breadth review on 2026-06-17 added a cross-bundle regression for the same
  bare default-member semantics. A referenced `Lib.Widget` publishes `Value` with
  `VB_UserMemId = 0`; the active project executes `src = 7`, `dst = src`, `Set mirror = dst`,
  `mirror = 9`, and `r = dst`. The row proves the referenced-project export surface already
  carries default-member metadata through `ExternMember` binding, and the clean binder's
  value-context rule now works across bundle boundaries without another production change.
  Checks: focused `cross_project_default_member_bare_let_get_preserves_object_reference` filter,
  `cargo test -p oxvba-bind --test cross_project --quiet`,
  `cargo test -p oxvba-bind --quiet`, `cargo fmt --check -p oxvba-bind`, and
  `git diff --check`.
- Imported-COM breadth review on 2026-06-17 added a structural binder regression for a typed COM
  receiver with a synthetic default `Value` property sharing dispid `0` across `PropertyGet` and
  `PropertyPut`. The source `w = 10; Set w2 = w; r = w2` now proves the binder emits exactly one
  early-bound default `PropertyLet` and one early-bound default `PropertyGet`, while the intervening
  `Set` assignment remains an object-reference store and does not trigger a default-member call.
  Checks: focused `typed_com_default_member_bare_let_get_lowers_to_early_com` filter,
  `cargo test -p oxvba-bind --quiet`, `cargo fmt --check -p oxvba-bind`, and `git diff --check`.

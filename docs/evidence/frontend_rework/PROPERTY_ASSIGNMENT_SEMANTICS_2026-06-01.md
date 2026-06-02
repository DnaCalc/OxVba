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
- `cargo test -p oxvba-compiler routes_imported_default_member --quiet`
- `cargo test -p oxvba-compiler imported_default_member_property --quiet`
- `cargo test -p oxvba-compiler external_default_member --quiet`
- `cargo test -p oxvba-com default_put_fixture --quiet`
- `cargo test -p oxvba-compiler frontend_member_dispatch --quiet`
- `cargo test -p oxvba-compiler frontend_hir_lowering --quiet`
- `cargo test -p oxvba-compiler compile_project --quiet`
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

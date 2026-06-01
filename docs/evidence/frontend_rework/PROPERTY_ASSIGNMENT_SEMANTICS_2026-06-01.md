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

## Checks

- `cargo test -p oxvba-compiler frontend_assignment_semantics --quiet`
- `cargo test -p oxvba-compiler frontend_hir --quiet`
- `cargo test -p oxvba-compiler frontend_type_hooks --quiet`
- `cargo test -p oxvba-compiler procedure_runtime_metadata_projects_first_signature_descriptor_view --quiet`
- `cargo test -p oxvba-compiler compile_property --quiet`
- `cargo test -p oxvba-compiler compile_project_ --quiet`
- `cargo test -p oxvba-compiler frontend_project_symbols --quiet`
- `cargo test -p oxvba-compiler compile_options_frontend_v2 --quiet`
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
  active-project default-member candidate selection uses that front-end route first. The legacy scan
  remains as fallback for rewrite-bridge, referenced projects, non-property members, and route gaps.
- The legacy line-scan fallback is a compatibility bridge, not the desired terminal shape. It avoids
  rejecting modules that the current parser cannot fully parse, but it only records signature-level
  procedures/properties/fields.
- This bead is not complete yet. The large `project.rs` property/default-member rewrite matrix
  still owns project/class property Get/Let/Set, default member reads/writes/invokes, and many
  assignment diagnostics for compiled projects. Next step: migrate assignment diagnostics onto the
  same front-end decision surface and narrow or quarantine the remaining legacy fallback scans.

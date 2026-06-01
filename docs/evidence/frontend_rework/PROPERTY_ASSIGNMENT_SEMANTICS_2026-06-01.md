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

## Checks

- `cargo test -p oxvba-compiler frontend_assignment_semantics --quiet`
- `cargo test -p oxvba-compiler frontend_hir --quiet`
- `cargo test -p oxvba-compiler frontend_type_hooks --quiet`
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
- This bead is not complete yet. The large `project.rs` property/default-member rewrite matrix
  still owns project/class property Get/Let/Set, default member reads/writes/invokes, and many
  assignment diagnostics for compiled projects. Next step: connect the property/default-member
  project route metadata to this typed semantics surface before closing the bead.

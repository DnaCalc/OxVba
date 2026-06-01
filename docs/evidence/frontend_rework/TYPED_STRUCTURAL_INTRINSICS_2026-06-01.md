# Typed Structural Intrinsics Evidence

Date: 2026-06-01
Bead: `bd-aprs.9.1`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_structural_intrinsics.rs`, a typed enum for structural
compiler intrinsics that previously appeared only as magic legacy names. The reopened continuation
moved the first production routes onto typed `BoundExpr::StructuralIntrinsicCall` nodes.

Covered families: Nothing, Null, omitted args, project instances, WithEvents attach/detach,
dynamic dispatch get/let/set/invoke, and pointer helpers.

Production route proof now covers:

- `Null` literal binding;
- `Nothing` literal binding; and
- omitted positional argument sentinels.

Those constructs no longer enter the backend as `IntrinsicCall { name: "__null" | "__nothing" |
"__omitted" }`; emit, metadata collection, optimization walks, and typechecking consume the typed
structural-intrinsic variant directly.

The remaining enum families still have a compatibility legacy-name bridge. Project instance,
WithEvents, dynamic dispatch, and pointer helper migration remains bounded to later FE-8/FE-9
lowering cleanup unless a follow-up slice in this bead moves them first.

## Checks

- `cargo test -p oxvba-compiler frontend_structural_intrinsics --quiet`
- `cargo test -p oxvba-compiler resolve_null_and_nothing_as_typed_structural_intrinsics --quiet`
- `cargo test -p oxvba-compiler resolve_omitted_positional_arguments_bind_sentinel --quiet`
- `cargo test -p oxvba-compiler null --quiet`
- `cargo test -p oxvba-compiler nothing --quiet`
- `cargo test -p oxvba-compiler omitted --quiet`
- `cargo test -p oxvba-compiler structural --quiet`
- `cargo test -p oxvba-compiler object_assignment --quiet`
- `cargo test -p oxvba-compiler emit --quiet`
- `cargo test -p oxvba-compiler compile_project_ --quiet`
- `cargo check -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The enum has an explicit legacy-name bridge for staged migration, but unknown magic strings do
  not classify as typed intrinsics.
- The first production migration is real IR shape, not only a classifier: resolver output for
  `Null`, `Nothing`, and omitted arguments is typed before emit/typecheck.
- A broader emit check exposed that the frontend assignment diagnostic pre-pass still treated bare
  assignment as explicit `Let`. The pre-pass gate now only surfaces `BIND-E-LET-OBJECT-TARGET`
  when source text actually used explicit `Let`, preserving the existing runtime-validation lane
  for implicit Variant-to-Object assignment.
- Legacy string use remains material for the non-migrated structural families and for ordinary VBA
  intrinsics. This evidence does not claim full retirement of every `IntrinsicCall { name }` path.

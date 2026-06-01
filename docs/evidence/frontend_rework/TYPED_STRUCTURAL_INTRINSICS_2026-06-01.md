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
- omitted positional argument sentinels;
- project-class instance materialisation (`__oxvba_project_instance`); and
- pointer helpers (`VarPtr`, `StrPtr`, `ObjPtr`) including external-call pointer writeback
  classification;
- WithEvents runtime helpers (`__oxvba_withevents_get`, `set`, `clear_owner`, `first_owner`,
  `next_owner`); and
- dynamic dispatch invoke helpers (`DispatchInvoke`, `__OxVbaEarlyInvoke`).

Those constructs no longer enter the backend as structural `IntrinsicCall { name: ... }` magic
strings; emit, metadata collection, optimization walks, typechecking, event binding, dynamic
dispatch, and pointer writeback consume the typed structural-intrinsic variant directly.

The compatibility bridge remains for ordinary VBA intrinsics and non-structural helper names.
Placeholder dynamic get/let/set variants were removed because this codebase has no production
`__oxvba_dispatch_get`/`let`/`set` helpers to migrate.

## Checks

- `cargo test -p oxvba-compiler frontend_structural_intrinsics --quiet`
- `cargo test -p oxvba-compiler resolve_null_and_nothing_as_typed_structural_intrinsics --quiet`
- `cargo test -p oxvba-compiler resolve_omitted_positional_arguments_bind_sentinel --quiet`
- `cargo test -p oxvba-compiler null --quiet`
- `cargo test -p oxvba-compiler nothing --quiet`
- `cargo test -p oxvba-compiler omitted --quiet`
- `cargo test -p oxvba-compiler structural --quiet`
- `cargo test -p oxvba-compiler pointer --quiet`
- `cargo test -p oxvba-compiler resolve_statement_level_call_without_parentheses_preserves_arguments --quiet`
- `cargo test -p oxvba-compiler compile_project_internal_dynamic_routes_do_not_keep_transitional_token_table --quiet`
- `cargo test -p oxvba-compiler withevents --quiet`
- `cargo test -p oxvba-compiler dispatchinvoke --quiet`
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
- The production structural-helper names in this bead now bind as typed structural intrinsics.
  Legacy string use remains material for ordinary VBA intrinsics and unrelated compatibility
  helpers. This evidence does not claim full retirement of every `IntrinsicCall { name }` path.

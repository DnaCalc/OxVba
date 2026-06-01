# Class Construction and Fields Evidence

Date: 2026-06-01
Bead: `bd-aprs.8.4`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_class_semantics.rs`, a typed route model for class
construction and object fields.

The model records:

- `New` expression construction;
- `As New` lazy construction;
- predeclared instance routes;
- ordinary fields;
- WithEvents fields;
- runtime object-field metadata: slot, refcounted object storage, As New initialization, and
  WithEvents flag.

## Checks

- `cargo test -p oxvba-compiler frontend_class_semantics --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- An initial attempt to reuse `ObjectMemberKindDescriptor::Field` was wrong because that descriptor
  has no field variant. The bead now uses a local `ClassMemberRuntimeKind::Field` and keeps existing
  object descriptors only where they actually fit.
- The metadata shape preserves the bd-1ufc requirement that object fields have explicit runtime
  layout/lifetime facts rather than implicit handle integers.

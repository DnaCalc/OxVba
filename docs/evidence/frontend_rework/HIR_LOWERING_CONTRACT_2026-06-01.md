# HIR Lowering Contract Evidence

Date: 2026-06-01
Bead: `bd-aprs.9.3`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Reopened production-route update: `crates/oxvba-compiler/src/frontend_lowering_contract.rs` now
derives executable lowering contracts from typed HIR and normal compilation validates emitted
`ProcedureRuntimeMetadata` against those contracts when the current HIR builder supports the source.

The contract path now checks that:

- procedure metadata exists for HIR procedure symbols,
- frame overlays resolve through symbol-backed runtime slots rather than assuming a flat slot list,
- HIR return-value assignments correspond to emitted return-slot metadata,
- HIR coercion overlays are scoped to the procedure containing the expression and correspond to
  emitted coercion descriptors, and
- the contract surface stays typed (`StructuralIntrinsic` values and symbol ids) rather than relying
  on legacy intrinsic-name strings.

Known HIR parser residue is explicitly quarantined: declaration modifiers that the current HIR
symbol pass can still misclassify as local symbols (`WithEvents`, `Optional`, `ByVal`, `ByRef`,
`ParamArray`) are not treated as required runtime frame slots.

## Checks

- `cargo test -p oxvba-compiler frontend_lowering_contract --quiet`
- `cargo test -p oxvba-compiler frontend_type_hooks --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_projects_named_slot_kinds --quiet`
- `cargo test -p oxvba-compiler mixed_call_coercion_variant_to_long_is_allowed --quiet`
- `cargo test -p oxvba-compiler compile_project_allows_withevents_in_class_module --quiet`
- `cargo test -p oxvba-compiler compile_project_ --quiet`
- `cargo test -p oxvba-compiler --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- This bead does not claim full HIR-to-bytecode emission; that remains FE-8.5. The production change
  here is that emitted metadata is now checked against the typed-HIR lowering contract in the normal
  compile route.
- The first implementation over-scoped coercion overlays across all procedures; full package tests
  caught that, and overlays are now constrained by HIR expression span to the owning procedure.
- The `WithEvents` modifier misclassification is recorded as current HIR detritus and quarantined
  locally instead of weakening production metadata slot requirements.

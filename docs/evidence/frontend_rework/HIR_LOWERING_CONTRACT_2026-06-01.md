# HIR Lowering Contract Evidence

Date: 2026-06-01
Bead: `bd-aprs.9.3`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_lowering_contract.rs`, a HIR lowering contract for
descriptor-backed calls, returns, writebacks, frame overlays, and typed structural intrinsics.

## Checks

- `cargo test -p oxvba-compiler frontend_lowering_contract --quiet`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The contract carries typed call-site hooks and structural intrinsic enum values rather than
  legacy intrinsic strings.
- Frame overlays distinguish symbol, temporary, and coercion slot sources, so the contract does not
  require a flat-slot assumption.
- This is a contract surface; production lowering still needs staged migration to consume it.

# Type and Coercion Hooks Evidence

Date: 2026-06-01
Bead: `bd-aprs.7.4`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_type_hooks.rs`, a HIR fact layer that connects FE-6 HIR
IDs to the existing compiler/runtime type descriptors.

The hook layer records:

- declared type facts from `SymbolId` to `HirTypeId` and `VbaTypeId`;
- assignment intent per `HirStmtId` (`Let` vs `Set`);
- coercion facts per `HirExprId` using existing `CoercionKindDescriptor`;
- call-site facts per `HirCallId`;
- parameter mechanics using existing `ParameterPassingMode`;
- `Optional`, `ParamArray`, and `OptionalDefaultValue` policy.

## Focused Fixtures

The unit tests cover:

- local declared type hook mapped to `VbaTypeId::Long`;
- `Let` assignment plus string-to-long coercion fact;
- call-site parameter mechanics with ByRef required parameter, ByVal optional parameter with
  explicit default, and ParamArray parameter.

## Checks

- `cargo test -p oxvba-compiler frontend_type_hooks --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The hooks reuse existing descriptor enums instead of introducing a parallel type vocabulary for
  coercions, parameter passing, and optional defaults.
- The hook layer is keyed by HIR IDs and symbols, so later lowering can consume facts without
  going back to parser-shaped strings.
- This bead does not yet lower through the production emitter from HIR. It provides the typed
  contract needed by FE-8.3 and later project semantics migration beads.

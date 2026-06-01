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

Reopened update: added `collect_type_hooks_from_source(module_name, source)`, which builds HIR
from source, extracts simple declared `As <type>` facts from CST-backed symbol spans, records
`Let`/`Set` assignment intent from HIR statements, and emits a scoped coercion fact for mismatched
assignment types such as string literal to `Long`.

## Focused Fixtures

The unit tests cover:

- local declared type hook mapped to `VbaTypeId::Long`;
- `Let` assignment plus string-to-long coercion fact;
- call-site parameter mechanics with ByRef required parameter, ByVal optional parameter with
  explicit default, and ParamArray parameter.
- source-backed HIR collection of parameter/local declared types (`ByVal seed As Long`,
  `Dim label As String`);
- source-backed HIR collection of `Let` assignment intent and string-to-long coercion for
  `count = "1"` where `count As Long`.

## Checks

- `cargo test -p oxvba-compiler frontend_type_hooks --quiet`
- `cargo test -p oxvba-compiler frontend_symbols --quiet`
- `cargo test -p oxvba-compiler frontend_hir --quiet`
- `cargo test -p oxvba-compiler frontend_semantic_model --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The hooks reuse existing descriptor enums instead of introducing a parallel type vocabulary for
  coercions, parameter passing, and optional defaults.
- The hook layer is keyed by HIR IDs and symbols, so later lowering can consume facts without
  going back to parser-shaped strings.
- The reopened source-backed route is deliberately small: it covers simple built-in `As` types,
  `Let`/`Set` intent, and a basic assignment coercion fact. It does not yet cover complete
  call-site binding, optional/default parsing from source, ParamArray lowering from source,
  contextual-keyword identifier edge cases, or production emitter lowering from HIR.
- This bead provides the typed contract and first executable route needed by FE-8.3 and later
  project semantics migration beads.

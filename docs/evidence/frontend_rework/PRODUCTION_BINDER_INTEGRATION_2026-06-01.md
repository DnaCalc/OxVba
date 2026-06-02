# Production Binder Integration Evidence

Date: 2026-06-01
Bead: `bd-aprs.7.6`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `analyze_frontend_v2_source` and routed `compile_with_options(... frontend_v2: true)`
through it. The original FE-6.6 route still emitted bytecode through the temporary syntax bridge;
later FE-8/FE-9 continuations moved bytecode and metadata emission to direct HIR production
lowering.

The opt-in production route now performs:

- `oxvba-syntax` parser diagnostics through `FrontendDiagnosticMapper`;
- FE-6.1 symbol collection;
- FE-6.2 HIR construction;
- FE-6.3 SemanticModel indexing;
- FE-6.4 type/coercion hook collection;
- HIR production lowering for bytecode and runtime metadata.

This is not the terminal production front-end replacement yet. It is the first integration point
where the opt-in compile route must successfully build compiler-owned binder/HIR/SemanticModel
facts before bytecode is emitted.

## Checks

- `cargo test -p oxvba-compiler compile_options_ --quiet`
- `cargo test -p oxvba-compiler frontend_v2_analysis --quiet`
- `cargo test -p oxvba-compiler frontend_type_hooks --quiet`
- `cargo test -p oxvba-compiler frontend_diagnostics --quiet`
- `cargo test -p oxvba-compiler syntax_bridge --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The route now lowers through `frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir`;
  the bridge-era note is historical, and `syntax_bridge` is crate-private support code.
- The analysis surface proves shared facts are available to compiler and SemanticModel callers,
  but the scoped binder still covers a small declaration/assignment/type subset.
- Parser diagnostics now fail before bridge lowering as `frontend_v2 diagnostics`, so the v2 route
  does not silently fall through to legacy behavior after syntax recovery.
- Project/class/COM/default-member semantics remain outside this FE-6.6 slice and are owned by
  FE-7 beads.

# Semantic/Diff Harness Evidence

Date: 2026-06-01
Bead: `bd-aprs.6.2`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added a compiler-layer semantic/differential harness in
`crates/oxvba-compiler/src/frontend_diff.rs`.

The harness can now compare:

- diagnostics as normalized strings;
- bytecode summaries, including instruction text, slot counts, and external call counts;
- runtime metadata summaries, including procedure line mappings, statement PCs, slot metadata,
  parameter slots, parameter types, return type, signature, call sites, array/type/layout facts,
  value states, expression/operator semantics, coercions, and name/member bindings;
- execution-trace and observable-output lanes as explicit `NotRun` statuses.

The front-end v2 side validates through the CST parser first, then uses the current legacy
semantic/lowering path. This preserves default compiler behavior while giving FE-5.3 and FE-5.4
a single report shape to extend with classifier rows and VM/host execution observations.

## Checks

- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo test -p oxvba-compiler compile_options_ --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The harness intentionally lives in `oxvba-compiler`, so it cannot execute VM traces without
  creating a compiler-to-VM dependency cycle. The report shape records execution trace and
  observable output as explicit `NotRun` values rather than silently omitting them. FE-5.4 should
  add a higher-layer runner for VM/host observations.
- Metadata comparison is available for v2 because v2 validation now exposes
  `validate_source_with_cst`, allowing the harness to validate syntax before calling
  `compile_with_runtime_metadata`.
- Bytecode comparison is not required to be byte-identical as the final rule for the workset.
  This bead only supplies the normalized diff surface; FE-5.3 classifies differences as bugs,
  harmless drift, or intentional improvements.
- Default compile behavior remains unchanged. `compile_with_options(... frontend_v2: true)` still
  routes through the existing bridge, and the new diff harness is opt-in test/support surface.

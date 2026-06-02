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

Reopened update: the front-end v2 side originally used the same syntax-bridge compile route as
`compile_with_options(... frontend_v2: true)`, including runtime metadata, so the harness no
longer compared a CST-only precheck followed by the unmodified legacy compiler. FE-8/FE-9
continuations then moved the v2 observer to direct
`frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir`, matching the current
`frontend_v2` production route. A v2-only inline statement-separator fixture proves the harness can
see a construct that the legacy baseline rejects and HIR accepts.

This preserves default compiler behavior while giving FE-5.3 and FE-5.4 a single report shape to
extend with classifier rows and VM/host execution observations.

## Checks

- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo test -p oxvba-compiler syntax_bridge --quiet`
- `cargo test -p oxvba-compiler compile_options_ --quiet`
- `cargo fmt -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- The harness intentionally lives in `oxvba-compiler`, so it cannot execute VM traces without
  creating a compiler-to-VM dependency cycle. The report shape records execution trace and
  observable output as explicit `NotRun` values rather than silently omitting them. FE-5.4 should
  add a higher-layer runner for VM/host observations.
- Metadata comparison is available for v2 because the diff observer now calls HIR production
  lowering directly and receives bytecode plus runtime metadata from the same front-end route.
- The fresh regression fixture `x = 1: x = x + 1` catches the prior blunder where v2 validation
  would have been reported even though real compilation still took the legacy-default path. The
  left side records the legacy diagnostic; the right side records HIR bytecode and metadata.
- Bytecode comparison is not required to be byte-identical as the final rule for the workset.
  This bead only supplies the normalized diff surface; FE-5.3 classifies differences as bugs,
  harmless drift, or intentional improvements.
- Default compile behavior remains HIR-first with fallback only for unsupported residuals.
  `compile_with_options(... frontend_v2: true)` reports HIR unsupported as a front-end error, and
  the diff harness is opt-in test/support surface.
- Remaining limitation: execution trace and host-visible output are still explicit `NotRun`
  compiler-layer placeholders. FE-5.4 must add higher-level VM/host runners before terminal
  production replacement can claim execution-observation parity.

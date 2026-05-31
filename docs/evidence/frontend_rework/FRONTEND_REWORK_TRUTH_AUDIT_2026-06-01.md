# Front-End Rework Truth Audit

Date: 2026-06-01
Bead: `bd-aprs.1.1`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Scope

This audit checks whether the front-end rework workset is aligned with the current repository
truth before execution beads start. It specifically reconciles:

- the existing `oxvba-syntax` syntax substrate,
- the existing `oxvba-languageservice` semantic bridge,
- the current compiler lowering and string-rewrite path,
- activation-frame state from `bd-xkwq`,
- per-instance project-object field state from `bd-1ufc`,
- the intended `frontend_v2` gate and non-byte-identical migration policy.

## Sources Checked

- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
- `crates/oxvba-syntax/src/lib.rs`
- `crates/oxvba-languageservice/src/lib.rs`
- `crates/oxvba-languageservice/src/semantic.rs`
- `crates/oxvba-compiler/src/project.rs`
- `crates/oxvba-vm/src/interpreter.rs`
- `crates/oxvba-runtime/src/object_ref.rs`
- bead records for `bd-aprs.1.1`, `bd-xkwq`, and `bd-1ufc`
- git history for `crates/oxvba-syntax` and `crates/oxvba-languageservice`

## Findings

1. The workset correctly treats `oxvba-syntax` as an existing partial substrate, not as work to
   invent from nothing. Git history confirms the crate was present at workspace bootstrap
   (`68965e4e`, 2026-02-26), then gained the larger language-service expression/accessor surface in
   `5f4da2f3` on 2026-03-23. The current crate exports the custom green/red tree, lexer, parser,
   `Parse`, `SyntaxNode`, `SyntaxToken`, and typed syntax kinds.

2. The production compiler path is still separate from that syntax substrate. The workset's S1/S3
   claims remain accurate: the compiler still relies on current project/compiler lowering and
   source-text surgery instead of feeding production lowering from the lossless CST.

3. `oxvba-languageservice` is a bridge, not the final compiler front-end authority. Its semantic
   snapshot parses through `oxvba_syntax::parse`, then resolves/checks through existing compiler
   APIs and correlates CST nodes back to current bound structures. This is useful precedent for
   editor-style queries, but the workset is correct to require reconciliation through shared
   HIR/SemanticModel APIs before broad IDE claims.

4. `rowan` and `cstree` are correctly documented as optional helper-library choices. The repo
   already has a custom green/red implementation, so a migration needs concrete measured benefit;
   it is not a prerequisite for the Roslyn-style architecture.

5. The runtime prerequisites named in the workset have landed. The VM contains an
   `activation_frames: Vec<ActivationFrame>` stack with frame slot lookup paths, and runtime
   object references now hold per-instance project fields in a single-threaded `RefCell` store with
   `project_field_get`/`project_field_set`. The workset should continue to build on those facts and
   must not reintroduce flat global register assumptions.

6. The `frontend_v2` switch is planned rather than present. That is not a gap in this preparation
   bead; it is assigned to `bd-aprs.6.1` / FE-5.1 and should remain stated as planned until that bead
   creates the gate.

7. The bytecode comparison policy is correctly stated. Migration gates should use semantic
   execution, diagnostics, metadata contracts, and documented intentional differences. Byte-for-byte
   bytecode identity is not required and should not be added as a closure condition.

## Residual Work

No material stale prerequisite claim was found in the workset. The remaining preparation work is
already assigned to later beads:

- decision-record cleanup for the Roslyn-style shape and helper-library options (`bd-aprs.1.2`),
- corpus inventory and better semantic/diff fixture sourcing (`bd-aprs.1.3`),
- formal grammar capture and coverage matrix (`bd-aprs.2.*`),
- substrate/library audit and any justified tree migration spike (`bd-aprs.3.*`),
- language-service reconciliation after the shared HIR/SemanticModel starts to exist
  (`bd-aprs.10.4`).

## Fresh-Eyes Notes

The main risk is not a false prerequisite, but over-trusting the current language-service bridge.
The workset already captures that as a risk and keeps the compiler authority with Excel/MS-VBAL
evidence plus the shared future HIR/SemanticModel. No further workset rewrite is needed for
`bd-aprs.1.1`.

## Checks

- `cargo test -p oxvba-syntax --quiet`: passed, 58 tests.
- `cargo test -p oxvba-languageservice --quiet`: passed, 57 library tests and 2 integration tests.
  The run emitted the existing `is_omitted_argument_expr` dead-code warning from
  `oxvba-compiler`.
- `git diff --check`: passed with line-ending warnings only for touched tracked files.

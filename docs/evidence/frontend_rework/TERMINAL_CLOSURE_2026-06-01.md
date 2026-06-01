# Terminal Closure Evidence - Superseded First-Run Artifact

Date: 2026-06-01
Bead: `bd-aprs.10.5`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

Superseded status, 2026-06-01:

This artifact records useful first-run checks and fixes, but it is not valid closure evidence for
the reopened workset. The first run passed compiler/syntax/VM/host checks while production
front-end replacement remained incomplete: `frontend_v2` was still an opt-in CST-validation bridge,
HIR/SemanticModel work was partly scaffold-level, and legacy `resolve.rs` / `project.rs`
production routes remained load-bearing. The workset has been reopened with a stronger terminal
gate requiring end-to-end production routing through CST -> binder -> HIR/SemanticModel -> HIR
lowering and retirement/quarantine of legacy production routes.

## Checks

Passed:

- `cargo test -p oxvba-compiler procedure_runtime_metadata_carries_expression_operator_and_coercion_descriptors -- --nocapture`
- `cargo test -p oxvba-compiler --quiet`
  - 924 passed.
- `cargo test -p oxvba-syntax --quiet`
  - 79 unit tests passed, 2 integration/doc-style tests passed.
- `cargo test -p oxvba-vm --quiet`
  - 20 unit tests passed, 3 package identity tests passed, 21 feature coverage tests passed.
- `cargo test -p oxvba-host --quiet`
  - full host crate passed, including the Access DAO/ADO, wrapped COM server, class lifetime, and
    project-hosting snapshot lanes that previously exposed terminal regressions.
- `cargo fmt --check -p oxvba-compiler -p oxvba-vm -p oxvba-host`
- `git diff --check`

## Terminal Fixes

- Compiler call-site metadata now keeps the runtime-facing argument `source_slot` as the
  descriptor-transfer temporary while carrying the caller variable type separately through
  `ArgumentBindingDescriptor::source_declared_type`.
- ParamArray call-site descriptors carry the packed array source slot, so descriptor-backed host
  ParamArray forwarding remains intact.
- VM descriptor evidence recognizes descriptor-native return copyout, ByRef writeback, optional
  default, ParamArray pack, and selected call-entry coercion observations.
- Host project-visible snapshots now use a VM-owned completed activation-frame snapshot keyed by
  procedure entry PC, rather than depending on the retained global register window after the
  startup shim returns.
- Completed-frame snapshots are captured after local/temp release and termination drain, preserving
  `Class_Terminate` timing. Terminating project-object references are sanitized to `Empty` in the
  evidence snapshot so the snapshot surface does not become an extra reference owner.

## First-Run Fresh-Eyes Review

- The earlier compiler metadata blocker was a real descriptor-shape bug, not a frontend workset
  documentation issue. It is fixed and covered by the full compiler crate.
- The host failures were a snapshot projection bug caused by descriptor-backed activation frames
  being popped before host projection. The fix is a narrow VM observation surface, not broad
  register mirroring.
- Byte-identical bytecode is not used as a closure criterion. The checks assert runtime behavior,
  metadata/evidence shape, descriptor observations, and host-visible snapshot behavior.
- These fixes remain valuable, but they do not close `bd-aprs.10.5` under the reopened production
  replacement criteria.

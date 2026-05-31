# Front-End Rework Decision Record

Date: 2026-06-01
Bead: `bd-aprs.1.2`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Decision

The front-end rework uses a Roslyn-style architecture as a shape:

- lossless immutable green syntax nodes,
- red/tree facade nodes with parent/position context,
- typed AST facades over syntax,
- binding and type information kept out of syntax,
- compiler-owned bound HIR for lowering,
- an IDE-facing SemanticModel query layer over CST nodes and bound facts,
- future incremental recomputation over parse/bind/typecheck/diagnostic queries.

This decision does not require Roslyn, rust-analyzer, `rowan`, or `cstree` as product
dependencies. The current custom `oxvba-syntax` green/red tree is the default substrate because it
already exists in repo and is covered by syntax tests.

## Helper-Library Policy

`rowan` and `cstree` remain optional migration candidates only. A migration is justified only if
the Phase-0 substrate audit/spike proves concrete benefit for the workset's actual needs:

- node/token identity and span stability,
- lossless round-trip behavior,
- typed facade ergonomics,
- parser integration,
- memory sharing and traversal cost,
- thread-safety requirements for the planned host shape,
- long-term maintainability.

Absent that evidence, the correct default is to harden the existing custom tree rather than churn
the substrate for fashion or terminology.

## Non-Decision

This bead does not choose the final interner, diagnostics renderer, query-engine shape, or CST
storage implementation. Those remain assigned to the Phase-0 decision/spike beads. This bead only
locks the vocabulary and removes ambiguity from the workset: Roslyn-style is the target shape;
helper libraries are implementation options.

## Fresh-Eyes Notes

The likely misconception to avoid is treating a named helper crate as the architecture. The
architecture is the separation of syntax, bound HIR, and SemanticModel queries. A helper crate can
support that separation, but cannot replace the binder/HIR/SemanticModel work.

## Checks

- `git diff --check`: passed with line-ending warnings only for touched tracked files.
- No code checks were required for this documentation-only decision cleanup.

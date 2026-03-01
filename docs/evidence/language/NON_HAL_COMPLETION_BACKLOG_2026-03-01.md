# NON_HAL_COMPLETION_BACKLOG_2026-03-01.md

Objective:
- Close remaining non-HAL language/runtime/library gaps that are implementable without external oracle dependency.
- Push oracle-dependent semantics into deferred-oracle gates for later foldback.
- Exclude HAL-adjacent features from this completion milestone.

## A. Implement-Now Targets (Non-HAL, Non-Oracle-Blocking)

1. Error model completion:
- Expand `Err` surface beyond current subset (`Number`, `Raise`, `Clear`) to include additional core fields and state transitions.
- Align procedure-entry/exit and post-success clearing behavior with documented rules where deterministically specified.

2. Runtime string/value model completion:
- Remove remaining placeholder/projection behavior in string operations where currently identity/count-only.
- `v150` completed array-tag-aware `Join` behavior; remaining string-sentinel and deeper parity work continues in `v151+`.
- Tighten `vbNullString` and String value-path invariants for non-boundary execution.
- `v151` adds compile-time `vbNullString` guard against numeric assignment/call targets; runtime/oracle parity remains open.

3. Financial intrinsic implementation upgrade:
- Replace deterministic projection placeholders for `NPV`, `IRR`, `MIRR`, `Rate`, `NPer` with real numeric algorithms.
- Add deterministic tolerance policy and stable numeric diagnostics.

4. UDT/value semantics hardening:
- Strengthen UDT copy/assignment/value-initialization behavior beyond flattened alias baseline where implementable without host interop.

5. Diagnostics and phase timing consistency:
- Stabilize compile-time vs runtime error timing for non-HAL language/runtime constructs.
- Ensure diagnostic IDs and category mapping remain stable.

## B. Explicitly Deferred To Oracle Gates (Non-HAL)

Tracked in:
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`
- `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`

Key deferred classes:
- edge semantics of control-flow/error transitions,
- ByRef temp/copy-back nuances,
- Null/Empty/Error coercion edge matrices,
- locale/time/random/format parity nuances,
- financial algorithm parity against host results.

## C. Excluded From This Milestone (HAL-Adjacent)

1. Host-sensitive runtime (`Shell`, `Environ`, `Dir`) parity expansion.
2. COM activation/dispatch parity beyond deterministic bridge.
3. Stateful file I/O host semantics parity.
4. UI/interaction surfaces (`MsgBox`, `InputBox`).
5. Rich external automation/type-library imports.

These remain tracked but outside the non-HAL completion ladder.

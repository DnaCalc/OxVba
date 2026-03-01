# DEFERRED_ORACLE_GATES.md

Deferred-oracle gate register for semantics requiring empirical validation against real VBA hosts.

Purpose:
- Track oracle-dependent semantics similarly to deferred formal gates.
- Keep implementation progress unblocked while preserving explicit reconciliation obligations.
- Separate non-HAL oracle topics from HAL-adjacent topics.

Machine-readable register:
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`

## Status Model

- `open`: deferred, no oracle run evidence captured yet.
- `running`: oracle probes/runs in progress.
- `foldback`: oracle evidence captured; implementation/test/doc alignment pending.
- `closed`: reconciled and reflected in implementation/tests/docs.
- `wont-fix`: intentionally left divergent with documented rationale.

## Reconciliation Rule

A deferred oracle gate can close only when all are present:
1. Oracle evidence artifact linked.
2. OxVba conformance fixture exists or is updated.
3. Divergence record updated if mismatch remains.
4. Topic status in `CONFORMANCE_CHECK_TOPICS.csv` updated accordingly.

## Current Scope Policy (2026-03-01)

- Non-HAL language/runtime/library oracle topics: tracked here and expected to fold back in later milestones.
- HAL-adjacent or host-sensitive topics: tracked here but excluded from the current non-HAL completion implementation ladder.

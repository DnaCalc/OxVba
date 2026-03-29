# Operational Incident Log

Status: `active`

Purpose:
- record process failures that affected planning, truth, or execution trust,
- capture root cause,
- record the doctrine/tooling change that prevents recurrence.

## Incident OI-001

- Date: 2026-03-29
- Title: `For Each` subset support was allowed to read as full closure
- Impact:
  - array `For Each` support was implemented and evidenced,
  - object-enumerator `For Each` remained unimplemented,
  - some active truth surfaces widened the claim and later work consumed that wording as if full support existed.
- Root cause:
  - subset-support truth was not enforced consistently across all active artifacts,
  - canonical validation matrices did not yet own the truth,
  - summaries could drift broader than evidence.
- Preventive changes:
  - canonical validation matrices introduced,
  - truth ownership map introduced,
  - conformance topics mapped to matrix owners,
  - closure-language doctrine tightened,
  - `For Each` arrays vs object-enumerators made the standing canary.

## Incident OI-002

- Date: 2026-03-29
- Title: Parallel bead graph mutations produced misleading `br create` results
- Impact:
  - multiple `br create` calls in parallel returned confusing ID stdout,
  - the intended epic rollout had to be audited against persisted bead state before further graph work could proceed safely.
- Root cause:
  - bead DB mutations were treated like read-only parallelizable operations,
  - no serialized wrapper or explicit single-writer doctrine existed.
- Preventive changes:
  - bead mutation serialization rule added to doctrine,
  - `scripts/invoke-br-serialized.ps1` introduced,
  - rollout work is now expected to use serialized graph mutations.

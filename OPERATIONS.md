# OPERATIONS.md — OxVBA Operations

## 1. Purpose
This document defines how OxVBA work is executed day-to-day: fast iteration, clear gates, evidence-backed compatibility claims, and low process overhead.

## 2. Operating Principles
- Correctness before optimization.
- Compatibility claims require reproducible evidence.
- Regressions become permanent tests.
- Keep process lightweight: only require artifacts that directly improve correctness, compatibility, or delivery confidence.

## 3. Execution Model
OxVBA follows the sequencing in `MACH1000_PLAN.md`.

Execution defaults:
- Build an end-to-end vertical slice early.
- Keep high-risk performance paths behind feature flags until parity/correctness gates are green.
- Use measurable phase gates (pass rates, divergence counts, benchmark thresholds).

## 4. Change Workflow
For behavior-affecting changes:
1. Implement code change.
2. Add/update tests (unit, conformance, or property tests as appropriate).
3. Update relevant docs (`MACH1000_PLAN.md`, design notes, or this file if doctrine changed).
4. Record compatibility evidence when claiming Office/VBA parity.

For bug fixes:
- Add a minimized regression case before or with the fix.

## 5. Evidence Discipline
Admissible evidence for compatibility claims:
- Public specifications/docs.
- Published research.
- Reproducible observation harness outputs.

Every compatibility claim should be traceable to a reproducible artifact (test case, harness output, or decision-table entry).

## 6. Testing and Gates
Minimum expectations before merge:
- Relevant crate tests pass.
- No new unexplained conformance divergences in touched behavior.
- For unsafe-sensitive areas, Miri/Kani lanes remain green where applicable.

Recommended routine:
- Fast local lane: `cargo test` for impacted crates.
- Full lane (CI): formatter/lints/tests plus heavier checks.
- For long-running Kani/profile formal steps, prefer async execution with repo scripts and log/state tracking, then merge results back into formal evidence artifacts.

## 7. Documentation and Synthesis
Use synthesis runs when changing plan-level direction or resolving multiple proposal inputs.

A synthesis run should include:
- frozen input hashes,
- suggestion index,
- per-suggestion decisions (`accept` / `adapt` / `defer` / `reject`),
- output report,
- manifest.

Not every change needs synthesis. Routine implementation changes should update docs directly.

## 8. Roles and Foundation Alignment
OxVBA is primarily a Rust delivery project within the broader DNA Calc structure.

- OxVBA maintains its own project-level charter, operations, and plan.
- Foundation doctrine remains the umbrella (clean-room, evidence, stabilization discipline).
- OxVBA process intentionally avoids heavyweight pack bureaucracy unless complexity demands it.

## 9. Definition of Done (Practical)
A change is done when:
- behavior is implemented,
- tests and evidence are updated,
- docs reflect the new truth,
- CI gates for the touched area are green.

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
Execution doctrine details and run hygiene are captured in `docs/LOCAL_EXECUTION_DOCTRINE.md`.

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

Commit discipline for ladder execution:
- Prefer split commits:
  - commit A: code/spec/docs changes,
  - commit B: evidence refresh artifacts.
- Run staged-scope guard before commit:
  - `./scripts/check-staged-commit-scope.ps1`
- For final validation before commit, prefer no-artifact mode:
  - `./scripts/meta-check.ps1 -Fast -NoArtifacts`
- Governance-only pass (fast, no compile):
  - `./scripts/check-governance.ps1`

For bug fixes:
- Add a minimized regression case before or with the fix.

## 5. Evidence Discipline
Admissible evidence for compatibility claims:
- Public specifications/docs.
- Published research.
- Reproducible observation harness outputs.

Every compatibility claim should be traceable to a reproducible artifact (test case, harness output, or decision-table entry).

Artifact naming convention:
- Prefer `.jsonl` over `.ndjson` for line-delimited JSON evidence/telemetry files.

PMR/event diagnostic source of truth:
- Canonical registry: `docs/evidence/diagnostics/PMR_EVENT_DIAGNOSTICS_V1.csv`
- Generated snippets (must be in sync): `docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md`
- Regenerate/check via:
  - `./scripts/generate-pmr-event-diagnostic-snippets.ps1`
  - `./scripts/generate-pmr-event-diagnostic-snippets.ps1 -Check`

## 6. Testing and Gates
Minimum expectations before merge:
- Relevant crate tests pass.
- No new unexplained conformance divergences in touched behavior.
- For unsafe-sensitive areas, Miri/Kani lanes remain green where applicable.

Recommended routine:
- Fast local lane: `cargo test` for impacted crates.
- Full lane (CI): formatter/lints/tests plus heavier checks.
- For long-running Kani/profile formal steps, prefer async execution with repo scripts and log/state tracking, then merge results back into formal evidence artifacts.
- Run governance checks early for doc/spec/conformance changes:
  - `./scripts/check-governance.ps1`

Additional required local checks for doc-heavy profile ladder runs:
- `./scripts/validate-profile-scaffold.ps1 -FromVersion <start> -ToVersion <end>`
- `./scripts/check-hal-clause-drift.ps1` (when HAL clause/spec surfaces are touched)

## 7. Documentation and Synthesis
Use synthesis runs when changing plan-level direction or resolving multiple proposal inputs.

A synthesis run should include:
- frozen input hashes,
- suggestion index,
- per-suggestion decisions (`accept` / `adapt` / `defer` / `reject`),
- output report,
- manifest.

Not every change needs synthesis. Routine implementation changes should update docs directly.

Post-semantics-change checklist (required when diagnostics or conformance semantics change):
1. Update canonical diagnostic manifest and regenerate snippets (`PMR_EVENT_DIAGNOSTICS_V1.csv` + generated files).
2. Update active conformance/spec docs (`CONFORMANCE_CHECK_TOPICS.csv`, `DEFERRED_ORACLE_GATES.csv`, integration catalog, divergence notes).
3. Ensure deferred-gate structured fields are consistent (`foldback_required`, `foldback_steps`, `close_condition`).
4. Run `./scripts/check-governance.ps1`.
5. Run impacted crate tests and `./scripts/meta-check.ps1 -Fast -NoArtifacts` before merge.

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

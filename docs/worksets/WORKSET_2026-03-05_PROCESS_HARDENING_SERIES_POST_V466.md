# Workset: Process Hardening Series (Post-v466)

Date: 2026-03-05  
Scope: operational hardening after COM early-binding ladder closure (`v466`)  
Status: planned

## Objectives

1. Eliminate accidental evidence churn during validation cycles.
2. Enforce cleaner commit hygiene (code/spec vs evidence refresh).
3. Prevent profile-scope drift (editing historical profile artifacts unintentionally).
4. Stabilize run identity across conformance/perf cycles.
5. Keep evidence storage bounded without losing useful history.
6. Strengthen machine-readable gate outputs for automation.
7. Improve formal-lane observability with structured, low-noise output.

## Hardening Ladder

### HN1 - No-Artifact Validation Default
- Deliverables:
  - `meta-check` support for `-NoArtifacts`.
  - matrix/formal/conformance lanes support no-artifact redirection to `temp/no-artifacts/...`.
- Evidence:
  - dry run with `./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal -NoArtifacts`.
- Exit:
  - no tracked `LATEST` files change in no-artifact mode.

### HN2 - Stable Run-ID Discipline
- Deliverables:
  - shared run-id resolver with lock-aware reuse window.
  - conformance/perf runners accept explicit `-RunId` and support deterministic run grouping.
- Evidence:
  - repeated same-cycle runs reuse run-id without duplicate timestamped churn.
- Exit:
  - run-id consistency visible in lane summaries.

### HN3 - Commit Scope Split Guard
- Deliverables:
  - staged-scope checker (`code/spec` vs `docs/evidence` mixed-set detection).
  - operator guidance in `OPERATIONS.md` and testing docs.
- Evidence:
  - scripted failure on mixed staged set (unless explicit override).
- Exit:
  - default workflow supports two-commit closure pattern.

### HN4 - Profile Artifact Scope Guard
- Deliverables:
  - staged/working guard that blocks profile artifact edits outside active ladder range by default.
- Evidence:
  - synthetic test: changing `docs/evidence/profiles/v<old>/...` fails under active newer ladder.
- Exit:
  - accidental historical profile mutations are stopped early.

### HN5 - Evidence Retention and Prune
- Deliverables:
  - prune utility implementing `LATEST + N` retention for timestamped evidence runs.
  - documented housekeeping policy.
- Evidence:
  - dry-run + live-run logs with explicit deleted/would-delete counts.
- Exit:
  - repeatable evidence-size control workflow in place.

### HN6 - Gate Manifest Formalization
- Deliverables:
  - machine-readable `gate.json` emitted alongside integrated gate reports.
  - markdown gate report generated from the same manifest data.
- Evidence:
  - integrated gate output contains `gate.json` + `integrated_gate.md`.
- Exit:
  - gate consumption can be automated without parsing markdown.

### HN7 - Formal Lane Structured Output and Quiet Mode
- Deliverables:
  - formal lane JSONL output (`latest_run.jsonl`).
  - quieter default reporting with optional verbose failure surfacing.
- Evidence:
  - formal run artifacts include markdown, csv, and jsonl records.
- Exit:
  - long formal runs are monitorable with reduced terminal noise.

### HN8 - Adoption and Backfill Hygiene
- Deliverables:
  - update script/docs inventories for new options and guardrails.
  - optional targeted backfill plan for historical gate manifests (non-blocking).
- Evidence:
  - docs references are current and command examples are executable.
- Exit:
  - operators can reliably use hardened flow without implicit tribal knowledge.

## Deferred/Optional Follow-ups

1. Add CI lane that runs `meta-check -NoArtifacts` and both commit-scope guards.
2. Add pre-commit hook template wiring `check-staged-commit-scope` and `validate-profile-artifact-scope`.
3. Backfill `gate.json` for selected historical terminal gates (`v386`, `v406`, `v466`) for downstream tooling consistency.
4. Add retention policy exceptions for milestone-tagged evidence bundles.

## Notes

- This series is operational and does not change language/runtime semantics.
- Formal failures remain non-blocking under existing deferred-gate policy unless policy is explicitly tightened.

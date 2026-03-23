# Review Triage Plan — 2026-03-09

## Purpose

This document defines how to triage [`docs/REVIEW_20260309.md`](./REVIEW_20260309.md) into three incorporation files:

- `docs/REVIEW_20260309_PROCEED.md`
- `docs/REVIEW_20260309_DEFER.md`
- `docs/REVIEW_20260309_FOLLOWUP.md`

Goal: convert a wide, mixed-quality review corpus into a controlled backlog that is actionable, auditable, and aligned with the active MACH1000 ladder.

## Scope

This triage covers recommendation-bearing material in:

- General code review items `1` through `10`
- HAL interface review items `H1` through `H11`
- Project proposal review items `PR-*`
- Spec/code drift items `SD-*`
- Cleanup backlog items `CB-*`
- Security/test/error-handling recommendations where they imply concrete work

Narrative praise, maturity estimates, and descriptive observations are not triaged unless they imply a concrete decision or work item.

## Output Files

### `docs/REVIEW_20260309_PROCEED.md`

Use for items that should become an executable workset now.

A `PROCEED` item must satisfy all of:

- aligned with current ladder or immediate enabling work
- low ambiguity
- no unresolved product decision needed
- can be expressed as a bounded workset with checks

### `docs/REVIEW_20260309_DEFER.md`

Use for items that are accepted in principle but should wait.

A `DEFER` item is appropriate when:

- value is real but timing is wrong
- current ladder does not depend on it
- the change is large and not needed for the next gate
- the area is intentionally scaffold/future-facing

### `docs/REVIEW_20260309_FOLLOWUP.md`

Use for items that need user guidance or a deliberate design decision.

A `FOLLOWUP` item should be rare. Use it only when:

- multiple plausible directions exist
- the choice is materially architectural
- local repo context does not settle the decision safely

## Triage Rules

1. One atomic item per entry.
2. Preserve provenance to the original review section.
3. Deduplicate repeated recommendations into one canonical entry with multiple sources.
4. Record decision rationale, not just the destination bucket.
5. Reject or mark not-applicable inside the entry notes when needed; do not force weak items into `DEFER`.
6. Bias toward `PROCEED` for correctness, compatibility, spec-drift, and safety items that are immediately actionable.
7. Bias toward `DEFER` for cleanup, architectural polish, and future-platform work not needed for the active gate.
8. Use `FOLLOWUP` only for real decisions, not for work that is merely large.

## Normalized Entry Template

All three triage files should use the same base shape.

```md
## [ID] Short Title

- Status: proceed | defer | followup | rejected
- Source: docs/REVIEW_20260309.md:line or section
- Additional sources: optional list
- Summary: one-paragraph normalized statement of the review point
- Why it matters: correctness | compatibility | safety | maintainability | delivery
- Decision: short disposition statement
- Rationale: why this bucket is correct for OxVba now
- Duplicates merged: optional list
- Next step: immediate action, deferral trigger, or user question
```

## Bucket-Specific Requirements

### `PROCEED` requirements

Each `PROCEED` entry must also contain:

- Proposed workset name
- Ladder steps
- Dependencies
- Verification

Template extension:

```md
- Proposed workset: WORKSET_...
- Ladder:
  - step 1
  - step 2
  - step 3
- Dependencies: none | list
- Verification: tests / governance / evidence to run
```

### `DEFER` requirements

Each `DEFER` entry must also contain:

- Why deferral is safe now
- Revisit trigger

Template extension:

```md
- Safe to defer because: ...
- Revisit when: ...
```

### `FOLLOWUP` requirements

Each `FOLLOWUP` entry must also contain:

- Exact unresolved question
- Options
- Recommendation
- Cost of delay

Template extension:

```md
- Question: ...
- Options:
  1. ...
  2. ...
  3. ...
- Recommendation: ...
- Cost of delay: ...
```

## Suggested Decision Heuristics For This Review

### Strong `PROCEED` candidates

These are immediately actionable and tightly tied to correctness, safety, or doc drift:

- fix hardcoded `HalProfileId::Windows` defaults
- consolidate VM reset sequencing
- deduplicate shared constants
- add cross-platform HAL smoke coverage
- refresh or supersede stale bytecode/spec docs
- update architecture docs to reflect the actual crate graph
- add `// SAFETY:` annotations in the COM FFI hot spots

### Strong `DEFER` candidates

These appear valuable but not critical to the active ladder gate:

- major `standard.rs` modularization
- broad HAL trait decomposition
- moving COM state into `oxvba-com`
- script taxonomy cleanup
- future C API phasing
- non-critical schema expansion such as dependency version fields
- CI/platform expansion beyond current execution needs

### Likely `FOLLOWUP` candidates

These need a project-level choice:

- whether `oxvba-com` should be deleted or repurposed
- UI framework direction for DNA VbCalc pathfinder
- exact event dispatch contract across host bridge and engine
- project reload semantics for the embedded host path

## Initial Processing Order

To maximize value and reduce noise, triage review sections in this order:

1. `SD-*` spec/code drift items
2. General review items `2`, `3`, `4`, `6`, `9`, `10`
3. Security recommendation on `// SAFETY:` annotations
4. Error/catalog and documentation quick wins
5. HAL review items `H1` through `H11`
6. Project proposal review items `PR-*`
7. Cleanup backlog `CB-*`
8. Remaining descriptive sections only if they imply a distinct action

Rationale:

- doc drift and correctness items convert cleanly into work
- smaller concrete items establish triage discipline early
- large HAL/host architecture suggestions benefit from seeing what remains after the concrete pass

## Deduplication Map

Use these as canonical merges during triage:

- `release_object` gap:
  - general project review / `PR-1`
  - HAL review `H5`

- multi-arg or explicit event dispatch concerns:
  - general review item `5`
  - HAL review `H2`
  - HAL review `H3`
  - project review `PR-1` event-dispatch/marshaling concerns

- `standard.rs` decomposition / test fixture isolation:
  - general review item `1`
  - HAL review `H9`
  - security review `unsafe` surface concerns

- cross-platform/profile correctness:
  - general review item `4`
  - general review item `10`
  - cleanup item `CB-16`

- `oxvba-com` purpose decision:
  - HAL review `H6`
  - cleanup items `CB-1`, `CB-2`, `CB-13`

## Triage Quality Bar

A triage pass is acceptable only if:

- every entry is atomic
- every entry cites source provenance
- repeated advice is merged
- `PROCEED` items are workset-ready
- `FOLLOWUP` items are decision-ready
- bucket rationale is explicit and specific to current ladder timing

## Expected Next Artifact

After this plan, create:

1. `docs/REVIEW_20260309_PROCEED.md`
2. `docs/REVIEW_20260309_DEFER.md`
3. `docs/REVIEW_20260309_FOLLOWUP.md`

and populate them in the processing order above.

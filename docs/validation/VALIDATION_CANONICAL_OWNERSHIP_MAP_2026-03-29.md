# Validation Canonical Ownership Map — 2026-03-29

Status: `active`  
Driver workset: `docs/worksets/WORKSET_2026-03-29_VALIDATION_MATRIX_RESET_AND_BEAD_EXECUTION_REFORM.md`

## Purpose

This file states, per domain, which artifact is allowed to carry active implementation truth.

It exists to stop the repo from drifting back into competing truth surfaces where:
1. one file carries exact subset boundaries,
2. another file summarizes more broadly,
3. the broader summary is then treated as the real status.

The rule is simple:
- each domain has one canonical truth matrix,
- authority/spec docs remain authority docs,
- gate registers remain gate registers,
- summaries are derived only and must not contradict the canonical matrix.

## Domain Ownership

| Domain | Canonical truth owner | Authority sources | Gate / secondary active artifacts | Artifacts to rewrite or archive |
|---|---|---|---|---|
| Language | `docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv` | `VBAL` obligations, `docs/evidence/SPEC_CHECKLIST.md` structure, related language specs and extracted requirement docs | `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`, `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv` | rewrite `docs/evidence/language/COVERAGE_INDEX.csv`; rewrite `docs/evidence/SPEC_CHECKLIST.md`; archive broad historical language closure/status files |
| COM / External Integration | `docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv` | `docs/spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md`, `docs/spec/COM_EARLY_BINDING_TYPELIB_CONFORMANCE_V1.md`, `docs/spec/COM_CLIENT_SERVER_CONFORMANCE_V1.md`, related COM specs | `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`, `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`, oracle capture directories | rewrite COM-domain topic truth out of broad summary files; archive broad COM closure/status files as historical only |
| Project / Hosting | `docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv` | `docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md`, explicit OxVba extension docs, `README.md` as user-facing guide | `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`, `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv` | archive `docs/INITIAL_SCOPE_STATUS_2026-03-24.md` as historical status; rewrite project-hosting truth out of broad summary files |
| Language Services / Formalization | `docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv` | `docs/spec/LANGUAGE_SERVICE_SPEC_V1.md`, `docs/spec/HAL_FORMALIZATION_PROGRAM.md`, `MACH1000_PLAN.md`, related formal docs | formal gate/evidence docs and future service-specific inventories | rewrite `docs/FORMAL.md` as derived overview if retained; avoid treating design/spec docs as implementation truth |

## Artifact Roles

### Canonical Matrix

Allowed to answer:
1. what the feature or obligation is,
2. what exact subset is supported,
3. what compiler/interpreter/JIT state exists,
4. what oracle/evidence state exists,
5. whether the feature is planned, in-progress, implemented-subset, implemented-full, or verified.

### Authority Source

Allowed to answer:
1. what the intended behavior or scope should be,
2. which design or reference clauses govern the feature,
3. what OxVba extension decision was made.

Not allowed to answer by itself:
1. current implementation truth,
2. current engine parity state,
3. current closure state.

### Gate Register

Allowed to answer:
1. what external oracle or deferred check is still open,
2. what evidence closes the gate,
3. what foldback steps are required.

Not allowed to answer by itself:
1. general feature completion truth.

### Derived Summary

Allowed to exist only if:
1. it points back to canonical matrix rows,
2. it preserves subset boundaries,
3. it does not widen any claim made by the matrix.

## Immediate Enforcement Rules

1. If a summary and a matrix disagree, the matrix wins and the summary must be fixed or archived in the same cycle.
2. If a gate register and a matrix disagree, the matrix keeps feature truth and the gate register keeps execution-state truth; cross-links must then be repaired.
3. If a feature cannot be represented precisely enough in a summary, the summary must defer to the matrix row instead of compressing the claim.
4. The standing canary remains `For Each`:
   - array `For Each` lives in the language matrix as a supported subset,
   - object-enumerator `For Each` lives in the language matrix as in-progress,
   - no summary file may merge those states into one closure claim.

## Next Actions

1. map each active `CONFORMANCE_CHECK_TOPICS.csv` row to one canonical matrix owner,
2. rewrite or demote `docs/evidence/language/COVERAGE_INDEX.csv`,
3. rewrite or demote `docs/evidence/SPEC_CHECKLIST.md`,
4. archive `docs/INITIAL_SCOPE_STATUS_2026-03-24.md` and broad historical closure/status surfaces,
5. create derived summaries only after their canonical matrix rows exist.

Current topic mapping artifact:
- `docs/validation/CONFORMANCE_TOPIC_MATRIX_MAP_2026-03-29.csv`

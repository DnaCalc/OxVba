# Validation Reset Audit

Date: 2026-03-29  
Status: in-progress  
Driver workset: `docs/worksets/WORKSET_2026-03-29_VALIDATION_MATRIX_RESET_AND_BEAD_EXECUTION_REFORM.md`

## Purpose

This audit is the staging document for replacing unreliable or over-broad active truth artifacts with fresh canonical validation matrices.

This file is not itself the canonical feature matrix.
It is the transition ledger that records:
1. which active artifacts still drive truth today,
2. whether they remain trustworthy,
3. whether they must be retained, rewritten, split, or archived.

## Classification Rules

Use one of:
1. `retain-active`
2. `rewrite`
3. `split`
4. `archive`

Use `split` when one artifact currently merges materially different subset states under a single broad feature label.

## Initial Audit Targets

| Artifact | Current role | Initial classification | Canonical replacement / retained role | Reason |
|---|---|---|---|---|
| `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv` | active conformance truth index | rewrite | `docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv` plus `docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv` and `docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv` | needs matrix-backed subset precision and clearer domain separation |
| `docs/evidence/language/COVERAGE_INDEX.csv` | active language coverage summary | split | `docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv` | merges subset and full-feature claims too loosely |
| `docs/evidence/SPEC_CHECKLIST.md` | spec obligation summary | rewrite | all four validation matrices, keyed by domain | useful structure, but not currently a canonical matrix with execution-engine detail |
| `docs/INITIAL_SCOPE_STATUS_2026-03-24.md` | broad active status summary | archive | historical status only; not replaced one-for-one | mixes current truth with historical closure narrative; too coarse to remain primary |
| `docs/worksets/PROFILE_LADDER_2026-02-28_MACH1000_V107_V146_FULL_VBA_LANGUAGE_BUILTINS.md` | historical ladder/status surface | archive | historical status only | historical planning artifact; contains broad language closure wording |
| `docs/worksets/PROFILE_LADDER_2026-02-28_MACH1000_V87_V106_LANGUAGE_COMPLETION.md` | historical ladder/status surface | archive | historical status only | historical planning artifact; should not remain active conformance truth |

## Expanded Domain Audit

### Language Domain

| Artifact | Current role | Classification | Canonical replacement / retained role | Notes |
|---|---|---|---|---|
| `docs/evidence/language/COVERAGE_INDEX.csv` | active language coverage truth | split | `docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv` | split broad rows into exact semantic subsets |
| `docs/evidence/SPEC_CHECKLIST.md` | prose summary of language/spec obligations | rewrite | `docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv` | keep only as a derived operator summary once matrix is authoritative |
| `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv` rows in language domains | oracle/topic truth | rewrite | `docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv` + retained gate/topic docs | topic tracking should survive, but truth state must live in the matrix |

### COM / External Integration Domain

| Artifact | Current role | Classification | Canonical replacement / retained role | Notes |
|---|---|---|---|---|
| `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv` rows in interop domains | oracle/topic truth | rewrite | `docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv` + retained topic register | split supported subsets from broader external parity claims |
| `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv` | gate register for deferred or open oracle work | retain-active | retained as execution gate register; cross-linked from canonical matrices | this is not a feature-truth matrix, but it remains the gate register |
| `docs/spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md` | authority/spec source | retain-active | authority source for matrix rows | keep as authority, not as truth summary |
| `docs/spec/COM_EARLY_BINDING_TYPELIB_CONFORMANCE_V1.md` | authority/spec source | retain-active | authority source for matrix rows | keep as authority, not as truth summary |
| `docs/spec/COM_CLIENT_SERVER_CONFORMANCE_V1.md` | authority/spec source | retain-active | authority source for matrix rows | keep as authority, not as truth summary |

### Project / Hosting Domain

| Artifact | Current role | Classification | Canonical replacement / retained role | Notes |
|---|---|---|---|---|
| `docs/INITIAL_SCOPE_STATUS_2026-03-24.md` | broad project/hosting status narrative | archive | `docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv` for active truth | current file mixes multiple domains and historical closure language |
| `docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md` | authority/spec source for project/hosting features | retain-active | authority source for matrix rows | keep as design authority, not as implementation truth |
| `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv` rows in project domains | startup/import/oracle topic truth | rewrite | `docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv` + retained topic register | startup and imported-runtime questions need matrix-scoped subset boundaries |

### Language Services / Formalization Domain

| Artifact | Current role | Classification | Canonical replacement / retained role | Notes |
|---|---|---|---|---|
| `docs/spec/LANGUAGE_SERVICE_SPEC_V1.md` | authority/spec source | retain-active | authority source for matrix rows | keep as scope authority for language-service work |
| `docs/FORMAL.md` | formal-program summary | rewrite | `docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv` | useful overview, but not precise enough to be the active truth carrier |
| `docs/spec/HAL_FORMALIZATION_PROGRAM.md` | authority/program source | retain-active | authority source for matrix rows | keep as authority, not as a substitute for matrix truth |

## First Canary

The first required process check is `For Each`.

The reset is only valid if the active matrices end up distinguishing at least:
1. `For Each` over arrays
2. `For Each` over object enumerators / `NewEnum`

If those remain merged under one active closure label, the reset has failed.

## Next Audit Steps

1. expand this ledger to all active truth surfaces in language, COM, project/hosting, and language-services/formalization,
2. mark an explicit canonical replacement for each archived or rewritten artifact,
3. ensure only one active source-of-truth family remains per domain.

## Immediate Retirement / Rewrite Queue

The next queued actions under this audit are:
1. rewrite the language-domain active truth so `For Each` over arrays and `For Each` over object enumerators are separate matrix rows with separate evidence,
2. classify `CONFORMANCE_CHECK_TOPICS.csv` topic-by-topic by target canonical matrix domain,
3. move broad historical status surfaces into an archive lane so they stop competing with matrix truth,
4. define how retained gate registers (`DEFERRED_ORACLE_GATES.csv`) cross-link to the canonical matrices without becoming duplicate truth stores.

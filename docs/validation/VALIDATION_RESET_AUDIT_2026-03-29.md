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

Canonical ownership reference:
- `docs/validation/VALIDATION_CANONICAL_OWNERSHIP_MAP_2026-03-29.md`

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

### 2026-03-29 COM/External Walk

Verification pass:
- `COM-0001` remains `implemented-subset` for controlled late-bound marshalling on scalar/object/array lanes.
- `COM-0002` remains `verified` for class-module COM export metadata flow.
- `COM-0003` is `verified` for TestEventServer typelib-driven binding and source-interface callbacks.
- `COM-0004` is `verified` for real registered `Scripting.Dictionary` `As New` activation.
- `COM-0005` is `verified` for dual-interface dispatch-vs-vtable transition stability.
- `COM-0006` is `verified` for file-backed typelib version selection and broken-reference repair.

Checked evidence:
- `crates/oxvba-host/tests/com_client_end_to_end.rs`
- `crates/oxvba-host/tests/com_client_registered_lane.rs`
- `crates/oxvba-host/tests/com_early_project_end_to_end.rs`
- `crates/oxvba-build/tests/com_attribute_export_end_to_end.rs`
- `docs/evidence/conformance/oracle_captures/com_testeventserver_marshaling_oracle_20260325T231210Z/summary.md`
- `docs/evidence/conformance/oracle_captures/com_testeventserver_oracle_20260325T221949Z/summary.md`
- `docs/evidence/conformance/oracle_captures/com_early_oracle_20260325T145433Z/summary.md`
- `docs/evidence/conformance/oracle_captures/com_dualinterface_oracle_20260325T224113Z/summary.md`
- `docs/evidence/conformance/oracle_captures/com_testeventserver_versioned_typelib_probe_20260325T222709Z/summary.md`

Result:
- The canonical COM matrix now separates late-bound marshalling, export metadata, TestEventServer typelib binding, registered `As New` activation, dual-interface strategy, and versioned/broken-reference repair into explicit rows.
- No unsupported COM claim was widened beyond its bounded subset.

### Project / Hosting Domain

| Artifact | Current role | Classification | Canonical replacement / retained role | Notes |
|---|---|---|---|---|
| `docs/INITIAL_SCOPE_STATUS_2026-03-24.md` | broad project/hosting status narrative | archive | `docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv` for active truth | current file mixes multiple domains and historical closure language |
| `docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md` | authority/spec source for project/hosting features | retain-active | authority source for matrix rows | keep as design authority, not as implementation truth |
| `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv` rows in project domains | startup/import/oracle topic truth | rewrite | `docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv` + retained topic register | startup and imported-runtime questions need matrix-scoped subset boundaries |

### 2026-03-29 Project/Hosting Walk

Verification pass:
- `PH-0001` remains `implemented-subset` for the executable startup ladder on explicit entrypoint, unique top-level mainline, and unique `Sub Main` fallback.
- `PH-0002` remains `implemented-subset` for top-level executable mainline behavior in program/script lanes, including the bounded module-state slice already documented in the matrix notes.
- `PH-0003` remains `implemented-subset` for the strict VBP-S0 adapter subset covering executable startup and ordered reference handling, with designer/startup-object surfaces still excluded.

Checked evidence:
- `crates/oxvba-host/tests/startup_entry_end_to_end.rs`
- `crates/oxvba-host/tests/project_hosting_examples_end_to_end.rs`
- `crates/oxvba-cli/src/main.rs` run-project unit tests
- `docs/evidence/conformance/project_integration/PROJECT_INTEGRATION_SUITE_LATEST.md`

Result:
- The bounded verification pass supports the startup/discovery/VBP subset claims on `PH-0001` through `PH-0003`.
- The canonical project/hosting matrix is now widened beyond the original three starter rows to capture adjacent honest lanes for host-project behavior, imported attribute runtime behavior, host-sensitive policy, and MS-OVBA roundtrip as separate rows rather than compressing them into one broad claim.
- Open follow-up remains governed by the existing topic and deferred-gate records, especially `CCT-045`, `CCT-049`, `CCT-050`, and `ODG-043`, rather than by this bead.

### Language Services / Formalization Domain

| Artifact | Current role | Classification | Canonical replacement / retained role | Notes |
|---|---|---|---|---|
| `docs/spec/LANGUAGE_SERVICE_SPEC_V1.md` | authority/spec source | retain-active | authority source for matrix rows | keep as scope authority for language-service work |
| `docs/FORMAL.md` | formal-program summary | rewrite | `docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv` | useful overview, but not precise enough to be the active truth carrier |
| `docs/spec/HAL_FORMALIZATION_PROGRAM.md` | authority/program source | retain-active | authority source for matrix rows | keep as authority, not as a substitute for matrix truth |

### 2026-03-29 Language/Services/Formalization Walk

Verification pass:
- `LSF-0001` remains `in-progress` as the bounded syntax/service inventory row. The checked evidence supports the internal service-surface claim, but not LSP parity or complete language-feature coverage.
- `LSF-0002` remains `in-progress` as the bounded formalization row. The checked evidence supports the presence of scaffolded formal artifacts and the current obligation registry, but not proof closure.

Checked evidence:
- `crates/oxvba-syntax/src/lexer.rs`
- `crates/oxvba-syntax/src/parser.rs`
- `crates/oxvba-syntax/src/red.rs`
- `crates/oxvba-languageservice/src/semantic.rs`
- `crates/oxvba-languageservice/src/service.rs`
- `crates/oxvba-languageservice/src/workspace.rs`
- `formal/lean/OxVba/VarType.lean`
- `formal/lean/OxVba/Coerce.lean`
- `formal/lean/OxVba/Arithmetic.lean`
- `formal/lean/OxVba/RefCount.lean`
- `docs/evidence/formal/MANIFEST.md`
- `docs/evidence/formal/INVENTORY.md`
- `docs/evidence/formal/obligations.csv`
- `docs/evidence/formal/latest_run.md`

Result:
- The canonical language-services/formalization matrix is now anchored to the concrete syntax, semantic-snapshot, and formal-registry artifacts that actually exist in the repo.
- No additional row split was required for this pass; the current `LSF-0001` / `LSF-0002` boundary remains the honest split between service-surface inventory and formalization scaffolding.
- Broader executable language semantics remain owned by `docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv`, not by this matrix.
- Bounded walk record: `docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_WALK_2026-03-29.md`.

## First Canary

The first required process check is `For Each`.

The reset is only valid if the active matrices end up distinguishing at least:
1. `For Each` over arrays
2. `For Each` over object enumerators / `NewEnum`

The first concrete implementation-ready blocker surfaced by the current walks is `LANG-0002`:
- object/class enumeration via `NewEnum` / `IEnumVARIANT` remains `in-progress`,
- the dependent `PH-0008` imported `NewEnum` runtime row stays downstream until `LANG-0002` is addressed.
- the scoped implementation bead is `bd-gm3.14.2.1`.
- the remaining imported/COM-backed split was rolled down through `bd-gm3.14.2.1.1` into:
  - `bd-gm3.14.2.1.1.1` for controlled imported COM `DISPID_NEWENUM` / `IUnknown` / `IEnumVARIANT` transport,
  - `bd-gm3.14.2.1.1.2` for the imported class-field `Collection`/`NewEnum` oracle-project shape.

If those remain merged under one active closure label, the reset has failed.

## Next Audit Steps

1. expand this ledger to all active truth surfaces in language, COM, project/hosting, and language-services/formalization,
2. mark an explicit canonical replacement for each archived or rewritten artifact,
3. ensure only one active source-of-truth family remains per domain.

## Immediate Retirement / Rewrite Queue

The next queued actions under this audit are:
1. rewrite the language-domain active truth so `For Each` over arrays and `For Each` over object enumerators are separate matrix rows with separate evidence,
2. carry `bd-gm3.14.2.1` through the bounded project-dynamic execution slice for `LANG-0002`, then continue the split imported lane through `bd-gm3.14.2.1.1.1` and `bd-gm3.14.2.1.1.2`,
3. classify `CONFORMANCE_CHECK_TOPICS.csv` topic-by-topic by target canonical matrix domain,
4. move broad historical status surfaces into an archive lane so they stop competing with matrix truth,
5. define how retained gate registers (`DEFERRED_ORACLE_GATES.csv`) cross-link to the canonical matrices without becoming duplicate truth stores.

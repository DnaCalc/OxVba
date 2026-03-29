# Workset: Validation Matrix Reset and Bead Execution Reform

Date: 2026-03-29  
Status: in-progress  
Scope: reset active conformance/completion truth, rebuild validation matrices across the main OxVba domains, and harden execution doctrine so subset support cannot be silently closed as full feature support.

## 0. Accepted Plan State

User acceptance for this reset plan was given on 2026-03-29.

This document is therefore the active umbrella plan for the reset.

Execution note:
1. the workset remains the umbrella planning document,
2. execution proceeds through the bead subtree rooted at `bd-gm3`,
3. this workset is expected to roll out into explicit phase epics,
4. first active canary remains the split between array `For Each` and object-enumerator `For Each`.

## 1. Purpose

This workset exists because the current repo truth has drifted.

The immediate trigger is the discovery that:
1. `For Each` is implemented for an array subset,
2. some documents recorded that subset honestly,
3. other documents and status surfaces used broader closure language,
4. later work then consumed that broader wording as if full `For Each` support existed.

This is a process and evidence failure, not just a missing feature.

The goal of this workset is therefore twofold:
1. re-establish confidence in what OxVba is supposed to support, against reference specs and explicit OxVba extension specs,
2. re-establish confidence in what OxVba actually supports, through fresh canonical validation matrices and systematic execution against them.

## 2. Required Outcomes

This workset is complete only when all of the following are true:
1. active work is executed through workset-generated bead subtrees rather than broad narrative slices,
2. active doctrine explicitly forbids closure language for unlabeled subsets,
3. fresh canonical validation matrices exist for the major OxVba domains,
4. unreliable legacy active truth artifacts are either rewritten, split, or archived,
5. the new matrices are being used to drive systematic verification of compiler, interpreter, JIT, oracle, and formal-model coverage,
6. the process catches the known `For Each` array-vs-object split as a deliberate in-progress distinction rather than allowing it to remain implicitly closed.

## 3. Main Domains To Rebuild

The reset covers four primary validation domains.

### 3.1 Language

This includes:
1. syntax and parser surface,
2. binding and typechecking,
3. lowering,
4. interpreter execution,
5. JIT execution,
6. subset boundaries for statements, expressions, builtins, and runtime semantics.

### 3.2 COM / External Integration

This includes:
1. late-bound COM,
2. early-bound typelibs and references,
3. COM server/export behavior,
4. marshaling and metadata publication,
5. real external host/oracle lanes,
6. explicit supported-subset boundaries.

### 3.3 Project / Hosting

This includes:
1. `.basproj`,
2. `.vbp` subset handling,
3. startup/entrypoint selection,
4. top-level mainline behavior,
5. CLI host/runtime policy surface,
6. host/project extensions beyond Excel VBA parity.

### 3.4 Language Services / Formalization

This includes:
1. parser/service feature inventory,
2. semantic model/service surfaces,
3. diagnostics model,
4. formal compiler/language representation progress,
5. the relationship between formal model progress and implemented feature claims.

## 4. Canonical Deliverables

The reset must produce the following active artifacts:
1. `docs/validation/VALIDATION_RESET_AUDIT_2026-03-29.md`
2. `docs/validation/VALIDATION_CANONICAL_OWNERSHIP_MAP_2026-03-29.md`
3. `docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv`
4. `docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv`
5. `docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv`
6. `docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv`
7. doctrine updates in:
   - `OPERATIONS.md`
   - `docs/LOCAL_EXECUTION_DOCTRINE.md`

Additional generated or derived active indexes may be introduced later, but these matrix files are the source-of-truth starting point.

## 5. Matrix Row Model

Every canonical matrix row must be capable of expressing:
1. feature or obligation id,
2. domain and subdomain,
3. authority type:
   - VBA spec,
   - observed Excel behavior,
   - OxVba extension spec,
   - internal formal-model requirement,
4. reference document or clause,
5. exact supported subset boundary,
6. compiler coverage state,
7. interpreter coverage state,
8. JIT coverage state,
9. oracle/evidence state,
10. formal-model state,
11. active tests and their locations,
12. current truth state:
   - planned,
   - in-progress,
   - implemented-subset,
   - implemented-full,
   - verified,
   - archived,
13. notes on ambiguity, divergence, or project decisions.

The row model must make it impossible to honestly describe a feature as closed without naming its subset and evidence.

## 6. Execution Method

This workset must use the BEADS method.

Method references:
1. `docs/methods/beads/BEADS_WORKING_METHOD.md`
2. `docs/methods/beads/BEADS_UTILITIES_CHEAT_SHEET.md`
3. `docs/methods/beads/BEADS_BREAKDOWN_EXAMPLE.md`
4. `docs/methods/beads/BEADS_BREAKDOWN_PROMPT.md`

Binding execution rule:
1. the workset defines the milestone and boundaries,
2. execution proceeds only through a bead subtree created from this workset,
3. each bead must represent one reviewable outcome with explicit completion evidence,
4. if a bead exposes uncovered required work, that work becomes a new bead before closure,
5. no broad area may be closed through narrative progress summaries alone.

## 7. Phases

### Phase A. Doctrine Reform

1. update `OPERATIONS.md` to require bead decomposition under active worksets,
2. update `docs/LOCAL_EXECUTION_DOCTRINE.md` with the local bead loop and anti-overclaim rules,
3. define strict subset-labeling and closure-language rules.

### Phase B. Truth-Surface Audit

1. inventory active conformance/completion/status artifacts,
2. classify each artifact as:
   - retain-active,
   - rewrite,
   - split,
   - archive,
3. identify the specific surfaces that overstate support.

### Phase C. Fresh Matrix Creation

1. create the four canonical validation matrices,
2. seed them with initial high-risk rows,
3. explicitly encode the `For Each` split:
   - arrays,
   - object enumerators / `NewEnum`.

### Phase D. Active Artifact Retirement

1. archive or mark superseded truth artifacts,
2. remove ambiguity about which files are authoritative,
3. preserve historical evidence while ending active reliance on unreliable summaries.

### Phase E. Systematic Validation Walk

Run the matrices in this order:
1. language,
2. COM / external integration,
3. project / hosting,
4. language services / formalization.

For each row:
1. verify the claim,
2. verify compiler/interpreter/JIT coverage,
3. verify oracle/evidence state,
4. add tests or downgrade claims,
5. create follow-up beads for missing work.

### Phase F. First Exposed Gap Fixes

Once the reset matrices are active and trusted, resume implementation bead-by-bead starting with the first newly exposed blocking gaps.

The current expected first canary is:
1. object `For Each` / `NewEnum` execution support.

## 8. Immediate First Slice

The first execution slice under this workset is:
1. create this workset,
2. update doctrine to adopt BEADS inside worksets,
3. create the audit scaffold,
4. create the fresh validation matrix source files,
5. seed initial rows that force the array/object `For Each` split to remain visible.

## 9. Acceptance Test For The New Process

The reset process should encounter `For Each` and force the following outcome:
1. array `For Each` remains a supported subset with evidence,
2. object `For Each` / enumerator support remains explicitly in-progress,
3. no active source-of-truth file can honestly describe the combined area as fully implemented or closed.

If the new process does not force that distinction, the reform is incomplete.

## 10. Initial Bead Slice

The first bead slice under `bd-gm3` is:
1. `bd-gm3.1` doctrine updates in `OPERATIONS.md` and `docs/LOCAL_EXECUTION_DOCTRINE.md`,
2. `bd-gm3.2` creation of this umbrella workset,
3. `bd-gm3.3` creation of the validation reset audit ledger,
4. `bd-gm3.4` seeding of the four canonical validation matrices,
5. `bd-gm3.5` expansion of the active truth-surface audit across the current repo artifacts,
6. `bd-gm3.6` first matrix canary review for `For Each` and adjacent loop semantics.

The reset does not advance to feature-repair execution until the truth-surface audit and the first canary pass are both explicit.

## 11. Epic Rollout Shape

This workset should ultimately roll out into the following execution epics:
1. workset initiation and epic rollout,
2. doctrine reform,
3. truth-surface audit and canonical ownership mapping,
4. canonical matrix foundation,
5. active artifact retirement and demotion,
6. systematic validation walk,
7. first exposed gap fixes.

Execution rule:
1. each epic may begin with a rollout bead that creates or refreshes its child bead set,
2. the workset is not considered fully rolled out until these execution lanes exist explicitly in the bead graph,
3. later discovery may add more beads or even more epics, but it must do so explicitly rather than narratively.

Current rollout state:
1. `bd-gm3.13` workset initiation and epic rollout
2. `bd-gm3.10` doctrine reform
3. `bd-gm3.15` truth-surface audit and canonical ownership mapping
4. `bd-gm3.11` canonical matrix foundation
5. `bd-gm3.16` active artifact retirement and demotion, closed on 2026-03-30
6. `bd-gm3.12` systematic validation walk, closed
7. `bd-gm3.14` first exposed gap fixes
8. `bd-gm3.14.4` residual LANG-0002/PH-0008 imported-runtime parity rollout

Current active unfinished execution lanes:
1. `bd-gm3.14` first exposed gap fixes
2. `bd-gm3.14.4` residual LANG-0002/PH-0008 imported-runtime parity rollout

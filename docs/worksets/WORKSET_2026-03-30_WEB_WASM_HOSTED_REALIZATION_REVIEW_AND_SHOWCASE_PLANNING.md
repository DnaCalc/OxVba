# Workset: Web/Wasm Hosted Realization Review and Showcase Planning

Date: 2026-03-30  
Status: completed  
Scope: review the current OxVba web/wasm substrate, determine the right hosted realization to pursue, assess whether there is an honest showcaseable slice today, and define the next execution workset for web-hosted and wasm environments.

## 0. Accepted Plan State

User direction for this workset was given on 2026-03-30.

This workset is therefore the active planning/review umbrella for the web/wasm theme.

Execution note:
1. this workset is review-first, not implementation-first,
2. execution proceeds through the bead subtree rooted at `bd-ae5`,
3. the first obligation is to distinguish what is only specified from what is actually runnable and evidence-backed,
4. no showcase claim may exceed current tested support.

## 1. Purpose

OxVba has accumulated meaningful wasm and host-policy substrate, but the product truth for a web-hosted realization is not yet crisply stated.

The immediate questions are:
1. what is the right hosted realization of OxVba for web and wasm environments,
2. which runtime classes and host contracts are already real versus only designed,
3. whether there is an honest demonstration/showcase slice that can be shown and tested now,
4. what follow-on execution workset should own the implementation and validation gaps.

This workset exists to answer those questions cleanly before a larger execution program starts.

## 2. Required Outcomes

This workset is complete only when all of the following are true:
1. the relevant current-state sources for wasm, browser-sandbox, wasi, hosting, and shell/UI direction are reviewed,
2. the recommended web/wasm hosted realization is stated explicitly,
3. the current showcaseable slice is described honestly with explicit boundaries,
4. the required validation surface for the showcase is identified,
5. the next implementation workset for the web/wasm theme is defined with a clear execution boundary.

## 3. Primary Questions To Answer

### 3.1 Realization Question

Which product shape is the right near-term realization:
1. pure wasm runtime in browser sandbox,
2. wasm runtime with a host-provided bridge,
3. desktop shell with web UI and Rust backend,
4. hybrid path with a desktop-first shell and later browser-native execution,
5. another constrained realization supported by current substrate.

### 3.2 Showcase Question

What can be honestly shown now:
1. pure compile/load/demo,
2. deterministic execution under a wasm profile,
3. host policy and capability-denial showcase,
4. shell/UI demonstration,
5. project-hosting demonstration,
6. or only a narrower review artifact rather than a runnable demo.

### 3.3 Validation Question

For the selected realization, what evidence is required across:
1. compiler,
2. interpreter,
3. JIT or non-JIT execution posture,
4. HAL/runtime profile enforcement,
5. project/hosting contract,
6. UI or shell behavior where relevant.

## 4. Canonical Inputs

The main source documents for this review are:
1. `docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md`
2. `docs/spec/HAL_WASM_RUNTIME_CLASSES_V1.md`
3. `docs/spec/HAL_RUNTIME_PROFILE_MATRIX_V1.md`
4. `docs/spec/HAL_OPERATING_ENVELOPE_V1.md`
5. `docs/DNAVBCALC_HOST_SHELL_BASELINE_PREPARATION_2026-03-09.md`
6. `MACH1000_PLAN.md`
7. current validation matrices where wasm/profile/project-hosting claims now live

Historical/archive discussion may be consulted only as supporting context, not as active truth.

## 5. Expected Deliverables

This workset should produce:
1. this workset document,
2. a review summary artifact under `docs/` that states:
   - recommended realization,
   - current honest showcase slice,
   - required validation lanes,
   - immediate gaps,
3. a next-workset recommendation for implementation/execution.

Produced artifacts:
1. `docs/reviews/WEB_WASM_HOSTED_REALIZATION_REVIEW_2026-03-30.md`
2. `docs/worksets/WORKSET_2026-03-30_WEB_WASM_DESKTOP_FIRST_HOST_SHELL_AND_BRIDGE_FOUNDATION.md`

## 6. Execution Method

This workset must use the BEADS method.

Binding execution rule:
1. this workset defines the review boundary,
2. work proceeds through explicit epics and rollout beads,
3. each bead must close one reviewable outcome,
4. if review exposes new mandatory questions, they become new beads before closure,
5. no implementation claim is upgraded merely because a design document exists.

## 7. Phase Structure

### Phase A. Workset Initiation and Rollout

1. register the workset in the tracker,
2. create explicit review epics,
3. create rollout beads and first executable review beads.

### Phase B. Current-State Review

1. review wasm runtime classes and host-policy posture,
2. review hosting/project/tooling proposals for UC-E and adjacent shell paths,
3. review current evidence and validation material for actual runnable support.

### Phase C. Realization and Showcase Recommendation

1. recommend the right near-term web/wasm realization,
2. define the honest showcase slice,
3. state what is not yet ready.

### Phase D. Validation and Next Workset Definition

1. define the validation lanes needed for the selected realization,
2. identify immediate gaps,
3. define the next execution workset that should own implementation.

## 8. Acceptance Test For This Review

The review succeeds only if it can answer the following without hand-waving:
1. what exactly we would run,
2. where it would run,
3. what host bridge or shell shape it depends on,
4. what tests/evidence already back it,
5. what remaining work prevents a broader showcase.

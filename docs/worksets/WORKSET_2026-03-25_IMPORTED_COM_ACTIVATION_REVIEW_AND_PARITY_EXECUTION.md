# Workset: Imported COM Activation Review and Parity Execution

Date: 2026-03-25  
Status: superseded in planning by `docs/worksets/WORKSET_2026-03-25_COM_ACTIVATION_TRUTH_REVIEW_AND_REPAIR.md`  
Scope: preserved as the narrower imported-activation precursor slice for `IP-05B`; active execution now continues in the broader COM activation truth-and-repair workset.

## 1. Purpose

This workset exists because the current imported COM story is not yet honest enough for closure language.

Current repo reality:
1. imported external `As New` no longer lowers to numeric `CreateObject(<selector>)`, but broad imported activation still lacks an authoritative real-library activation contract,
2. the live typelib path does not yet provide a trustworthy activation contract for arbitrary real COM libraries,
3. native Windows string-ProgID `CreateObject("...")` remains a real late-bound COM path and is not the primary blocker for this workset,
4. a narrow real registered early-bound anchor now exists for `Scripting.Dictionary` activation plus `Count`,
5. therefore `ODG-044` is not a pure oracle-scheduling item and `ODG-031` is not only a harness-construction problem.

This workset is the review-plus-implementation run to fix that correctly rather than papering over it with more selector aliases.

## 2. Problem Statement

The current imported COM implementation has three distinct truth gaps:
1. compiler/lowering truth gap:
   - imported `As New` activation now rejects unsupported non-authoritative cases instead of emitting numeric fallback, but the supported imported scope still needs a fully trustworthy real activation identity contract.
2. metadata-model truth gap:
   - imported typelib metadata does not yet serve as an authoritative source for real activation identity across the supported scope.
3. runtime-transport truth gap:
   - richer real-library member traffic on the new registered early-bound anchor still faults through projected callback transport.
4. claim-language truth gap:
   - current status/oracle wording can overread the implementation as if broad real-library import support were already complete.
5. scope-boundary truth gap:
   - selector-based activation must not be described as equivalent to the resolved string-ProgID late-bound COM lane.

## 3. Objectives

1. Review the imported COM activation path end to end:
   - compiler lowering,
   - imported typelib metadata,
   - host/HAL activation seam,
   - live typelib loader assumptions.
2. Define the correct activation contract for imported external COM types:
   - ProgID, CLSID, coclass identity, or explicit bounded fallback where authority is unavailable.
3. Implement the corrected activation path for the supported initial-scope target.
4. Add regressions that prove real registered imported activation on the supported scope.
5. Reconcile status/oracle docs so parity claims and remaining blockers match the implementation truth.

## 4. Execution Phases

### Phase A. Review and truth table

Deliverables:
1. trace the exact path from imported `Dim obj As New ...` to runtime activation,
2. classify which parts are:
   - authoritative,
   - selector/test-era scaffolding,
   - fallback,
   - non-authoritative for parity claims,
3. record the supported-vs-unsupported imported activation matrix.

Exit condition:
1. there is one explicit reviewed statement of the current imported activation model and its gaps.

### Phase B. Activation contract correction

Deliverables:
1. remove or narrow selector-era behavior where it distorts real imported COM activation,
   - numeric `CreateObject(<selector>)` fallback in imported lowering is removed from the live path,
2. carry authoritative activation identity through the supported imported path,
3. define deterministic behavior when imported metadata does not provide enough authority to activate honestly.

Exit condition:
1. supported imported `As New` activation no longer depends on a misleading test-era alias for real registered COM objects.
2. unsupported imported `As New` cases fail explicitly instead of lowering to non-VBA syntax.

### Phase C. Real-host regression coverage

Deliverables:
1. add or upgrade registered real-COM tests for the supported scope,
2. keep the new `scrrun` / `Scripting.Dictionary` activation-plus-Count anchor reproducible,
3. expand that anchor past the current `Add` / `Exists` callback-transport fault,
4. keep controlled `OxVba.TestDispatch` coverage separate from real external-library claims.

Exit condition:
1. the repo has a reproducible real-host regression lane for the supported imported early-bound target.
2. the next remaining fault on that lane is explicitly documented as member/event transport work, not activation ambiguity.

### Phase D. Oracle readiness foldback

Deliverables:
1. update `ODG-044` readiness docs based on the corrected implementation truth,
2. re-evaluate `ODG-031` scope wording based on the same review,
3. fold status/blocker/workset language back into the active ladder docs.

Exit condition:
1. `ODG-044` can be described honestly as either:
   - implementation-ready and waiting on oracle capture, or
   - still blocked with exact remaining implementation work.

## 5. Acceptance Criteria

This workset is only complete when all of the following are true:
1. imported real COM activation is described by one authoritative implementation model,
2. supported initial-scope imported `As New` activation uses a real activation identity or an explicitly bounded and documented equivalent,
3. the supported registered real-COM lane has permanent regression coverage,
4. `INITIAL_SCOPE_STATUS`, blocker notes, and oracle readiness docs all reflect the new truth,
5. no doc still implies that broad real COM library import parity is already complete when it is not.

## 6. Non-Goals

1. claiming arbitrary real COM library import parity without evidence,
2. treating a new selector alias by itself as closure of imported real-library support,
3. broad post-scope typelib/version-repair closure beyond the initial-scope target,
4. reopening the already-resolved native string-ProgID late-bound COM activation lane under `IP-03`.

## 7. Immediate Next Actions

1. finish the imported activation path review and capture the concrete seam that distorts `Scripting.Dictionary`,
2. implement the corrected supported activation path,
3. add registered-lane regressions,
4. fold back into `ODG-044` and `ODG-031` status/evidence docs.

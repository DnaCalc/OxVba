# Workset: COM Activation Truth Review and Repair

Date: 2026-03-25  
Status: in-progress  
Supersedes planning scope: `docs/worksets/WORKSET_2026-03-25_IMPORTED_COM_ACTIVATION_REVIEW_AND_PARITY_EXECUTION.md`  
Scope: review and correct the real COM activation story across imported early-bound activation, native late-bound ProgID activation, and adjacent deterministic fallback/projection scaffolding, with immediate closure focus on `ODG-031` and `ODG-044`.

## 1. Purpose

This workset exists because the repo currently has more than one COM activation story, and some docs overread them as if they were all equally parity-complete.

Current repo reality:
1. native Windows `CreateObject("ProgID")` remains the live late-bound activation path on Windows,
2. imported external `Dim obj As New Ref.Type` is narrower and still not backed by a general authoritative real-library activation contract,
3. deterministic fallback/projection/test scaffolding still exists in adjacent lanes and remains useful for bounded testing,
4. the new real registered early-bound `Scripting.Dictionary` anchor proves activation plus `Count`, but richer traffic still faults through callback transport,
5. therefore the remaining initial-scope COM closure work is broader than imported-lowering review alone.

## 2. Problem Statement

The current COM activation story has four distinct truth gaps:
1. imported early-bound activation authority gap:
   - imported `As New` currently succeeds only for a narrow real-library identity subset and is not yet a general trustworthy activation contract for arbitrary real COM libraries.
2. registered real-library transport gap:
   - the new `Scripting.Dictionary` early-bound anchor proves activation plus `Count`, but `Add` / `Exists` still fail through projected callback/event transport.
3. activation-boundary claim gap:
   - native Windows late-bound ProgID activation is real, but docs can still blur it with deterministic fallback/projection/test-era behavior.
4. status/register gap:
   - some repo-level closure language still overstates `IP-05` or broad COM closure relative to the actual supported scope.

## 3. Objectives

1. Review the full activation path matrix:
   - imported early-bound `As New`,
   - native late-bound `CreateObject("ProgID")`,
   - portable/test fallback and projection seams.
2. Record one explicit truth table for:
   - authoritative real activation,
   - bounded deterministic scaffolding,
   - unsupported cases that must fail explicitly.
3. Repair the supported imported early-bound activation path for the initial-scope target.
4. Fix the richer registered `Scripting.Dictionary` member traffic fault on the real early-bound lane.
5. Reconcile status/blocker/worklist/spec wording so parity claims match the implementation truth.

## 4. Execution Phases

### Phase A. Activation truth-table review

Deliverables:
1. trace the live paths for imported early-bound, native late-bound, and fallback/projection activation,
2. classify each seam as:
   - authoritative parity path,
   - bounded deterministic scaffolding,
   - unsupported/non-authoritative,
3. fold the reviewed matrix into active status docs.

Exit condition:
1. repo docs contain one explicit statement of which activation paths are real parity targets and which are scaffolding only.

### Phase B. Imported early-bound authority repair

Deliverables:
1. carry authoritative activation identity through the supported imported path,
2. keep unsupported imported activation failing explicitly,
3. avoid reintroducing selector-era or non-VBA lowering.

Exit condition:
1. supported imported `As New` activation no longer depends on an overbroad or misleading real-library authority claim.

### Phase C. Registered real-library transport repair

Deliverables:
1. extend the real registered `Scripting.Dictionary` lane beyond activation-plus-Count,
2. fix the richer `Add` / `Exists` transport fault,
3. add permanent regressions for the corrected supported member subset.

Exit condition:
1. the registered real-library anchor is honest enough to support side-by-side oracle closure work for `ODG-044`.

### Phase D. Late-bound activation boundary audit

Deliverables:
1. audit the native Windows `CreateObject("ProgID")` path and adjacent deterministic fallback/projection seams,
2. ensure docs/tests do not describe bounded scaffolding as equivalent to native parity support,
3. fix any real late-bound activation defect discovered by that audit.

Exit condition:
1. native late-bound activation claims are either upheld with explicit boundaries or corrected with code/tests.

### Phase E. Foldback and gate reconciliation

Deliverables:
1. update `INITIAL_SCOPE_STATUS`, `CURRENT_BLOCKERS`, and `IN_PROGRESS_FEATURE_WORKLIST`,
2. re-evaluate `ODG-031` and `ODG-044` wording against the repaired implementation truth,
3. leave one explicit next-step queue for oracle capture and formal follow-through.

Exit condition:
1. the repo no longer uses closure language that outruns the actual COM activation implementation state.

## 5. Acceptance Criteria

This workset is only complete when all of the following are true:
1. imported early-bound activation is described by one authoritative bounded model,
2. native Windows late-bound ProgID activation is documented/tested separately from deterministic scaffolding,
3. the registered `Scripting.Dictionary` early-bound lane proves more than activation-plus-Count for the supported initial-scope member subset,
4. active status/blocker/worklist docs all match the same activation-truth statement,
5. no doc still implies broad real-library COM activation parity when the supported scope is narrower.

## 6. Non-Goals

1. claiming arbitrary real COM library activation parity without evidence,
2. treating deterministic test scaffolding as closure of real COM activation support,
3. reopening the already-correct removal of numeric `CreateObject(<selector>)` lowering,
4. expanding beyond the initial-scope activation/truth gap into post-scope COM-server tiers.

## 7. Immediate Next Actions

1. finish the activation truth-table review and fold it into the active docs,
2. trace the registered `Scripting.Dictionary` `Add` / `Exists` callback-transport fault to the concrete runtime seam,
3. add or upgrade regressions on the supported real registered lane,
4. then return to the oracle harness items for `ODG-031` and `ODG-044`.

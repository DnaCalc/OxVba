# PROFILE_STATUS_V86.md

## Profile
- ID: mvp-full-typing-conformance-gate-v86
- Ladder step: v86

## Scope Summary
- Consolidated typing-ladder gate across formal, matrix, and benchmark lanes.
- Promoted default script profile targets to `v86` artifacts.
- Reconciled deferred-gate register state via foldback of completed strict runs and explicit deferred audit for unresolved lanes.

## Gate Artifacts
- scripts/run-profile-gate.ps1
- scripts/run-formal.ps1
- scripts/run-matrix.ps1
- scripts/run-bench.ps1
- docs/worksets/WORKSET_2026-02-28_FULL_TYPING_CONFORMANCE_GATE_V86.md
- docs/evidence/profiles/v86/integrated_gate.md
- docs/evidence/profiles/v86/matrix_latest.csv
- docs/evidence/profiles/v86/benchmark_latest.md
- docs/evidence/formal/DG_AUDIT_V86.md
- docs/PHASE12_STATUS.md

## Closure Signals
Profile `v86` is complete when integrated gate status is `PASS`, FO-V86-* obligations are pass, and deferred-gate rows are folded or explicitly deferred with unblock steps.

## Gate Result (2026-02-28)
- Integrated gate: pass (`docs/evidence/profiles/v86/integrated_gate.md`).
- Matrix gate: pass (`docs/evidence/profiles/v86/gate_report.md`, required cells `2/2` green).
- Formal obligations: pass through FO-V86 (`docs/evidence/formal/latest_run.md`).
- Deferred-gate reconciliation: completed/folded lanes and explicit deferred audit published (`docs/evidence/formal/DEFERRED_GATES.md`, `docs/evidence/formal/DG_AUDIT_V86.md`).

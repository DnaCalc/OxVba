# WORKSET_2026-02-27_RESUME_GOTO_V12.md

## Profile
- ID: mvp-resume-goto-v12
- Ladder step: v12

## Purpose
Execute and stabilize profile scope: GoTo 0 + Resume Next subset.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-resume-goto-v12 -OutputDir docs/evidence/profiles/v12
./scripts/run-formal.ps1 -ProfileScope mvp-resume-goto-v12
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

# WORKSET_2026-02-27_JIT_EXEC_V20.md

## Profile
- ID: mvp-jit-exec-v20
- Ladder step: v20

## Purpose
Execute and stabilize profile scope: JIT execution path activation with VM-equivalence.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-jit-exec-v20 -OutputDir docs/evidence/profiles/v20
./scripts/run-formal.ps1 -ProfileScope mvp-jit-exec-v20
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

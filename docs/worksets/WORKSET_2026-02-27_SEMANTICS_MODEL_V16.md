# WORKSET_2026-02-27_SEMANTICS_MODEL_V16.md

## Profile
- ID: mvp-semantics-model-v16
- Ladder step: v16

## Purpose
Execute and stabilize profile scope: Spec trace model checks.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-semantics-model-v16 -OutputDir docs/evidence/profiles/v16
./scripts/run-formal.ps1 -ProfileScope mvp-semantics-model-v16
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

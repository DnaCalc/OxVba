# WORKSET_2026-02-27_PROOF_INTEGRATION_V17.md

## Profile
- ID: mvp-proof-integration-v17
- Ladder step: v17

## Purpose
Execute and stabilize profile scope: Formal integration into standard quality gates.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-proof-integration-v17 -OutputDir docs/evidence/profiles/v17
./scripts/run-formal.ps1 -ProfileScope mvp-proof-integration-v17
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

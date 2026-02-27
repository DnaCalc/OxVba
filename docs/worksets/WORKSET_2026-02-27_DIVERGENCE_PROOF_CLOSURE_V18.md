# WORKSET_2026-02-27_DIVERGENCE_PROOF_CLOSURE_V18.md

## Profile
- ID: mvp-divergence-proof-closure-v18
- Ladder step: v18

## Purpose
Execute and stabilize profile scope: Divergence audit/proof linkage checks.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-divergence-proof-closure-v18 -OutputDir docs/evidence/profiles/v18
./scripts/run-formal.ps1 -ProfileScope mvp-divergence-proof-closure-v18
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

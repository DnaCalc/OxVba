# WORKSET_2026-02-27_OPTIMIZER_PACK2_V25.md

## Profile
- ID: mvp-optimizer-pack2-v25
- Ladder step: v25

## Purpose
Execute and stabilize profile scope: Optimizer pack2 + equivalence guardrails.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-optimizer-pack2-v25 -OutputDir docs/evidence/profiles/v25
./scripts/run-formal.ps1 -ProfileScope mvp-optimizer-pack2-v25
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

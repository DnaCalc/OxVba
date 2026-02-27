# WORKSET_2026-02-27_FORMAL_ASYNC_OPS_V27.md

## Profile
- ID: mvp-formal-async-ops-v27
- Ladder step: v27

## Purpose
Execute and stabilize profile scope: Async formal/Kani operations and evidence workflow stabilization.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-formal-async-ops-v27 -OutputDir docs/evidence/profiles/v27
./scripts/run-formal.ps1 -ProfileScope mvp-formal-async-ops-v27
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

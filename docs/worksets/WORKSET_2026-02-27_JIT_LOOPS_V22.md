# WORKSET_2026-02-27_JIT_LOOPS_V22.md

## Profile
- ID: mvp-jit-loops-v22
- Ladder step: v22

## Purpose
Execute and stabilize profile scope: JIT loop subset + backedge parity.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-jit-loops-v22 -OutputDir docs/evidence/profiles/v22
./scripts/run-formal.ps1 -ProfileScope mvp-jit-loops-v22
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

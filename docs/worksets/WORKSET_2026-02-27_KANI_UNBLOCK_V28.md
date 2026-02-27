# WORKSET_2026-02-27_KANI_UNBLOCK_V28.md

## Profile
- ID: mvp-kani-unblock-v28
- Ladder step: v28

## Purpose
Execute and stabilize profile scope: Kani unblock and harness decomposition for failing obligations.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-kani-unblock-v28 -OutputDir docs/evidence/profiles/v28
./scripts/run-formal.ps1 -ProfileScope mvp-kani-unblock-v28
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

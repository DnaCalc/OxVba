# WORKSET_2026-02-27_KANI_CAPACITY_V29.md

## Profile
- ID: mvp-kani-capacity-v29
- Ladder step: v29

## Purpose
Execute and stabilize profile scope: Kani capacity profiles and deterministic strict formal reruns.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-kani-capacity-v29 -OutputDir docs/evidence/profiles/v29
./scripts/run-formal.ps1 -ProfileScope mvp-kani-capacity-v29
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

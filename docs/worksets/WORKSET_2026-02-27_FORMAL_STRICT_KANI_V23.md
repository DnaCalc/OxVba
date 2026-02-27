# WORKSET_2026-02-27_FORMAL_STRICT_KANI_V23.md

## Profile
- ID: mvp-formal-strict-kani-v23
- Ladder step: v23

## Purpose
Execute and stabilize profile scope: Kani activation + strict formal lane wiring.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-formal-strict-kani-v23 -OutputDir docs/evidence/profiles/v23
./scripts/run-formal.ps1 -ProfileScope mvp-formal-strict-kani-v23
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

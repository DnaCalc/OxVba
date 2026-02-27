# WORKSET_2026-02-27_VARIANT_NUMERIC_V13.md

## Profile
- ID: mvp-variant-numeric-v13
- Ladder step: v13

## Purpose
Execute and stabilize profile scope: Variant numeric coercion core evidence.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-variant-numeric-v13 -OutputDir docs/evidence/profiles/v13
./scripts/run-formal.ps1 -ProfileScope mvp-variant-numeric-v13
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

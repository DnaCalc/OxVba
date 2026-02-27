# WORKSET_2026-02-27_COM_VARIANT_CONFORMANCE_V30.md

## Profile
- ID: mvp-com-variant-conformance-v30
- Ladder step: v30

## Purpose
Execute and stabilize profile scope: COM VARIANT conformance obligations and runtime compatibility checks.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-com-variant-conformance-v30 -OutputDir docs/evidence/profiles/v30
./scripts/run-formal.ps1 -ProfileScope mvp-com-variant-conformance-v30
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal

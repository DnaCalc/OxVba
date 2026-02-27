# WORKSET_2026-02-27_DATE_CURRENCY_V15.md

## Profile
- ID: mvp-date-currency-v15
- Ladder step: v15

## Purpose
Execute and stabilize profile scope: Date/Currency conversion-law scaffolding.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-date-currency-v15 -OutputDir docs/evidence/profiles/v15
./scripts/run-formal.ps1 -ProfileScope mvp-date-currency-v15
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
